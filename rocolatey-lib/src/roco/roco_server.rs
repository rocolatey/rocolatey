use reqwest::header::CONTENT_TYPE;
use serde_json;
use std::time::Duration;

use crate::server::{JobStatus, RocoServerChocoCommandRequest, ClientTlsConfig, RocoServerTrustRenewResponse};
use reqwest::ClientBuilder;
use crate::roco::client_tls;

fn client_fingerprint_header_value(
    tls_config: &ClientTlsConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let cert_pem = std::fs::read(&tls_config.cert_path)?;
    crate::bootstrap::fingerprint_full(&cert_pem)
}

fn determine_scheme() -> &'static str {
    "https"
}

fn build_client() -> Result<reqwest::Client, Box<dyn std::error::Error>> {
    let tls_config = ClientTlsConfig::default();
    build_client_with_config(&tls_config)
}

fn build_client_with_config(
    tls_config: &ClientTlsConfig,
) -> Result<reqwest::Client, Box<dyn std::error::Error>> {
    if client_tls::client_tls_materials_exist(&tls_config.cert_path, &tls_config.key_path) {
        // Pinned server certificate is mandatory. We refuse to connect without it so that
        // self-signed server certs are validated against a known-good copy rather than
        // accepted blindly. There is no fallback to danger_accept_invalid_certs.
        if !tls_config.known_server_keys_exist() {
            return Err(format!(
                "Pinned server certificate not found at {}. \
                 Copy the server's certificate PEM there before running remote commands \
                 (see `roco setup` for guidance).",
                tls_config.known_server_keys_path.display()
            ).into());
        }

        let pem = tls_config.read_known_server_keys().map_err(|e| {
            format!(
                "Failed to read pinned server certificate at {}: {}",
                tls_config.known_server_keys_path.display(),
                e
            )
        })?;

        let cert = reqwest::Certificate::from_pem(pem.as_bytes()).map_err(|e| {
            format!(
                "Pinned server certificate at {} is not valid PEM: {}. \
                 Re-copy the server's certificate PEM file.",
                tls_config.known_server_keys_path.display(),
                e
            )
        })?;

        let builder = ClientBuilder::new()
            .tls_built_in_root_certs(false)
            // Bound handshake/response time so TLS failures are surfaced promptly.
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .add_root_certificate(cert);

        Ok(builder.build()?)
    } else {
        Err(format!(
            "TLS client materials not found at {} / {}. \n\
             Run `roco setup` to generate client credentials before connecting to a rocolatey server.",
            tls_config.cert_path.display(),
            tls_config.key_path.display()
        ).into())
    }
}

fn resolve_server_url(server_ip: &str, server_port: u16, scheme: &str, request_path: &str) -> String {
    format!("{}://{}:{}/rocolatey{}", scheme, server_ip, server_port, request_path)
}

fn print_deny_if_present(endpoint: &str, body: &str) -> bool {
    let deny = match serde_json::from_str::<crate::server::RocoServerDenyResponse>(body) {
        Ok(v) => v,
        Err(_) => return false,
    };

    anstream::eprintln!(
        "Server deny on {}: code={:?} request_id={} message={}",
        endpoint, deny.code, deny.request_id, deny.message
    );

    if let Some(hint) = deny.enrollment_hint {
        anstream::eprintln!("Hint: {}", hint);
    }

    if let Some(fp) = deny.short_fingerprint {
        anstream::eprintln!("Fingerprint: {}", fp);
    }

    true
}

pub async fn run_on_server_simple_get(request_path: &str, request_body: &str) -> Option<String> {
    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let scheme = determine_scheme();
    let url = resolve_server_url(&ip, port.parse().unwrap_or_default(), scheme, request_path);
    let tls_config = ClientTlsConfig::default();

    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            anstream::eprintln!("Failed to build HTTP client: {}", e);
            return None;
        }
    };

    match client
        .get(&url)
        .header(CONTENT_TYPE, "application/json")
        .header(
            crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
            client_fingerprint_header_value(&tls_config).unwrap_or_default(),
        )
        .body(request_body.to_string())
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(txt) => Some(txt),
            Err(e) => {
                anstream::eprintln!("Request to {} failed: {}", url, e);
                None
            }
        },
        Err(e) => {
            if handle_tls_cert_error(&e).await {
                // Renewal succeeded — caller should retry the original request
                anstream::eprintln!("[INFO] Server certificate renewed. Please retry your request.");
            } else {
                anstream::eprintln!("Request to {} failed: {}", url, e);
            }
            None
        }
    }
}

