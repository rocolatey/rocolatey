use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SERVER_BINARY_ONCE: OnceLock<PathBuf> = OnceLock::new();

fn reserve_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to reserve an ephemeral port")
        .local_addr()
        .expect("failed to read reserved local address")
        .port()
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create destination directory");
    for entry in fs::read_dir(src).expect("read source directory") {
        let entry = entry.expect("read directory entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let metadata = entry.metadata().expect("read directory metadata");
        if metadata.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).expect("copy file");
        }
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout must be valid UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr must be valid UTF-8")
}

fn run_roco_with_env(
    args: &[&str],
    chocolatey_home: &Path,
    port: Option<u16>,
    extra_env: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roco"));
    cmd.env("ChocolateyInstall", chocolatey_home);
    if let Some(port) = port {
        cmd.env("ROCO_SERVER_IP", "127.0.0.1")
            .env("ROCO_SERVER_PORT", port.to_string());
    }
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    cmd.args(args);
    cmd.output().expect("run roco command")
}

fn resolve_server_binary_path() -> PathBuf {
    SERVER_BINARY_ONCE
        .get_or_init(|| {
            let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

            let server_binary = {
                #[cfg(windows)]
                {
                    repo_root.join("target/debug/rocolatey-server.exe")
                }
                #[cfg(not(windows))]
                {
                    repo_root.join("target/debug/rocolatey-server")
                }
            };

            if !server_binary.exists() {
                let status = Command::new("cargo")
                    .current_dir(&repo_root)
                    .args(["build", "-q", "-p", "rocolatey-server"])
                    .status()
                    .expect("failed to prebuild rocolatey-server binary");
                assert!(status.success(), "failed to build rocolatey-server binary");
            }

            assert!(
                server_binary.exists(),
                "rocolatey-server binary not found at {}",
                server_binary.display()
            );

            server_binary
        })
        .clone()
}

fn start_server_with_env(port: u16, chocolatey_home: &Path, extra_env: &[(&str, &str)]) -> Child {
    let mut command = Command::new(resolve_server_binary_path());
    command.env("ChocolateyInstall", chocolatey_home);
    for (key, value) in extra_env {
        command.env(key, value);
    }

    command
        .args(["--address", "127.0.0.1", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start rocolatey-server binary")
}

fn prepare_tls_test_roots() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();

    let workspace = std::env::temp_dir().join(format!(
        "rocolatey-ssl-full-stack-{}-{}",
        std::process::id(),
        unique
    ));
    let client_home = workspace.join("client-home");
    let server_root = workspace.join("server-root");
    let fake_home = workspace.join("fake-choco-home");
    let fake_repo = repo_root.join("test/fake_repo");

    fs::create_dir_all(&client_home).expect("create client root");
    fs::create_dir_all(&server_root).expect("create server root");
    copy_dir_recursive(&repo_root.join("test/fake_choco_home"), &fake_home);
    fs::create_dir_all(fake_home.join("lib-bad")).expect("create empty lib-bad directory");

    let config_path = fake_home.join("config/chocolatey.config");
    let config = fs::read_to_string(&config_path).expect("read fake chocolatey.config");
    let fake_repo_value = fake_repo.to_string_lossy().replace('&', "&amp;");
    let config = config
        .replace(
            r#"<source id="chocolatey" value="https://chocolatey.org/api/v2" disabled="false""#,
            r#"<source id="chocolatey" value="https://chocolatey.org/api/v2" disabled="true""#,
        )
        .replace(
            r#"<source id="nuget.org" value="https://api.nuget.org/v3/index.json" disabled="false""#,
            r#"<source id="nuget.org" value="https://api.nuget.org/v3/index.json" disabled="true""#,
        )
        .replace(
            r#"<source id="local-dev" value="c:/local-pkgs" disabled="true""#,
            &format!(
                r#"<source id="local-dev" value="{}" disabled="false""#,
                fake_repo_value
            ),
        );
    fs::write(&config_path, config).expect("write fake chocolatey.config");

    (workspace, client_home, server_root, fake_home)
}

fn wait_for_remote_tls_success(
    chocolatey_home: &Path,
    port: u16,
    extra_env: &[(&str, &str)],
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let output = run_roco_with_env(
            &["source", "--json"],
            chocolatey_home,
            Some(port),
            extra_env,
        );

        if output.status.success() {
            let output_stdout = stdout(&output);
            let output_stderr = stderr(&output);
            let has_ready_payload = output_stdout.contains("\"schema_version\":1")
                || output_stdout.contains("\"schema_version\": 1")
                || output_stdout.contains("local-dev");
            let has_transport_error = output_stderr.contains("Request to ")
                || output_stderr.contains("Error fetching")
                || output_stderr.contains("Connection refused")
                || output_stderr.contains("error trying to connect");

            if has_ready_payload && !has_transport_error {
                return output;
            }
        }

        if Instant::now() > deadline {
            panic!(
                "server did not reach the expected success state; last stderr was: {}",
                stderr(&output)
            );
        }

        thread::sleep(Duration::from_millis(250));
    }
}

