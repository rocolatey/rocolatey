use rocolatey_lib::bootstrap;
use rocolatey_lib::server::{ClientTlsConfig, ServerTlsConfig, ServerTlsDiagnostics};
use std::path::Path;
use std::fs;

pub async fn server(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));

    if matches.get_flag("setup-tls-help") {
        setup_tls_help();
        return;
    }

    if matches.get_flag("gen-cert") {
        let force = matches.get_flag("force");
        gen_cert(force);
        return;
    }

    if matches.get_flag("bootstrap-local-trust") {
        if let Err(e) = bootstrap_local_trust() {
            anstream::eprintln!("[ERROR] Failed to bootstrap local trust: {}", e);
            std::process::exit(1);
        }
        return;
    }

    anstream::eprintln!("roco server: use --setup-tls-help, --gen-cert, or --bootstrap-local-trust");
    std::process::exit(1);
}

fn setup_tls_help() {
    anstream::println!("\n=== roco Server TLS Setup Guide ===\n");
    let client_cfg = rocolatey_lib::server::ClientTlsConfig::default();
    let server_cfg = ServerTlsConfig::default();
    anstream::println!("Setup instructions:");
    anstream::println!("  1. Run on client host: roco server --gen-cert");
    anstream::println!("  2. Run on server host: roco server --gen-cert");
    anstream::println!("  2a. Single-host bootstrap (installer account scope): roco server --bootstrap-local-trust");
    anstream::println!("  3. Copy the server's certificate PEM to the client's known_server_keys file:");
    anstream::println!("     scp server:{} {}", server_cfg.cert_path.display(), client_cfg.known_server_keys_path.display());
    anstream::println!("  4. Add one enrolled client fingerprint per line in server authorized_keys:");
    anstream::println!("     (see fingerprint printed by 'roco server --gen-cert')");
    anstream::println!("     Path: {}", server_cfg.authorized_keys_path.display());
    anstream::println!("  5. Start rocolatey-server and verify enrollment mode clears\n");
    anstream::println!(
        "  6. Emergency override (audited only): set {}=<client_fingerprint> only during incident recovery\n",
        rocolatey_lib::server::authorization::EMERGENCY_TRUST_OVERRIDE_ENV,
    );

    let client_diags = rocolatey_lib::roco::client_tls::get_client_diagnostics();
    anstream::println!("--- Client TLS Materials ---");
    print_tls_diagnostics(&client_diags);

    let mut actionable_problem = false;

    if !client_diags.materials_ready {
        actionable_problem = true;
        anstream::println!("\n[WARN] Client materials missing or incomplete.");
        anstream::println!("\nTo generate client TLS materials, run:");
        anstream::println!("  roco server --gen-cert");
    } else {
        anstream::println!("\n[OK] Client materials OK\n");
    }

    match get_server_diagnostics_safe() {
        Ok(server_diags) => {
            anstream::println!("--- Server TLS Materials ---");
            print_server_diagnostics(&server_diags);
            if !server_diags.materials_ready {
                actionable_problem = true;
            }
            if server_diags.in_empty_enrollment_mode {
                actionable_problem = true;
                anstream::println!("\n[WARN] Server is in EMPTY ENROLLMENT MODE (no authorized clients).");
                anstream::println!("Enroll at least one client fingerprint into authorized_keys.");
            } else if server_diags.materials_ready {
                anstream::println!("\n[OK] Server materials OK\n");
            }
        }
        Err(e) => {
            actionable_problem = true;
            anstream::println!("--- Server TLS Materials ---");
            anstream::println!("[WARN] Cannot access server materials: {}", e);
        }
    }

    anstream::println!("\nEnrollment steps:");
    anstream::println!("  1. Generate certs on each client host: roco server --gen-cert");
    anstream::println!("  2. Read each client cert fingerprint and append to server authorized_keys");
    anstream::println!("  3. Keep one fingerprint token per line (base64url, comments with # allowed)");
    anstream::println!("  4. Restart rocolatey-server if needed\n");

    if actionable_problem {
        std::process::exit(1);
    }
}