pub async fn run_on_server_simple_get_with_config(
    server_ip: &str,
    server_port: u16,
    request_path: &str,
    request_body: &str,
    tls_config: &ClientTlsConfig,
) -> Option<String> {
    let url = resolve_server_url(server_ip, server_port, "https", request_path);

    let client = match build_client_with_config(tls_config) {
        Ok(c) => c,
        Err(e) => {
            anstream::eprintln!("Failed to build HTTP client: {}", e);
            return None;
        }
    };

    match client
        .get(&url)
        .header(CONTENT_TYPE, "application/json")
        .header(
            crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
            client_fingerprint_header_value(tls_config).unwrap_or_default(),
        )
        .body(request_body.to_string())
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(txt) => Some(txt),
            Err(e) => {
                anstream::eprintln!("Request to {} failed: {}", url, e);
                None
            }
        },
        Err(e) => {
            if handle_tls_cert_error(&e).await {
                anstream::eprintln!("[INFO] Server certificate renewed. Please retry your request.");
            } else {
                anstream::eprintln!("Request to {} failed: {}", url, e);
            }
            None
        }
    }
}

pub async fn run_on_server_simple_post(request_path: &str, request_body: &str) -> Option<String> {
    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let scheme = determine_scheme();
    let url = resolve_server_url(&ip, port.parse().unwrap_or_default(), scheme, request_path);
    let tls_config = ClientTlsConfig::default();

    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            anstream::eprintln!("Failed to build HTTP client: {}", e);
            return None;
        }
    };

    match client
        .post(&url)
        .header(CONTENT_TYPE, "application/json")
        .header(
            crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
            client_fingerprint_header_value(&tls_config).unwrap_or_default(),
        )
        .body(request_body.to_string())
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(txt) => Some(txt),
            Err(e) => {
                anstream::eprintln!("Request to {} failed: {}", url, e);
                None
            }
        },
        Err(e) => {
            if handle_tls_cert_error(&e).await {
                anstream::eprintln!("[INFO] Server certificate renewed. Please retry your request.");
            } else {
                anstream::eprintln!("Request to {} failed: {}", url, e);
            }
            None
        }
    }
}

pub async fn run_on_server_simple_post_with_config(
    server_ip: &str,
    server_port: u16,
    request_path: &str,
    request_body: &str,
    tls_config: &ClientTlsConfig,
) -> Option<String> {
    let url = resolve_server_url(server_ip, server_port, "https", request_path);

    let client = match build_client_with_config(tls_config) {
        Ok(c) => c,
        Err(e) => {
            anstream::eprintln!("Failed to build HTTP client: {}", e);
            return None;
        }
    };

    match client
        .post(&url)
        .header(CONTENT_TYPE, "application/json")
        .header(
            crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
            client_fingerprint_header_value(tls_config).unwrap_or_default(),
        )
        .body(request_body.to_string())
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(txt) => Some(txt),
            Err(e) => {
                anstream::eprintln!("Request to {} failed: {}", url, e);
                None
            }
        },
        Err(e) => {
            if handle_tls_cert_error(&e).await {
                anstream::eprintln!("[INFO] Server certificate renewed. Please retry your request.");
            } else {
                anstream::eprintln!("Request to {} failed: {}", url, e);
            }
            None
        }
    }
}