fn wait_for_remote_request_without_refused(
    args: &[&str],
    chocolatey_home: &Path,
    port: u16,
    extra_env: &[(&str, &str)],
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let output = run_roco_with_env(args, chocolatey_home, Some(port), extra_env);
        let output_stderr = stderr(&output);

        if !output_stderr.contains("Connection refused")
            && !output_stderr.contains("tcp connect error")
        {
            return output;
        }

        if Instant::now() > deadline {
            panic!(
                "server never progressed past connection-refused startup window; last stderr was: {}",
                output_stderr
            );
        }

        thread::sleep(Duration::from_millis(250));
    }
}

fn read_client_certificate_fingerprint(client_cert_path: &Path) -> String {
    let cert_pem = fs::read(client_cert_path).expect("read client cert");
    rocolatey_lib::bootstrap::fingerprint_full(&cert_pem).expect("fingerprint client cert")
}

fn read_server_certificate_pem(server_cert_path: &Path) -> Vec<u8> {
    fs::read(server_cert_path).expect("read server cert")
}

fn make_server_cert_with_window(not_before: time::OffsetDateTime, not_after: time::OffsetDateTime) -> Vec<u8> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
    let mut params = CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, "rocolatey-server");
    params.not_before = not_before;
    params.not_after = not_after;

    Certificate::from_params(params)
        .expect("build cert params")
        .serialize_pem()
        .expect("serialize cert")
        .into_bytes()
}

fn wait_for_server_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if Instant::now() > deadline {
                    return None;
                }
            }
            Err(_) => return None,
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn start_plaintext_probe_server() -> (u16, Arc<Mutex<Vec<u8>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind plaintext probe server");
    let port = listener
        .local_addr()
        .expect("read plaintext probe local address")
        .port();
    listener
        .set_nonblocking(true)
        .expect("set plaintext probe nonblocking");

    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_thread = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .expect("set probe read timeout");
                    let mut buf = [0u8; 64];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n > 0 {
                        let mut guard = captured_for_thread.lock().expect("lock probe capture");
                        guard.extend_from_slice(&buf[..n]);
                    }

                    // Return a basic HTTP response in case a plaintext HTTP request is sent.
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
                    );
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }
    });

    (port, captured, handle)
}

