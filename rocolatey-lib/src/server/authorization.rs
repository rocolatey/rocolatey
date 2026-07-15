use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    RocoServerDenyCode, RocoServerDenyResponse, ROCO_SERVER_SCHEMA_VERSION,
};

pub const ROTATION_CADENCE_DAYS: i64 = 90;
pub const OVERLAP_WINDOW_DAYS: i64 = 30;
pub const ROTATION_JITTER_MAX_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
pub const MAX_CLOCK_SKEW_SECONDS: i64 = 8 * 60;
pub const EMERGENCY_TRUST_OVERRIDE_ENV: &str = "ROCO_EMERGENCY_TRUST_OVERRIDE";
pub const EMERGENCY_TRUST_OVERRIDE_TTL_SECONDS: i64 = 15 * 60;
pub const EMERGENCY_TRUST_OVERRIDE_HEADER: &str = "x-roco-client-fingerprint";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthorizedKeysWarning {
    pub line_number: usize,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthorizedKeysParseResult {
    pub fingerprints: Vec<String>,
    pub warnings: Vec<AuthorizedKeysWarning>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EmergencyOverrideRecord {
    pub fingerprint: String,
    pub expires_at_utc: DateTime<Utc>,
    pub remaining_uses: u32,
    pub last_used_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateTimeStatus {
    Valid,
    Expired,
    TimeSkewExceeded,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct RotationWindow {
    pub issued_at: DateTime<Utc>,
    pub rotate_after: DateTime<Utc>,
    pub overlap_until: DateTime<Utc>,
    pub jitter_seconds: i64,
}

/// Parse the authorized_keys file and return a set of valid fingerprints.
/// Lines starting with '#' or blank lines are ignored.
/// Each line should contain a single SPKI SHA-256 fingerprint in base64url format.
pub fn parse_authorized_keys(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect()
}

pub fn parse_authorized_keys_with_warnings(content: &str) -> AuthorizedKeysParseResult {
    let mut fingerprints = Vec::new();
    let mut warnings = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line_number = idx + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.split_whitespace().count() != 1 {
            warnings.push(AuthorizedKeysWarning {
                line_number,
                message: "expected one fingerprint token per line".to_string(),
            });
            continue;
        }

        if !line
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            warnings.push(AuthorizedKeysWarning {
                line_number,
                message: "invalid fingerprint: only base64url characters are allowed"
                    .to_string(),
            });
            continue;
        }

        fingerprints.push(line.to_string());
    }

    AuthorizedKeysParseResult {
        fingerprints,
        warnings,
    }
}

/// Parse and validate authorized_keys content with strict policy:
/// - one token per line
/// - comments and blank lines are ignored
/// - fingerprints must be base64url
pub fn parse_authorized_keys_strict(content: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.split_whitespace().count() != 1 {
            return Err(format!(
                "invalid authorized_keys format at line {}: expected one fingerprint token per line",
                idx + 1
            ));
        }

        if !line
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(format!(
                "invalid authorized_keys fingerprint at line {}: only base64url characters are allowed",
                idx + 1
            ));
        }

        out.push(line.to_string());
    }

    Ok(out)
}

/// Load authorized keys from file path
pub fn load_authorized_keys(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_authorized_keys(&content))
}

pub fn load_authorized_keys_with_warnings(
    path: &Path,
) -> Result<AuthorizedKeysParseResult, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_authorized_keys_with_warnings(&content))
}

pub fn load_authorized_keys_strict(
    path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    parse_authorized_keys_strict(&content)
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::InvalidData, msg).into())
}

/// Compute the SPKI SHA-256 fingerprint of a certificate in base64url format
pub fn fingerprint_from_cert(cert_pem: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut hasher = Sha256::new();
    hasher.update(cert_pem);
    let digest = hasher.finalize();
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.as_slice()))
}

/// Get the short form of a fingerprint (first 12 characters)
pub fn short_fingerprint(full_fp: &str) -> String {
    full_fp.chars().take(12).collect()
}