/// Run a Chocolatey command on a remote rocolatey server and poll for completion.
///
/// What it does:
/// - Builds a JSON `RocoServerChocoCommandRequest` from the provided
///   `choco_args` and `package_names` and POSTs it to the server at
///   `https://<ROCO_SERVER_IP>:<ROCO_SERVER_PORT>/rocolatey/choco` (or http:// if no TLS).
/// - Expects the server to respond with a job id. It then polls
///   `GET /rocolatey/choco/status/<id>` to fetch
///   the job state and logs.
/// - New log lines received from the server are printed immediately
///   with `anstream::println!`, so they are streamed to the caller's stdout as
///   they arrive (they are not captured or returned by this function).
/// - Returns `true` when the server reports `JobStatus::Completed` and
///   `false` for `JobStatus::Failed` or persistent errors.
///
/// Error handling and retries:
/// - If the initial POST or JSON parsing fails the function logs an
///   error to stderr and returns `false`.
/// - While polling, transient failures to fetch or parse status are
///   retried; persistent failures increment an internal `err_count`.
///   Once `err_count` reaches 5 the function returns `false`.
///
/// Notes:
/// - Logs are printed incrementally using `anstream::println!`, so they appear
///   in real time on stdout; stderr from the remote job is expected to
///   be included in the server-provided `logs` list if applicable.
pub async fn run_on_server_poll(choco_args: &[&str], package_names: &[&str]) -> i32 {
    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let scheme = determine_scheme();
    let base = format!("{}://{}:{}/rocolatey", scheme, ip, port);
    let poll_millis = crate::server::get_server_poll_interval_millis();

    let command = choco_args
        .get(0)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upgrade".to_string());
    let mut args: Vec<String> = choco_args.iter().skip(1).map(|s| s.to_string()).collect();
    args.extend(package_names.iter().map(|s| s.to_string()));
    let body = RocoServerChocoCommandRequest { command, args };

    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            anstream::eprintln!("Failed to build HTTP client: {}", e);
            return -1;
        }
    };

    // send POST -> get id
    let body_json = serde_json::to_string(&body).expect("serialize body");
    let resp = client
        .post(&format!("{}/choco", base))
        .header(CONTENT_TYPE, "application/json")
        .header(
            crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
            client_fingerprint_header_value(&ClientTlsConfig::default()).unwrap_or_default(),
        )
        .body(body_json)
        .send()
        .await;
    let id = match resp {
        Ok(r) => {
            let status = r.status();
            let txt = match r.text().await {
                Ok(t) => t,
                Err(e) => {
                    anstream::eprintln!("request failed: {}", e);
                    return -1;
                }
            };

            if print_deny_if_present("/choco", &txt) {
                return -1;
            }

            if !status.is_success() {
                anstream::eprintln!("request failed with status {}: {}", status, txt);
                return -1;
            }

            match serde_json::from_str::<crate::server::RocoServerChocoJobIdResponse>(&txt) {
                Ok(j) => j.id,
                Err(e) => {
                    anstream::eprintln!("request failed: {}", e);
                    return -1;
                }
            }
        }
        Err(e) => {
            if handle_tls_cert_error(&e).await {
                anstream::eprintln!("[INFO] Server certificate renewed. Please retry your request.");
            } else {
                anstream::eprintln!("request failed: {}", e);
            }
            return -1;
        }
    };

    let mut err_count = 0;
    // poll
    let mut last_log_idx = 0usize;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(poll_millis)).await;
        let status_resp = client
            .get(&format!("{}/choco/status/{}", base, id))
            .header(
                crate::server::authorization::EMERGENCY_TRUST_OVERRIDE_HEADER,
                client_fingerprint_header_value(&ClientTlsConfig::default()).unwrap_or_default(),
            )
            .send()
            .await;
        match status_resp {
            Ok(r) => {
                let status = r.status();
                let txt = match r.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        anstream::eprintln!("status query failed: {}", e);
                        err_count += 1;
                        if err_count >= 5 {
                            return -1;
                        }
                        continue;
                    }
                };

                if print_deny_if_present("/choco/status", &txt) {
                    return -1;
                }

                if !status.is_success() {
                    anstream::eprintln!("status query failed with status {}: {}", status, txt);
                    err_count += 1;
                    if err_count >= 5 {
                        return -1;
                    }
                    continue;
                }

                match serde_json::from_str::<crate::server::JobState>(&txt) {
                    Ok(js) => {
                        // print new logs
                        for line in js.logs.iter().skip(last_log_idx) {
                            anstream::println!("{}", line);
                        }
                        last_log_idx = js.logs.len();
                        match js.status {
                            JobStatus::Pending | JobStatus::Running => continue,
                            JobStatus::Completed { exit_code } => return exit_code,
                            JobStatus::Failed { .. } => return -1,
                        }
                    }
                    Err(e) => {
                        anstream::eprintln!("ERROR: failed to parse json: {} - {}\n", txt, e);
                        err_count += 1;
                        if err_count >= 5 {
                            return -1;
                        }
                    }
                }
            }
            Err(e) => {
                anstream::eprintln!("ERROR: status query failed: {}", e);
                err_count += 1;
                if err_count >= 5 {
                    return -1;
                }
            }
        }
    }
}

/// Rate-limit file for renewal attempts (one per hour)
fn renewal_rate_limit_path() -> std::path::PathBuf {
    let tls_config = ClientTlsConfig::default();
    tls_config
        .cert_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".renewal_last_attempt")
}