fn print_tls_diagnostics(diags: &rocolatey_lib::roco::client_tls::ClientTlsDiagnostics) {
    anstream::println!("Certificate: {}", diags.cert_path.display());
    anstream::println!("  Exists: {}", if diags.cert_exists { "yes" } else { "no" });
    anstream::println!("Private key: {}", diags.key_path.display());
    anstream::println!("  Exists: {}", if diags.key_exists { "yes" } else { "no" });
}

fn print_server_diagnostics(diags: &ServerTlsDiagnostics) {
    anstream::println!("Certificate: {}", diags.cert_path.display());
    anstream::println!("  Exists: {}", if diags.cert_exists { "yes" } else { "no" });
    anstream::println!("Private key: {}", diags.key_path.display());
    anstream::println!("  Exists: {}", if diags.key_exists { "yes" } else { "no" });
    anstream::println!("Authorized keys: {}", diags.authorized_keys_path.display());
    anstream::println!("  Exists: {}", if diags.authorized_keys_exist { "yes" } else { "no" });
    anstream::println!("  Valid: {}", if diags.authorized_keys_valid { "yes" } else { "no" });
    anstream::println!("  Entry count: {}", diags.authorized_keys_entry_count);
}

fn get_server_diagnostics_safe() -> Result<ServerTlsDiagnostics, String> {
    let config = ServerTlsConfig::default();
    let cert_dir = config.cert_path.parent().unwrap_or_else(|| Path::new("/"));
    if !cert_dir.exists() {
        return Err("Server trust directories not found".to_string());
    }
    Ok(bootstrap::get_server_diagnostics())
}

fn gen_cert(force: bool) {
    match bootstrap::bootstrap_client_tls(&ClientTlsConfig::default()) {
        Ok(_) => {
            anstream::println!("[OK] Client TLS materials ready");
            print_cert_fingerprint("Client", &ClientTlsConfig::default().cert_path);
        }
        Err(e) => {
            anstream::eprintln!("[ERROR] Failed to generate client TLS materials: {}", e);
            std::process::exit(1);
        }
    }

    match gen_server_certs(force) {
        Ok((cert_backup, key_backup)) => {
            anstream::println!("[OK] Server TLS materials ready");
            print_cert_fingerprint("Server", &ServerTlsConfig::default().cert_path);

            if force {
                // Auto-update local client's pinned server cert after forced rotation
                let server_cfg = ServerTlsConfig::default();
                let client_cfg = ClientTlsConfig::default();
                if let Ok(server_cert) = fs::read(&server_cfg.cert_path) {
                    if let Err(e) = bootstrap::write_atomic_with_backup(
                        &client_cfg.known_server_keys_path,
                        &server_cert,
                    ) {
                        anstream::eprintln!(
                            "[WARN] Could not update local pinned server cert: {}. Manual update required.",
                            e
                        );
                    } else {
                        anstream::println!("[OK] Local client pinned server cert updated automatically");
                    }
                }

                anstream::println!("[WARN] Forced server key rotation executed.");
                anstream::println!("Remediation for remote clients:");
                anstream::println!("  1. Distribute updated server fingerprint to remote clients");
                anstream::println!("  2. Remote clients must re-pin the server cert in known_server_keys");
                anstream::println!("  3. Keep backups until all clients can re-authenticate");
                if let Some(path) = cert_backup {
                    anstream::println!("  Backup certificate: {}", path.display());
                }
                if let Some(path) = key_backup {
                    anstream::println!("  Backup key: {}", path.display());
                }
                anstream::println!(
                    "  Continuity proof: {}",
                    ServerTlsConfig::default().continuity_proof_path().display()
                );
            }
        }
        Err(e) => {
            anstream::eprintln!("[WARN] Could not generate server TLS materials: {}", e);
        }
    }
}

