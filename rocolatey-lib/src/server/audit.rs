//! Structured security audit event emission and retention for rocolatey.
//!
//! Server-side audit logs retain full fingerprints and correlation IDs.
//! Client-facing output omits full fingerprints (kept concise by design).
//!
//! Retention policy: 14 days, pruned on server startup.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const AUDIT_LOG_FILENAME: &str = "audit.log";
pub const AUDIT_RETENTION_DAYS: i64 = 14;

/// Structured security audit event kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// TLS materials bootstrapped on first start.
    Bootstrap,
    /// Authorized keys loaded or enrollment state changed.
    Enrollment,
    /// A request was denied by the authorization layer.
    DenyDecision,
    /// Identity material was rotated.
    Rotation,
    /// Server key continuity proof was checked.
    ContinuityCheck,
    /// Emergency trust override was accepted (audited path).
    EmergencyOverride,
    /// Certificate or clock skew expiry caused a fail-closed rejection.
    ExpiryFailure,
    /// The authorized_keys filesystem watcher encountered an error.
    WatcherFault,
    /// The authorized_keys parser skipped malformed line(s) and kept valid entries.
    WatcherWarning,
    /// The authorized_keys file was reloaded successfully by the watcher.
    WatcherReload,
}

/// A structured audit event. All events are written as a single JSON line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub kind: AuditEventKind,
    /// Server-assigned correlation ID for the triggering request, if applicable.
    pub request_id: Option<String>,
    /// Full SPKI SHA-256 fingerprint in base64url — server-side only, never sent to clients.
    pub full_fingerprint: Option<String>,
    /// Deny code from `RocoServerDenyCode`, serialized as snake_case string.
    pub deny_code: Option<String>,
    pub message: String,
}