#[test]
fn ssl_full_stack_valid_pinned_server_certificate_succeeds() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let server_cert = read_server_certificate_pem(&server_cfg.cert_path);
    fs::write(&client_cfg.known_server_keys_path, server_cert).expect("write pinned server cert");

    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);

    let success_output = wait_for_remote_tls_success(&fake_home, port, &shared_env);
    let success_stdout = stdout(&success_output);
    let success_stderr = stderr(&success_output);

    assert!(
        success_output.status.success(),
        "expected pinned TLS path to succeed; stdout={:?} stderr={:?}",
        success_stdout,
        success_stderr
    );
    assert!(
        success_stdout.contains("schema_version") || success_stdout.contains("local-dev"),
        "expected source payload after successful pinned TLS request, got: {:?}",
        success_stdout
    );

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn ssl_full_stack_missing_pinned_server_certificate_fails_with_guidance() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);

    let output = wait_for_remote_request_without_refused(
        &["source", "--json"],
        &fake_home,
        port,
        &shared_env,
    );
    let output_stdout = stdout(&output);
    let output_stderr = stderr(&output);

    assert!(
        output_stdout.trim().is_empty(),
        "missing pin should not produce a successful payload; stdout={:?} stderr={:?}",
        output_stdout,
        output_stderr
    );
    assert!(
        output_stderr.contains("Pinned server certificate not found")
            || output_stderr.contains("known_server_keys"),
        "expected pinning setup guidance in stderr, got: {:?}",
        output_stderr
    );

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn ssl_full_stack_unpinned_server_certificate_is_rejected() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    // Pin a different self-signed certificate than the one the server actually presents.
    let wrong_pinned_cert = make_server_cert_with_window(
        time::OffsetDateTime::now_utc() - time::Duration::days(1),
        time::OffsetDateTime::now_utc() + time::Duration::days(30),
    );
    fs::write(&client_cfg.known_server_keys_path, wrong_pinned_cert)
        .expect("write mismatched pinned server cert");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);

    let output = wait_for_remote_request_without_refused(
        &["source", "--json"],
        &fake_home,
        port,
        &shared_env,
    );
    let output_stdout = stdout(&output);
    let output_stderr = stderr(&output);

    assert!(
        output_stdout.trim().is_empty(),
        "mismatched pin must not produce a successful payload; stdout={:?} stderr={:?}",
        output_stdout,
        output_stderr
    );
    assert!(
        output_stderr.contains("certificate")
            || output_stderr.contains("tls")
            || output_stderr.contains("invalid peer certificate")
            || output_stderr.contains("UnknownIssuer"),
        "expected TLS certificate verification failure for mismatched pin; stderr={:?}",
        output_stderr
    );

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn ssl_full_stack_protected_route_never_targets_http_plaintext() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    // Client certs must exist and a pinned cert must be present so the client attempts a request.
    let server_cert = read_server_certificate_pem(&server_cfg.cert_path);
    fs::write(&client_cfg.known_server_keys_path, server_cert).expect("write pinned server cert");

    let (probe_port, captured, handle) = start_plaintext_probe_server();
    let output = run_roco_with_env(&["source", "--json"], &fake_home, Some(probe_port), &shared_env);
    let output_stdout = stdout(&output);
    let output_stderr = stderr(&output);

    handle.join().expect("join plaintext probe server");

    let captured_bytes = captured.lock().expect("lock probe capture").clone();
    assert!(
        !captured_bytes.is_empty(),
        "expected a connection attempt to plaintext probe; stdout={:?} stderr={:?}",
        output_stdout,
        output_stderr
    );

    assert!(
        output_stdout.trim().is_empty(),
        "plaintext probe should never produce protected route payload; stdout={:?} stderr={:?}",
        output_stdout,
        output_stderr
    );
    assert!(
        output_stderr.contains("Request to ")
            || output_stderr.contains("error trying to connect")
            || output_stderr.contains("tls")
            || output_stderr.contains("Error fetching"),
        "expected HTTPS transport failure against plaintext probe; got stderr={:?}",
        output_stderr
    );

    assert!(
        !captured_bytes.starts_with(b"GET ")
            && !captured_bytes.starts_with(b"POST ")
            && !captured_bytes.starts_with(b"PUT ")
            && !captured_bytes.starts_with(b"DELETE ")
            && !captured_bytes.starts_with(b"PATCH "),
        "protected route attempted plaintext HTTP request bytes: {:?}",
        captured_bytes
    );
}