fn print_cert_fingerprint(label: &str, cert_path: &std::path::Path) {
    match std::fs::read(cert_path) {
        Ok(cert_pem) => {
            let short = bootstrap::fingerprint_short(&cert_pem);
            let full = bootstrap::fingerprint_full(&cert_pem);
            match (short, full) {
                (Ok(s), Ok(f)) => {
                    anstream::println!("{} certificate fingerprint (short): {}", label, s);
                    anstream::println!("{} certificate fingerprint (full): {}", label, f);
                }
                (Err(e), _) | (_, Err(e)) => {
                    anstream::eprintln!("[WARN] Could not compute {} certificate fingerprint: {}", label, e);
                }
            }
        }
        Err(e) => {
            anstream::eprintln!("[WARN] Could not read {} certificate for fingerprint output: {}", label, e);
        }
    }
}

fn gen_server_certs(force: bool) -> Result<(Option<std::path::PathBuf>, Option<std::path::PathBuf>), Box<dyn std::error::Error>> {
    let config = ServerTlsConfig::default();
    if config.cert_exists() && config.key_exists() && !force {
        return Ok((None, None));
    }
    if force {
        bootstrap::regenerate_server_tls_with_backup(&config)
    } else {
        bootstrap::bootstrap_server_tls(&config)?;
        Ok((None, None))
    }
}

fn bootstrap_local_trust() -> Result<(), Box<dyn std::error::Error>> {
    let client_cfg = ClientTlsConfig::default();
    let server_cfg = ServerTlsConfig::default();

    // Ensure trust directories exist before any read/write operations
    if let Some(parent) = client_cfg.cert_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = server_cfg.cert_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Account-scoped by design: client paths come from the current process user profile
    // while server trust remains machine/global via ServerTlsConfig default path.
    bootstrap::bootstrap_client_tls(&client_cfg)?;
    bootstrap::bootstrap_server_tls(&server_cfg)?;

    let server_cert = fs::read(&server_cfg.cert_path)?;
    bootstrap::write_atomic_with_backup(&client_cfg.known_server_keys_path, &server_cert)?;

    let client_cert = fs::read(&client_cfg.cert_path)?;
    let client_fingerprint = bootstrap::fingerprint_full(&client_cert)?;

    let authorized_keys_current = if server_cfg.authorized_keys_path.exists() {
        fs::read_to_string(&server_cfg.authorized_keys_path)?
    } else {
        String::new()
    };

    let existing = rocolatey_lib::server::authorization::parse_authorized_keys_strict(
        &authorized_keys_current,
    )
    .map_err(|msg| std::io::Error::new(std::io::ErrorKind::InvalidData, msg))?;

    let mut added_fingerprint = false;
    if !existing.iter().any(|v| v == &client_fingerprint) {
        let mut updated = authorized_keys_current;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(&client_fingerprint);
        updated.push('\n');
        bootstrap::write_atomic_with_backup(&server_cfg.authorized_keys_path, updated.as_bytes())?;
        added_fingerprint = true;
    }

    let account = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERPROFILE").map(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(p)
        }))
        .unwrap_or_else(|_| {
            anstream::eprintln!("[WARN] Could not detect account name from environment; using fallback 'current-user' in audit log");
            "current-user".to_string()
        });

    if added_fingerprint {
        anstream::println!(
            "[OK] Local trust bootstrap complete for account '{}': client fingerprint enrolled.",
            account
        );
    } else {
        anstream::println!(
            "[OK] Local trust bootstrap complete for account '{}': client fingerprint already enrolled.",
            account
        );
    }
    anstream::println!("Client cert: {}", client_cfg.cert_path.display());
    anstream::println!("Client key: {}", client_cfg.key_path.display());
    anstream::println!(
        "Pinned server cert store: {}",
        client_cfg.known_server_keys_path.display()
    );
    anstream::println!(
        "Server enrollment file: {}",
        server_cfg.authorized_keys_path.display()
    );

    rocolatey_lib::server::audit::emit_audit_event(
        &rocolatey_lib::server::audit::server_audit_log_path(),
        &rocolatey_lib::server::audit::AuditEvent::new(
            rocolatey_lib::server::audit::AuditEventKind::Enrollment,
            format!(
                "Installer-account scoped local trust bootstrap for '{}'{}",
                account,
                if added_fingerprint {
                    " (fingerprint added)"
                } else {
                    " (fingerprint already present)"
                }
            ),
        )
        .with_fingerprint(client_fingerprint),
    );

    Ok(())
}