impl AuditEvent {
    pub fn new(kind: AuditEventKind, message: impl Into<String>) -> Self {
        AuditEvent {
            timestamp: Utc::now(),
            kind,
            request_id: None,
            full_fingerprint: None,
            deny_code: None,
            message: message.into(),
        }
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub fn with_fingerprint(mut self, fp: impl Into<String>) -> Self {
        self.full_fingerprint = Some(fp.into());
        self
    }

    pub fn with_deny_code(mut self, code: impl Into<String>) -> Self {
        self.deny_code = Some(code.into());
        self
    }
}

/// Emit a structured security audit event to the given log path.
///
/// Best-effort: errors are printed to stderr but do not interrupt the caller.
/// Server-side logs use `server_audit_log_path()`; never write to client-facing outputs.
pub fn emit_audit_event(log_path: &Path, event: &AuditEvent) {
    if let Err(e) = try_emit_audit_event(log_path, event) {
        anstream::eprintln!(
            "[AUDIT-ERROR] failed to write audit event to {}: {}",
            log_path.display(),
            e
        );
    }
}

fn try_emit_audit_event(
    log_path: &Path,
    event: &AuditEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

/// Remove audit log entries older than `AUDIT_RETENTION_DAYS` days.
///
/// Retention is symmetric between client and server at 14 days.
/// Returns the number of pruned entries. Safe to call on a non-existent log.
pub fn prune_audit_log_if_due(log_path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    if !log_path.exists() {
        return Ok(0);
    }

    let cutoff = Utc::now() - chrono::Duration::days(AUDIT_RETENTION_DAYS);
    let content = std::fs::read_to_string(log_path)?;
    let mut retained: Vec<&str> = Vec::new();
    let mut pruned = 0usize;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEvent>(line) {
            Ok(event) if event.timestamp < cutoff => {
                pruned += 1;
            }
            _ => retained.push(line),
        }
    }

    if pruned > 0 {
        let new_content = if retained.is_empty() {
            String::new()
        } else {
            retained.join("\n") + "\n"
        };
        std::fs::write(log_path, new_content)?;
    }

    Ok(pruned)
}

/// Default server-side audit log path (same trust directory as server materials).
pub fn server_audit_log_path() -> PathBuf {
    super::default_server_trust_dir().join(AUDIT_LOG_FILENAME)
}

/// Default client-side audit log path (same trust directory as client materials).
pub fn client_audit_log_path() -> PathBuf {
    super::default_client_trust_dir().join(AUDIT_LOG_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_log_path() -> PathBuf {
        let id = uuid::Uuid::new_v4().to_string();
        std::env::temp_dir().join(format!("roco-audit-test-{}.log", id))
    }

    #[test]
    fn audit_event_round_trips_json() {
        let event = AuditEvent::new(AuditEventKind::Bootstrap, "Server bootstrapped")
            .with_request_id("req-123")
            .with_fingerprint("abc123fingerprint");

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();

        assert!(matches!(parsed.kind, AuditEventKind::Bootstrap));
        assert_eq!(parsed.request_id.as_deref(), Some("req-123"));
        assert_eq!(parsed.full_fingerprint.as_deref(), Some("abc123fingerprint"));
        assert_eq!(parsed.message, "Server bootstrapped");
    }

    #[test]
    fn audit_event_kinds_serialize_as_snake_case() {
        let cases: &[(AuditEventKind, &str)] = &[
            (AuditEventKind::Bootstrap, "bootstrap"),
            (AuditEventKind::DenyDecision, "deny_decision"),
            (AuditEventKind::EmergencyOverride, "emergency_override"),
            (AuditEventKind::WatcherFault, "watcher_fault"),
            (AuditEventKind::WatcherWarning, "watcher_warning"),
            (AuditEventKind::WatcherReload, "watcher_reload"),
            (AuditEventKind::ContinuityCheck, "continuity_check"),
            (AuditEventKind::ExpiryFailure, "expiry_failure"),
        ];
        for (kind, expected_str) in cases {
            let event = AuditEvent::new(kind.clone(), "msg");
            let json = serde_json::to_string(&event).unwrap();
            assert!(
                json.contains(expected_str),
                "expected '{}' in '{}'",
                expected_str,
                json
            );
        }
    }

    #[test]
    fn audit_log_write_and_prune_old_entries() {
        let log_path = make_temp_log_path();

        // Write an old event (>14 days ago)
        let old_event = AuditEvent {
            timestamp: Utc::now() - chrono::Duration::days(20),
            kind: AuditEventKind::DenyDecision,
            request_id: Some("old-req".to_string()),
            full_fingerprint: None,
            deny_code: Some("not_enrolled_client".to_string()),
            message: "Old deny event".to_string(),
        };
        emit_audit_event(&log_path, &old_event);

        // Write a recent event
        let new_event = AuditEvent::new(AuditEventKind::Enrollment, "Recent enrollment");
        emit_audit_event(&log_path, &new_event);

        let pruned = prune_audit_log_if_due(&log_path).unwrap();
        assert_eq!(pruned, 1);

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Recent enrollment"));
        assert!(!content.contains("Old deny event"));

        let _ = std::fs::remove_file(&log_path);
    }

    #[test]
    fn prune_nonexistent_log_returns_zero() {
        let log_path = make_temp_log_path();
        assert!(!log_path.exists());

        let pruned = prune_audit_log_if_due(&log_path).unwrap();
        assert_eq!(pruned, 0);
    }

    #[test]
    fn prune_keeps_all_recent_entries() {
        let log_path = make_temp_log_path();

        for i in 0..3 {
            let event =
                AuditEvent::new(AuditEventKind::Rotation, format!("rotation event {}", i));
            emit_audit_event(&log_path, &event);
        }

        let pruned = prune_audit_log_if_due(&log_path).unwrap();
        assert_eq!(pruned, 0);

        let _ = std::fs::remove_file(&log_path);
    }

    #[test]
    fn emit_audit_event_creates_parent_dirs() {
        let id = uuid::Uuid::new_v4().to_string();
        let log_path = std::env::temp_dir()
            .join(format!("roco-audit-subdir-{}", id))
            .join("nested")
            .join("audit.log");

        let event = AuditEvent::new(AuditEventKind::Bootstrap, "nested dir test");
        emit_audit_event(&log_path, &event);

        assert!(log_path.exists());
        let _ = std::fs::remove_dir_all(log_path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn audit_event_without_optional_fields() {
        let event = AuditEvent::new(AuditEventKind::WatcherFault, "watcher failed");
        assert!(event.request_id.is_none());
        assert!(event.full_fingerprint.is_none());
        assert!(event.deny_code.is_none());

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();
        assert!(parsed.request_id.is_none());
    }

    #[test]
    fn retention_policy_is_14_days() {
        assert_eq!(AUDIT_RETENTION_DAYS, 14);
    }
}