pub fn deterministic_rotation_jitter_seconds(seed: &str) -> i64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let raw = u64::from_be_bytes(bytes);

    let span = (ROTATION_JITTER_MAX_SECONDS * 2 + 1) as u64;
    let offset = (raw % span) as i64;
    offset - ROTATION_JITTER_MAX_SECONDS
}

pub fn compute_rotation_window(
    issued_at: DateTime<Utc>,
    deterministic_seed: &str,
) -> RotationWindow {
    let jitter_seconds = deterministic_rotation_jitter_seconds(deterministic_seed);
    let rotate_after = issued_at + Duration::days(ROTATION_CADENCE_DAYS) + Duration::seconds(jitter_seconds);
    let overlap_until = rotate_after + Duration::days(OVERLAP_WINDOW_DAYS);

    RotationWindow {
        issued_at,
        rotate_after,
        overlap_until,
        jitter_seconds,
    }
}

pub fn build_continuity_proof(previous_fingerprint: &str, next_fingerprint: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous_fingerprint.as_bytes());
    hasher.update(b"->");
    hasher.update(next_fingerprint.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

pub fn verify_continuity_proof(
    previous_fingerprint: &str,
    next_fingerprint: &str,
    proof: &str,
) -> bool {
    build_continuity_proof(previous_fingerprint, next_fingerprint) == proof
}

pub fn emergency_trust_override_enabled() -> bool {
    std::env::var(EMERGENCY_TRUST_OVERRIDE_ENV)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

pub fn configured_emergency_override_fingerprint() -> Option<String> {
    std::env::var(EMERGENCY_TRUST_OVERRIDE_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn is_base64url_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub fn configured_emergency_override_matches(client_fingerprint: &str) -> bool {
    configured_emergency_override_fingerprint()
        .filter(|fp| is_base64url_token(fp))
        .map(|fp| fp == client_fingerprint)
        .unwrap_or(false)
}

pub fn load_emergency_override_record(
    path: &Path,
) -> Result<Option<EmergencyOverrideRecord>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let record = serde_json::from_str::<EmergencyOverrideRecord>(&content)?;
    Ok(Some(record))
}

fn write_emergency_override_record(
    path: &Path,
    record: &EmergencyOverrideRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = serde_json::to_vec_pretty(record)?;
    crate::bootstrap::write_atomic_with_backup(path, &content)?;
    Ok(())
}

pub fn consume_emergency_override(
    path: &Path,
    client_fingerprint: &str,
    request_id: &str,
    now: DateTime<Utc>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !configured_emergency_override_matches(client_fingerprint) {
        return Ok(false);
    }

    let mut record = match load_emergency_override_record(path)? {
        Some(existing) if existing.fingerprint != client_fingerprint => return Ok(false),
        Some(existing) if existing.expires_at_utc > now => existing,
        _ => EmergencyOverrideRecord {
            fingerprint: client_fingerprint.to_string(),
            expires_at_utc: now + Duration::seconds(EMERGENCY_TRUST_OVERRIDE_TTL_SECONDS),
            remaining_uses: 1,
            last_used_request_id: None,
        },
    };

    if record.remaining_uses == 0 || record.expires_at_utc <= now {
        return Ok(false);
    }

    record.remaining_uses -= 1;
    record.last_used_request_id = Some(request_id.to_string());
    write_emergency_override_record(path, &record)?;
    Ok(true)
}

pub fn evaluate_certificate_time(
    cert_pem: &[u8],
    now: SystemTime,
) -> Result<CertificateTimeStatus, Box<dyn std::error::Error>> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let not_before = cert.validity().not_before.timestamp();
    let not_after = cert.validity().not_after.timestamp();
    let now_ts = now.duration_since(UNIX_EPOCH)?.as_secs() as i64;

    if now_ts > not_after {
        return Ok(CertificateTimeStatus::Expired);
    }

    if now_ts + MAX_CLOCK_SKEW_SECONDS < not_before {
        return Ok(CertificateTimeStatus::TimeSkewExceeded);
    }

    Ok(CertificateTimeStatus::Valid)
}

pub fn evaluate_certificate_time_now(
    cert_pem: &[u8],
) -> Result<CertificateTimeStatus, Box<dyn std::error::Error>> {
    evaluate_certificate_time(cert_pem, SystemTime::now())
}

/// Check if a certificate fingerprint is in the authorized keys file
pub fn is_authorized(
    cert_fingerprint: &str,
    authorized_keys_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !authorized_keys_path.exists() {
        return Ok(false);
    }

    let authorized_keys = load_authorized_keys(authorized_keys_path)?;
    Ok(authorized_keys.contains(&cert_fingerprint.to_string()))
}

/// Get the count of authorized keys
pub fn authorized_key_count(
    authorized_keys_path: &Path,
) -> Result<usize, Box<dyn std::error::Error>> {
    if !authorized_keys_path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(authorized_keys_path)?;
    let keys = parse_authorized_keys(&content);
    Ok(keys.len())
}

/// Check if the system is in empty enrollment mode (0 authorized keys)
pub fn is_empty_enrollment_mode(
    authorized_keys_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let count = authorized_key_count(authorized_keys_path)?;
    Ok(count == 0)
}

/// Build a client-facing denial response for an unauthorized or unauthenticated client
pub fn build_deny_response(
    request_id: String,
    code: RocoServerDenyCode,
    message: String,
    cert_fingerprint: Option<&str>,
    enrollment_hint: Option<String>,
) -> RocoServerDenyResponse {
    let short_fingerprint = cert_fingerprint.map(|fp| short_fingerprint(fp));
    
    RocoServerDenyResponse {
        schema_version: ROCO_SERVER_SCHEMA_VERSION,
        request_id,
        code,
        message,
        enrollment_hint,
        short_fingerprint,
    }
}

/// Check client authorization for a protected route.
/// Returns None if authorized, Some(deny_response) if unauthorized.
pub fn check_authorization(
    request_id: String,
    cert_pem: Option<&[u8]>,
    authorized_keys_path: &Path,
) -> Result<Option<RocoServerDenyResponse>, Box<dyn std::error::Error>> {
    // Check if client certificate is present
    let cert_pem = match cert_pem {
        Some(cert) => cert,
        None => {
            return Ok(Some(build_deny_response(
                request_id,
                RocoServerDenyCode::ClientCertificateMissing,
                "Client certificate is required for this endpoint".to_string(),
                None,
                Some("Please generate a client certificate and establish a mutual TLS connection. Use 'roco server --gen-cert' to generate client credentials.".to_string()),
            )));
        }
    };

    // Compute fingerprint from certificate
    let cert_fingerprint = fingerprint_from_cert(cert_pem)?;

    // Check if in empty enrollment mode
    let empty_enrollment = is_empty_enrollment_mode(authorized_keys_path)?;
    if empty_enrollment {
        return Ok(Some(build_deny_response(
            request_id,
            RocoServerDenyCode::NotEnrolledClient,
            format!(
                "Server is in empty enrollment mode. Your client (fingerprint: {}) is not yet enrolled.",
                short_fingerprint(&cert_fingerprint)
            ),
            Some(&cert_fingerprint),
            Some(format!(
                "Server has no authorized clients. An administrator must add your client fingerprint to the authorized_keys file. Your fingerprint is: {}",
                cert_fingerprint
            )),
        )));
    }

    // Check if client is authorized
    let is_auth = is_authorized(&cert_fingerprint, authorized_keys_path)?;
    if !is_auth {
        return Ok(Some(build_deny_response(
            request_id,
            RocoServerDenyCode::NotEnrolledClient,
            format!(
                "Client certificate not authorized. Fingerprint: {}",
                short_fingerprint(&cert_fingerprint)
            ),
            Some(&cert_fingerprint),
            Some(format!(
                "This client is not enrolled for remote access. Contact your administrator to add your fingerprint to the authorized keys: {}",
                cert_fingerprint
            )),
        )));
    }

    Ok(None) // Client is authorized
}


#[cfg(test)]
mod tests {
    use super::*;

    static OVERRIDE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_parse_authorized_keys() {
        let content = r#"
# This is a comment
abc123def456ghi789jkl012mno345pqr

# Another comment

xyz789uvw456rst123opq012mno345pqr
"#;
        let keys = parse_authorized_keys(content);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"abc123def456ghi789jkl012mno345pqr".to_string()));
        assert!(keys.contains(&"xyz789uvw456rst123opq012mno345pqr".to_string()));
    }

    #[test]
    fn test_short_fingerprint() {
        let full = "abc123def456ghi789jkl012mno345pqr";
        let short = short_fingerprint(full);
        assert_eq!(short, "abc123def456");
    }

    #[test]
    fn tolerant_parser_keeps_valid_and_reports_warnings() {
        let content = "\n# comment\nvalid_token\nbad token pair\ninvalid!token\nnext_valid\n";
        let parsed = parse_authorized_keys_with_warnings(content);

        assert_eq!(parsed.fingerprints, vec!["valid_token", "next_valid"]);
        assert_eq!(parsed.warnings.len(), 2);
        assert_eq!(parsed.warnings[0].line_number, 4);
        assert_eq!(parsed.warnings[1].line_number, 5);
    }

    #[test]
    fn tolerant_loader_does_not_fail_on_malformed_lines() {
        let path = std::env::temp_dir().join(format!(
            "roco-authorized-keys-tolerant-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, "good_token\nbad token\nalso_good\ninvalid!\n").unwrap();

        let parsed = load_authorized_keys_with_warnings(&path).unwrap();
        assert_eq!(parsed.fingerprints, vec!["good_token", "also_good"]);
        assert_eq!(parsed.warnings.len(), 2);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn strict_parser_rejects_whitespace_tokens() {
        let content = "good_token second_token\n";
        let err = parse_authorized_keys_strict(content).expect_err("must reject multi token line");
        assert!(err.contains("line 1"));
    }

    #[test]
    fn jitter_is_deterministic() {
        let a = deterministic_rotation_jitter_seconds("seed-a");
        let b = deterministic_rotation_jitter_seconds("seed-a");
        assert_eq!(a, b);
        assert!(a >= -ROTATION_JITTER_MAX_SECONDS);
        assert!(a <= ROTATION_JITTER_MAX_SECONDS);
    }

    #[test]
    fn continuity_proof_roundtrip() {
        let old_fp = "oldfingerprint";
        let new_fp = "newfingerprint";
        let proof = build_continuity_proof(old_fp, new_fp);
        assert!(verify_continuity_proof(old_fp, new_fp, &proof));
        assert!(!verify_continuity_proof(old_fp, "other", &proof));
    }

    #[test]
    fn emergency_override_requires_exact_fingerprint_match() {
        let _guard = OVERRIDE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var(EMERGENCY_TRUST_OVERRIDE_ENV, "override-fingerprint");
        assert!(configured_emergency_override_matches("override-fingerprint"));
        assert!(!configured_emergency_override_matches("different-fingerprint"));
        std::env::remove_var(EMERGENCY_TRUST_OVERRIDE_ENV);
    }

    #[test]
    fn emergency_override_is_one_shot_within_ttl() {
        let _guard = OVERRIDE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = std::env::temp_dir().join(format!(
            "roco-emergency-override-{}.json",
            uuid::Uuid::new_v4()
        ));
        let now = Utc::now();

        std::env::set_var(EMERGENCY_TRUST_OVERRIDE_ENV, "override-fingerprint");

        let first = consume_emergency_override(
            &path,
            "override-fingerprint",
            "req-1",
            now,
        )
        .unwrap();
        let second = consume_emergency_override(
            &path,
            "override-fingerprint",
            "req-2",
            now + Duration::seconds(5),
        )
        .unwrap();

        assert!(first);
        assert!(!second);

        let record = load_emergency_override_record(&path).unwrap().unwrap();
        assert_eq!(record.remaining_uses, 0);
        assert_eq!(record.last_used_request_id.as_deref(), Some("req-1"));

        std::env::remove_var(EMERGENCY_TRUST_OVERRIDE_ENV);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn emergency_override_reopens_after_ttl_expiry() {
        let _guard = OVERRIDE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = std::env::temp_dir().join(format!(
            "roco-emergency-override-{}.json",
            uuid::Uuid::new_v4()
        ));
        let now = Utc::now();

        std::env::set_var(EMERGENCY_TRUST_OVERRIDE_ENV, "override-fingerprint");

        assert!(consume_emergency_override(&path, "override-fingerprint", "req-1", now).unwrap());
        assert!(consume_emergency_override(
            &path,
            "override-fingerprint",
            "req-2",
            now + Duration::seconds(EMERGENCY_TRUST_OVERRIDE_TTL_SECONDS + 1),
        )
        .unwrap());

        std::env::remove_var(EMERGENCY_TRUST_OVERRIDE_ENV);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod phase7_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    // --- Helpers ---

    /// Generate a self-signed cert PEM valid within [now - 1d, now + days_valid].
    fn make_cert_valid_for_days(days_valid: i64) -> Vec<u8> {
        use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
        let mut params = CertificateParams::new(vec!["localhost".to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "roco-test");
        let not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
        let not_after = not_before + time::Duration::days(days_valid + 1);
        params.not_before = not_before;
        params.not_after = not_after;
        Certificate::from_params(params)
            .unwrap()
            .serialize_pem()
            .unwrap()
            .into_bytes()
    }

    /// Generate a cert PEM that expired `days_ago` days ago.
    fn make_expired_cert(days_ago: i64) -> Vec<u8> {
        use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
        let mut params = CertificateParams::new(vec!["localhost".to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "roco-expired");
        let not_before = time::OffsetDateTime::now_utc() - time::Duration::days(days_ago + 5);
        let not_after = not_before + time::Duration::days(4); // expired days_ago days ago
        params.not_before = not_before;
        params.not_after = not_after;
        Certificate::from_params(params)
            .unwrap()
            .serialize_pem()
            .unwrap()
            .into_bytes()
    }

    /// Generate a cert PEM whose validity starts far in the future (beyond clock skew).
    fn make_future_cert(hours_in_future: i64) -> Vec<u8> {
        use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
        let mut params = CertificateParams::new(vec!["localhost".to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "roco-future");
        let not_before =
            time::OffsetDateTime::now_utc() + time::Duration::hours(hours_in_future);
        let not_after = not_before + time::Duration::days(90);
        params.not_before = not_before;
        params.not_after = not_after;
        Certificate::from_params(params)
            .unwrap()
            .serialize_pem()
            .unwrap()
            .into_bytes()
    }

    fn make_temp_keys_file(content: &str) -> std::path::PathBuf {
        let id = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!("roco-test-keys-{}", id));
        std::fs::write(&path, content).unwrap();
        path
    }

    // --- Certificate time evaluation ---

    #[test]
    fn cert_time_valid_returns_valid() {
        let cert_pem = make_cert_valid_for_days(90);
        let status = evaluate_certificate_time_now(&cert_pem).unwrap();
        assert_eq!(status, CertificateTimeStatus::Valid);
    }

    #[test]
    fn cert_time_expired_returns_expired() {
        let cert_pem = make_expired_cert(2);
        let status = evaluate_certificate_time_now(&cert_pem).unwrap();
        assert_eq!(status, CertificateTimeStatus::Expired);
    }

    #[test]
    fn cert_time_future_beyond_skew_returns_time_skew_exceeded() {
        // Cert starts 2 hours in the future, MAX_CLOCK_SKEW_SECONDS = 8 min → time skew exceeded
        let cert_pem = make_future_cert(2);
        let status = evaluate_certificate_time_now(&cert_pem).unwrap();
        assert_eq!(status, CertificateTimeStatus::TimeSkewExceeded);
    }

    #[test]
    fn cert_time_evaluate_with_explicit_time_expired() {
        let cert_pem = make_cert_valid_for_days(1);
        // Simulate checking "in the future" when cert is no longer valid
        let far_future = SystemTime::now() + Duration::from_secs(365 * 24 * 60 * 60 * 2);
        let status = evaluate_certificate_time(&cert_pem, far_future).unwrap();
        assert_eq!(status, CertificateTimeStatus::Expired);
    }

    // --- Deny response code mapping ---

    #[test]
    fn deny_response_maps_code_and_preserves_request_id() {
        let response = build_deny_response(
            "req-abc-123".to_string(),
            RocoServerDenyCode::NotEnrolledClient,
            "not enrolled".to_string(),
            None,
            Some("enroll hint".to_string()),
        );
        assert_eq!(response.code, RocoServerDenyCode::NotEnrolledClient);
        assert_eq!(response.request_id, "req-abc-123");
        assert!(response.enrollment_hint.is_some());
        assert!(response.short_fingerprint.is_none());
        assert_eq!(response.schema_version, ROCO_SERVER_SCHEMA_VERSION);
    }

    #[test]
    fn deny_response_with_fingerprint_shows_short_12_chars() {
        let full_fp = "abcdefghijklmnopqrstuvwxyz012345";
        let response = build_deny_response(
            "req-xyz".to_string(),
            RocoServerDenyCode::NotEnrolledClient,
            "not enrolled".to_string(),
            Some(full_fp),
            None,
        );
        assert_eq!(response.short_fingerprint.as_deref(), Some("abcdefghijkl"));
    }

    #[test]
    fn deny_response_all_codes_are_constructable() {
        let codes = [
            RocoServerDenyCode::NotEnrolledClient,
            RocoServerDenyCode::ClientCertificateMissing,
            RocoServerDenyCode::ClientCertificateInvalid,
            RocoServerDenyCode::ClientCertificateExpired,
            RocoServerDenyCode::TimeSkewExceeded,
            RocoServerDenyCode::ServerTrustMismatch,
            RocoServerDenyCode::InvalidRequest,
            RocoServerDenyCode::ForbiddenOperation,
            RocoServerDenyCode::ResourceNotFound,
            RocoServerDenyCode::InternalAuthorizationError,
        ];
        for code in codes {
            let r = build_deny_response(
                "r".to_string(),
                code.clone(),
                "msg".to_string(),
                None,
                None,
            );
            assert_eq!(r.code, code);
        }
    }

    // --- Rotation window ---

    #[test]
    fn rotation_window_rotate_after_is_approximately_90_days() {
        let now = chrono::Utc::now();
        let window = compute_rotation_window(now, "test-seed-rotation");
        let delta = window.rotate_after - now;
        // Should be 90 days ± 7 days jitter
        assert!(delta.num_days() >= 83);
        assert!(delta.num_days() <= 97);
    }

    #[test]
    fn rotation_window_overlap_is_exactly_30_days_after_rotate() {
        let now = chrono::Utc::now();
        let window = compute_rotation_window(now, "test-seed-overlap");
        let overlap_delta = window.overlap_until - window.rotate_after;
        assert_eq!(overlap_delta.num_days(), OVERLAP_WINDOW_DAYS);
    }

    #[test]
    fn rotation_window_different_seeds_produce_different_jitter() {
        let now = chrono::Utc::now();
        let a = compute_rotation_window(now, "seed-alpha");
        let b = compute_rotation_window(now, "seed-beta");
        // Different seeds should (almost always) produce different jitter values
        assert_ne!(a.jitter_seconds, b.jitter_seconds);
    }

    #[test]
    fn rotation_jitter_is_within_bounds() {
        for seed in &["s1", "s2", "s3", "long-seed-value", "12345"] {
            let j = deterministic_rotation_jitter_seconds(seed);
            assert!(
                j >= -ROTATION_JITTER_MAX_SECONDS && j <= ROTATION_JITTER_MAX_SECONDS,
                "jitter {} out of bounds for seed {}",
                j,
                seed
            );
        }
    }

    // --- Request correlation: UUID format ---

    #[test]
    fn build_deny_response_request_id_is_uuid_shaped() {
        let request_id = uuid::Uuid::new_v4().to_string();
        // UUIDs are 36 chars with hyphens
        assert_eq!(request_id.len(), 36);
        assert_eq!(request_id.chars().filter(|c| *c == '-').count(), 4);

        let response = build_deny_response(
            request_id.clone(),
            RocoServerDenyCode::InternalAuthorizationError,
            "err".to_string(),
            None,
            None,
        );
        assert_eq!(response.request_id, request_id);
    }

    // --- check_authorization with temp files ---

    #[test]
    fn check_authorization_no_cert_returns_missing() {
        let keys_path = make_temp_keys_file("");
        let request_id = uuid::Uuid::new_v4().to_string();
        let result = check_authorization(request_id, None, &keys_path).unwrap();
        assert!(result.is_some());
        let deny = result.unwrap();
        assert_eq!(deny.code, RocoServerDenyCode::ClientCertificateMissing);
        let _ = std::fs::remove_file(keys_path);
    }

    #[test]
    fn check_authorization_empty_enrollment_denies_client() {
        let keys_path = make_temp_keys_file("");
        let cert_pem = make_cert_valid_for_days(90);
        let request_id = uuid::Uuid::new_v4().to_string();
        let result = check_authorization(request_id, Some(&cert_pem), &keys_path).unwrap();
        assert!(result.is_some());
        let deny = result.unwrap();
        assert_eq!(deny.code, RocoServerDenyCode::NotEnrolledClient);
        assert!(deny.enrollment_hint.is_some());
        let _ = std::fs::remove_file(keys_path);
    }

    #[test]
    fn check_authorization_not_enrolled_client_denied() {
        // Keys file has one entry that doesn't match
        let keys_path = make_temp_keys_file("differentfingerprint0000000000000000000000000\n");
        let cert_pem = make_cert_valid_for_days(90);
        let request_id = uuid::Uuid::new_v4().to_string();
        let result = check_authorization(request_id, Some(&cert_pem), &keys_path).unwrap();
        assert!(result.is_some());
        let deny = result.unwrap();
        assert_eq!(deny.code, RocoServerDenyCode::NotEnrolledClient);
        let _ = std::fs::remove_file(keys_path);
    }

    #[test]
    fn check_authorization_enrolled_client_allowed() {
        let cert_pem = make_cert_valid_for_days(90);
        // Compute the actual fingerprint and add it to the keys file
        let fingerprint = fingerprint_from_cert(&cert_pem).unwrap();
        let keys_path = make_temp_keys_file(&format!("{}\n", fingerprint));

        let request_id = uuid::Uuid::new_v4().to_string();
        let result = check_authorization(request_id, Some(&cert_pem), &keys_path).unwrap();
        assert!(result.is_none(), "enrolled client should be authorized");
        let _ = std::fs::remove_file(keys_path);
    }

    // --- Trust-file parsing edge cases ---

    #[test]
    fn parse_authorized_keys_ignores_empty_lines_and_comments() {
        let content = "\n# comment\nfp1\n\n# another comment\nfp2\n";
        let keys = parse_authorized_keys(content);
        assert_eq!(keys, vec!["fp1", "fp2"]);
    }

    #[test]
    fn parse_authorized_keys_strict_accepts_valid_content() {
        let content = "validtoken123\nanother_valid-token\n# comment\n";
        let result = parse_authorized_keys_strict(content).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_authorized_keys_strict_rejects_invalid_chars() {
        let content = "bad!token\n";
        let err = parse_authorized_keys_strict(content).expect_err("should reject non-base64url");
        assert!(err.contains("line 1"));
    }

    #[test]
    fn parse_authorized_keys_strict_rejects_multiple_tokens_on_one_line() {
        let content = "token1 token2\n";
        let err = parse_authorized_keys_strict(content).expect_err("should reject multi-token");
        assert!(err.contains("line 1"));
    }

    // --- Continuity proof ---

    #[test]
    fn continuity_proof_verify_rejects_tampered_proof() {
        let old_fp = "fingerprint-old";
        let new_fp = "fingerprint-new";
        let proof = build_continuity_proof(old_fp, new_fp);
        assert!(!verify_continuity_proof(old_fp, "other-new", &proof));
        assert!(!verify_continuity_proof("other-old", new_fp, &proof));
        assert!(!verify_continuity_proof(old_fp, new_fp, "tampered"));
    }
}
