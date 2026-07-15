use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Clone)]
pub struct AuthorizedKeysRuntime {
    pub path: PathBuf,
    keys: Arc<RwLock<Vec<String>>>,
    watcher_healthy: Arc<AtomicBool>,
}

impl AuthorizedKeysRuntime {
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let parsed = rocolatey_lib::server::authorization::load_authorized_keys_with_warnings(path)?;
        for warning in &parsed.warnings {
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::WatcherWarning,
                    format!(
                        "authorized_keys startup load skipped malformed line {}: {}",
                        warning.line_number, warning.message
                    ),
                ),
            );
        }
        Ok(Self {
            path: path.to_path_buf(),
            keys: Arc::new(RwLock::new(parsed.fingerprints)),
            watcher_healthy: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn empty_unhealthy(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            keys: Arc::new(RwLock::new(Vec::new())),
            watcher_healthy: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn authorized_key_count(&self) -> usize {
        match self.keys.read() {
            Ok(guard) => guard.len(),
            Err(poisoned) => {
                // Recover from poisoned lock — the inner data is still usable
                let guard = poisoned.into_inner();
                guard.len()
            }
        }
    }

    pub fn watcher_healthy(&self) -> bool {
        self.watcher_healthy.load(Ordering::Relaxed)
    }

    fn update_keys_or_recover(
        keys: &Arc<RwLock<Vec<String>>>,
        new_keys: Vec<String>,
        healthy: &Arc<AtomicBool>,
        is_healthy: bool,
    ) {
        match keys.write() {
            Ok(mut guard) => {
                *guard = new_keys;
            }
            Err(poisoned) => {
                // Recover from poisoned lock — the inner data is still usable
                let mut guard = poisoned.into_inner();
                *guard = new_keys;
                eprintln!("[HIGH] authorized_keys lock was poisoned; recovered key set");
                rocolatey_lib::server::audit::emit_audit_event(
                    &rocolatey_lib::server::audit::server_audit_log_path(),
                    &rocolatey_lib::server::audit::AuditEvent::new(
                        rocolatey_lib::server::audit::AuditEventKind::WatcherFault,
                        "authorized_keys RwLock was poisoned; recovered key set from poison".to_string(),
                    ),
                );
            }
        }
        healthy.store(is_healthy, Ordering::Relaxed);
    }

    pub fn start_watcher(&self) {
        let watch_path = self.path.clone();
        let watch_filename = self.path.file_name().map(|n| n.to_os_string());
        let watch_parent = self.path.parent().unwrap_or(&self.path).to_path_buf();
        let keys = self.keys.clone();
        let healthy = self.watcher_healthy.clone();

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();

            let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
                let _ = tx.send(res);
            }) {
                Ok(v) => v,
                Err(err) => {
                    healthy.store(false, Ordering::Relaxed);
                    eprintln!(
                        "[HIGH] authorized_keys watcher startup failed: {}. Keeping last known good key set.",
                        err
                    );
                    rocolatey_lib::server::audit::emit_audit_event(
                        &rocolatey_lib::server::audit::server_audit_log_path(),
                        &rocolatey_lib::server::audit::AuditEvent::new(
                            rocolatey_lib::server::audit::AuditEventKind::WatcherFault,
                            format!("authorized_keys watcher startup failed: {}. Using last known good key set.", err),
                        ),
                    );
                    return;
                }
            };

            // Watch the parent directory so the watcher survives file deletion/recreation
            if let Err(err) = watcher.watch(&watch_parent, RecursiveMode::NonRecursive) {
                healthy.store(false, Ordering::Relaxed);
                eprintln!(
                    "[HIGH] authorized_keys watcher attach failed for {}: {}. Keeping last known good key set.",
                    watch_parent.display(),
                    err
                );
                rocolatey_lib::server::audit::emit_audit_event(
                    &rocolatey_lib::server::audit::server_audit_log_path(),
                    &rocolatey_lib::server::audit::AuditEvent::new(
                        rocolatey_lib::server::audit::AuditEventKind::WatcherFault,
                        format!(
                            "authorized_keys watcher attach failed for {}: {}. Using last known good key set.",
                            watch_parent.display(),
                            err
                        ),
                    ),
                );
                return;
            }

            while let Ok(event) = rx.recv() {
                match event {
                    Ok(ev)
                        if matches!(
                            ev.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        ) =>
                    {
                        // Only reload when the target file (or its temp variant) is affected
                        let relevant = ev.paths.iter().any(|p| {
                            p.file_name() == watch_filename.as_deref()
                                || p.file_name()
                                    .and_then(|n| n.to_str())
                                    .map_or(false, |n| n.starts_with(".tmp.") && {
                                        // Check if the temp file belongs to our target
                                        watch_filename.as_ref().map_or(false, |target| {
                                            let target_str = target.to_string_lossy();
                                            n.contains(&*target_str)
                                        })
                                    })
                        });
                        if !relevant {
                            continue;
                        }

                        match rocolatey_lib::server::authorization::load_authorized_keys_with_warnings(
                            &watch_path,
                        ) {
                            Ok(parsed) => {
                                let count = parsed.fingerprints.len();
                                Self::update_keys_or_recover(&keys, parsed.fingerprints, &healthy, true);
                                for warning in &parsed.warnings {
                                    rocolatey_lib::server::audit::emit_audit_event(
                                        &rocolatey_lib::server::audit::server_audit_log_path(),
                                        &rocolatey_lib::server::audit::AuditEvent::new(
                                            rocolatey_lib::server::audit::AuditEventKind::WatcherWarning,
                                            format!(
                                                "authorized_keys reload skipped malformed line {}: {}",
                                                warning.line_number, warning.message
                                            ),
                                        ),
                                    );
                                }
                                rocolatey_lib::server::audit::emit_audit_event(
                                    &rocolatey_lib::server::audit::server_audit_log_path(),
                                    &rocolatey_lib::server::audit::AuditEvent::new(
                                        rocolatey_lib::server::audit::AuditEventKind::WatcherReload,
                                        format!(
                                            "authorized_keys reloaded successfully: {} entries from {}",
                                            count,
                                            watch_path.display()
                                        ),
                                    ),
                                );
                            }
                            Err(err) => {
                                // File may have been deleted; fall back to empty set
                                Self::update_keys_or_recover(&keys, Vec::new(), &healthy, false);
                                eprintln!(
                                    "[HIGH] authorized_keys reload failed for {}: {}. Using empty key set until file is restored.",
                                    watch_path.display(),
                                    err
                                );
                                rocolatey_lib::server::audit::emit_audit_event(
                                    &rocolatey_lib::server::audit::server_audit_log_path(),
                                    &rocolatey_lib::server::audit::AuditEvent::new(
                                        rocolatey_lib::server::audit::AuditEventKind::WatcherFault,
                                        format!(
                                            "authorized_keys reload failed for {}: {}. Using empty key set until file is restored.",
                                            watch_path.display(),
                                            err
                                        ),
                                    ),
                                );
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        healthy.store(false, Ordering::Relaxed);
                        eprintln!(
                            "[HIGH] authorized_keys watcher runtime error for {}: {}. Keeping last known good key set.",
                            watch_path.display(),
                            err
                        );
                        rocolatey_lib::server::audit::emit_audit_event(
                            &rocolatey_lib::server::audit::server_audit_log_path(),
                            &rocolatey_lib::server::audit::AuditEvent::new(
                                rocolatey_lib::server::audit::AuditEventKind::WatcherFault,
                                format!(
                                    "authorized_keys watcher runtime error for {}: {}. Using last known good key set.",
                                    watch_path.display(),
                                    err
                                ),
                            ),
                        );
                    }
                }
            }
        });
    }
}
