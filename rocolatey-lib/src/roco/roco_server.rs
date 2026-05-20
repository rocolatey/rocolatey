use reqwest::header::CONTENT_TYPE;
use serde_json;
use std::time::Duration;

use crate::server::{JobStatus, RocoServerChocoCommandRequest, ClientTlsConfig};
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
            anstream::eprintln!("Request to {} failed: {}", url, e);
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
            anstream::eprintln!("Request to {} failed: {}", url, e);
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
            anstream::eprintln!("Request to {} failed: {}", url, e);
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
            anstream::eprintln!("Request to {} failed: {}", url, e);
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
            anstream::eprintln!("request failed: {}", e);
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