#[test]
fn ssl_local_bootstrap_first_run_completes_and_allows_server_access() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let bootstrap_output = run_roco_with_env(
        &["server", "--bootstrap-local-trust"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(
        bootstrap_output.status.success(),
        "bootstrap-local-trust must succeed on first run; stdout={:?} stderr={:?}",
        stdout(&bootstrap_output),
        stderr(&bootstrap_output)
    );

    let out = stdout(&bootstrap_output);
    assert!(
        out.contains("enrolled") || out.contains("OK"),
        "expected enrollment confirmation output; got: {:?}",
        out
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    assert!(client_cfg.cert_path.exists(), "client cert must exist after bootstrap");
    assert!(client_cfg.key_path.exists(), "client key must exist after bootstrap");
    assert!(client_cfg.known_server_keys_path.exists(), "pinned server cert must exist after bootstrap");
    assert!(server_cfg.authorized_keys_path.exists(), "authorized_keys must exist after bootstrap");

    let client_cert = fs::read(&client_cfg.cert_path).expect("read client cert");
    let client_fingerprint =
        rocolatey_lib::bootstrap::fingerprint_full(&client_cert).expect("fingerprint");
    let authorized =
        fs::read_to_string(&server_cfg.authorized_keys_path).expect("read authorized_keys");
    assert!(
        authorized.contains(&client_fingerprint),
        "local client fingerprint must appear in authorized_keys"
    );

    let pinned =
        fs::read_to_string(&client_cfg.known_server_keys_path).expect("read known_server_keys");
    assert!(
        pinned.contains("BEGIN CERTIFICATE"),
        "known_server_keys must hold server PEM"
    );

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);
    let success = wait_for_remote_tls_success(&fake_home, port, &shared_env);
    let success_stdout = stdout(&success);
    assert!(
        success.status.success(),
        "server should accept bootstrapped local client; stdout={:?} stderr={:?}",
        success_stdout,
        stderr(&success)
    );
    assert!(
        success_stdout.contains("schema_version") || success_stdout.contains("local-dev"),
        "expected valid server response after local bootstrap"
    );
    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn ssl_empty_enrollment_emergency_override_is_fingerprint_scoped_and_one_shot() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let server_cert = read_server_certificate_pem(&server_cfg.cert_path);
    fs::write(&client_cfg.known_server_keys_path, server_cert).expect("write pinned server cert");

    // Keep authorized_keys empty to force the emergency override path.
    fs::write(&server_cfg.authorized_keys_path, "").expect("write empty authorized_keys");

    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    let override_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
        (
            rocolatey_lib::server::authorization::EMERGENCY_TRUST_OVERRIDE_ENV,
            client_fingerprint.as_str(),
        ),
    ];

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &override_env);

    let first = wait_for_remote_tls_success(&fake_home, port, &override_env);
    assert!(
        first.status.success(),
        "first override-backed request must succeed: stdout={:?} stderr={:?}",
        stdout(&first),
        stderr(&first)
    );

    let second = run_roco_with_env(&["source", "--json"], &fake_home, Some(port), &override_env);
    let second_stdout = stdout(&second);
    let second_stderr = stderr(&second);
    assert!(
        second_stdout.trim().is_empty(),
        "second request must not return a payload after one-shot override use: stdout={:?} stderr={:?}",
        second_stdout,
        second_stderr
    );
    assert!(
        second_stderr.contains("NotEnrolledClient") || second_stderr.contains("not enrolled"),
        "second request should fail closed as not enrolled; stderr={:?}",
        second_stderr
    );

    let audit_log = server_root.join("audit.log");
    let audit_content = fs::read_to_string(&audit_log).expect("read audit log");
    assert!(
        audit_content.contains("emergency_override"),
        "audit log must contain emergency override event; got: {:?}",
        audit_content
    );
    assert!(
        audit_content.contains(&client_fingerprint),
        "audit log must include full override fingerprint; got: {:?}",
        audit_content
    );

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn ssl_local_bootstrap_is_idempotent() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let first = run_roco_with_env(
        &["server", "--bootstrap-local-trust"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(first.status.success(), "first bootstrap must succeed");

    let second = run_roco_with_env(
        &["server", "--bootstrap-local-trust"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(second.status.success(), "second bootstrap must also succeed");

    let second_out = stdout(&second);
    assert!(
        second_out.contains("already enrolled"),
        "second run should report fingerprint already enrolled; got: {:?}",
        second_out
    );

    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );
    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let client_cert = fs::read(&client_cfg.cert_path).expect("read client cert");
    let client_fingerprint =
        rocolatey_lib::bootstrap::fingerprint_full(&client_cert).expect("fingerprint");
    let authorized =
        fs::read_to_string(&server_cfg.authorized_keys_path).expect("read authorized_keys");
    let occurrences = authorized
        .lines()
        .filter(|l| l.trim() == client_fingerprint)
        .count();
    assert_eq!(
        occurrences, 1,
        "fingerprint must appear exactly once, got {} occurrences",
        occurrences
    );
}

#[test]
fn ssl_local_bootstrap_preserves_existing_authorized_keys_entries() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output =
        run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(gen_cert_output.status.success(), "cert generation must succeed");

    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );
    let external_fp = "externalfingerprint0000000000000000000000000A";
    fs::write(
        &server_cfg.authorized_keys_path,
        format!("# external client\n{}\n", external_fp),
    )
    .expect("write external fingerprint");

    let bootstrap_output = run_roco_with_env(
        &["server", "--bootstrap-local-trust"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(
        bootstrap_output.status.success(),
        "bootstrap must succeed with existing entries; stdout={:?} stderr={:?}",
        stdout(&bootstrap_output),
        stderr(&bootstrap_output)
    );

    let authorized =
        fs::read_to_string(&server_cfg.authorized_keys_path).expect("read authorized_keys");
    assert!(
        authorized.contains(external_fp),
        "external fingerprint must be preserved after local bootstrap; got: {:?}",
        authorized
    );
}

#[test]
fn ssl_server_startup_fails_closed_on_expired_certificate() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(gen_cert_output.status.success(), "cert generation must succeed");

    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let expired_cert = make_server_cert_with_window(
        time::OffsetDateTime::now_utc() - time::Duration::days(30),
        time::OffsetDateTime::now_utc() - time::Duration::days(1),
    );
    fs::write(&server_cfg.cert_path, expired_cert).expect("write expired cert");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);
    let status = wait_for_server_exit(&mut server, Duration::from_secs(30));

    assert!(status.is_some(), "server should exit fail-closed on expired cert");
    assert!(!status.unwrap().success(), "expired cert must cause non-zero exit");
}