fn is_renewal_rate_limited() -> bool {
    let path = renewal_rate_limit_path();
    if !path.exists() {
        return false;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(last_attempt) = content.trim().parse::<i64>() else {
        return false;
    };
    let now = chrono::Utc::now().timestamp();
    // Rate limit: once per hour (3600 seconds)
    now - last_attempt < 3600
}

fn record_renewal_attempt() {
    let path = renewal_rate_limit_path();
    let now = chrono::Utc::now().timestamp();
    let _ = std::fs::write(&path, now.to_string());
}

/// Check if an error from reqwest is a TLS certificate mismatch.
fn is_tls_cert_mismatch_error(err: &reqwest::Error) -> bool {
    let err_str = err.to_string();
    // reqwest/rustls reports cert mismatch with these patterns
    err_str.contains("certificate")
        && (err_str.contains("unknown issuer")
            || err_str.contains("bad signature")
            || err_str.contains("cert")
            || err_str.contains("peer")
            || err_str.contains("verify"))
}

/// Attempt to renew the pinned server certificate from the overlap window renewal endpoint.
///
/// This function:
/// 1. Checks if renewal is rate-limited (once per hour)
/// 2. Connects to the renewal endpoint on port+1 using the old pinned cert
/// 3. Verifies the continuity proof
/// 4. Updates the known_server_keys file with the new cert
///
/// Returns Ok(true) if renewal succeeded, Ok(false) if skipped, Err on failure.
pub async fn try_renew_pinned_server_cert() -> Result<bool, Box<dyn std::error::Error>> {
    if is_renewal_rate_limited() {
        return Ok(false);
    }

    let tls_config = ClientTlsConfig::default();
    if !tls_config.known_server_keys_exist() {
        return Ok(false);
    }

    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let server_port: u16 = port.parse().unwrap_or(29295);
    let renewal_port = server_port + 1;

    // Read old pinned cert for the renewal connection
    let old_pinned_pem = tls_config.read_known_server_keys()?;
    let old_pinned_cert = reqwest::Certificate::from_pem(old_pinned_pem.as_bytes())?;

    // Build a client that trusts only the old pinned cert
    let renew_client = ClientBuilder::new()
        .tls_built_in_root_certs(false)
        .add_root_certificate(old_pinned_cert)
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()?;

    let url = format!("https://{}:{}/rocolatey/trust/renew", ip, renewal_port);

    let resp = match renew_client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            anstream::eprintln!("[INFO] Renewal endpoint not available: {}", e);
            record_renewal_attempt();
            return Ok(false);
        }
    };

    if !resp.status().is_success() {
        anstream::eprintln!(
            "[WARN] Renewal endpoint returned status {}",
            resp.status()
        );
        record_renewal_attempt();
        return Ok(false);
    }

    let renew_response: RocoServerTrustRenewResponse = resp.json().await?;

    // Verify continuity proof
    let old_fingerprint = crate::bootstrap::fingerprint_full(
        &std::fs::read(&tls_config.known_server_keys_path)?,
    )?;

    if renew_response.previous_fingerprint != old_fingerprint {
        anstream::eprintln!(
            "[WARN] Renewal response previous fingerprint does not match our pinned cert. Possible MITM."
        );
        record_renewal_attempt();
        return Ok(false);
    }

    let expected_proof = crate::server::authorization::build_continuity_proof(
        &renew_response.previous_fingerprint,
        &renew_response.new_fingerprint,
    );

    if expected_proof != renew_response.continuity_proof {
        anstream::eprintln!(
            "[WARN] Renewal continuity proof mismatch. Possible MITM."
        );
        record_renewal_attempt();
        return Ok(false);
    }

    // Update pinned server cert
    let new_cert_pem = renew_response.new_server_cert_pem.as_bytes();
    crate::bootstrap::write_atomic_with_backup(
        &tls_config.known_server_keys_path,
        new_cert_pem,
    )?;

    let new_short = crate::server::authorization::short_fingerprint(&renew_response.new_fingerprint);
    let old_short = crate::server::authorization::short_fingerprint(&old_fingerprint);
    anstream::println!(
        "[OK] Server certificate renewed: {} -> {} (overlap window auto-renewal)",
        old_short, new_short
    );

    record_renewal_attempt();
    Ok(true)
}

/// Check if a reqwest error indicates a TLS certificate issue that might
/// trigger auto-renewal. If so, attempt renewal and return true if successful.
pub async fn handle_tls_cert_error(err: &reqwest::Error) -> bool {
    if !is_tls_cert_mismatch_error(err) {
        return false;
    }

    anstream::eprintln!(
        "[INFO] TLS certificate mismatch detected. Attempting auto-renewal via overlap window..."
    );

    match try_renew_pinned_server_cert().await {
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            anstream::eprintln!("[WARN] Auto-renewal attempt failed: {}", e);
            false
        }
    }
}