#[test]
fn ssl_server_startup_fails_closed_on_clock_skew_certificate() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let gen_cert_output = run_roco_with_env(&["server", "--gen-cert"], &fake_home, None, &shared_env);
    assert!(gen_cert_output.status.success(), "cert generation must succeed");

    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );

    let future_start = time::OffsetDateTime::now_utc() + time::Duration::hours(2);
    let skewed_cert = make_server_cert_with_window(
        future_start,
        future_start + time::Duration::days(365),
    );
    fs::write(&server_cfg.cert_path, skewed_cert).expect("write skewed cert");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);
    let status = wait_for_server_exit(&mut server, Duration::from_secs(30));

    assert!(status.is_some(), "server should exit fail-closed on time-skew cert");
    assert!(!status.unwrap().success(), "time-skew cert must cause non-zero exit");
}

#[tokio::test]
async fn ssl_deny_contract_schema_version_matches_client_upgrade_assumption() {
    let (_workspace, client_home, server_root, fake_home) = prepare_tls_test_roots();

    let client_xdg = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = [
        ("XDG_CONFIG_HOME", client_xdg.as_str()),
        ("ROCO_SERVER_TRUST_DIR", server_trust.as_str()),
    ];

    let bootstrap_output = run_roco_with_env(
        &["server", "--bootstrap-local-trust"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(bootstrap_output.status.success(), "bootstrap-local-trust must succeed");

    let port = reserve_free_port();
    let mut server = start_server_with_env(port, &fake_home, &shared_env);

    let ready = wait_for_remote_tls_success(&fake_home, port, &shared_env);
    assert!(ready.status.success(), "server should be ready before schema check");

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home.join("rocolatey/client/client.crt.pem"),
        client_home.join("rocolatey/client/client.key.pem"),
        client_home.join("rocolatey/client/known_server_keys"),
    );
    let pinned = fs::read_to_string(&client_cfg.known_server_keys_path).expect("read pinned cert");
    let cert = reqwest::Certificate::from_pem(pinned.as_bytes()).expect("parse pinned cert");
    let http = reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(cert)
        .build()
        .expect("build https client");

    let deny_resp = http
        .get(format!("https://127.0.0.1:{}/rocolatey/choco/status/not-a-uuid", port))
        .send()
        .await
        .expect("send deny request");
    assert_eq!(deny_resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let deny: rocolatey_lib::server::RocoServerDenyResponse = deny_resp
        .json()
        .await
        .expect("decode deny schema payload");
    assert_eq!(deny.schema_version, rocolatey_lib::server::ROCO_SERVER_SCHEMA_VERSION);
    assert_eq!(deny.code, rocolatey_lib::server::RocoServerDenyCode::InvalidRequest);

    let _ = server.kill();
    let _ = server.wait();
}
