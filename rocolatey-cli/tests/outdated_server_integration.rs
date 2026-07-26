use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SERVER_BINARY_ONCE: OnceLock<PathBuf> = OnceLock::new();

fn has_ansi_escape(text: &str) -> bool {
    text.contains('\x1b')
}

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

fn prepare_fake_chocolatey_home() -> PathBuf {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let template_home = repo_root.join("test/fake_choco_home");
    let fake_repo = repo_root.join("test/fake_repo");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();
    let home = std::env::temp_dir().join(format!(
        "rocolatey-phase4-test-{}-{}",
        std::process::id(),
        unique
    ));

    copy_dir_recursive(&template_home, &home);
    fs::create_dir_all(home.join("lib-bad")).expect("create empty lib-bad directory");

    let config_path = home.join("config/chocolatey.config");
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

    home
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

fn run_roco(args: &[&str], chocolatey_home: &Path, port: Option<u16>) -> Output {
    run_roco_with_env(args, chocolatey_home, port, &[])
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

fn wait_for_remote_tls_deny(
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

        let stderr = stderr(&output);
        if stderr.contains("NotEnrolledClient") || stderr.contains("empty enrollment mode") {
            return output;
        }

        if Instant::now() > deadline {
            panic!(
                "server did not reach the expected deny state; last stderr was: {}",
                stderr
            );
        }

        thread::sleep(Duration::from_millis(250));
    }
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

fn read_client_certificate_fingerprint(client_cert_path: &Path) -> String {
    let cert_pem = fs::read(client_cert_path).expect("read client cert");
    rocolatey_lib::bootstrap::fingerprint_full(&cert_pem).expect("fingerprint client cert")
}

fn read_server_certificate_pem(server_cert_path: &Path) -> Vec<u8> {
    fs::read(server_cert_path).expect("read server cert")
}

fn prepare_tls_test_roots() -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();

    let workspace = std::env::temp_dir().join(format!(
        "rocolatey-phase7-tls-{}-{}",
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

    (workspace, client_home, server_root, fake_home, fake_repo)
}

#[cfg(windows)]
fn tls_test_env<'a>(client_config_root: &'a str, server_trust: &'a str) -> [(&'static str, &'a str); 2] {
    [
        ("APPDATA", client_config_root),
        ("ROCO_SERVER_TRUST_DIR", server_trust),
    ]
}

#[cfg(not(windows))]
fn tls_test_env<'a>(client_config_root: &'a str, server_trust: &'a str) -> [(&'static str, &'a str); 2] {
    [
        ("XDG_CONFIG_HOME", client_config_root),
        ("ROCO_SERVER_TRUST_DIR", server_trust),
    ]
}

fn assert_local_and_server_match(chocolatey_home: &Path, args: &[&str], expect_ansi: bool) {
    // Set up TLS for server (required by secure-only transport model)
    let (_workspace, client_home, server_root, _fake_home, _) = prepare_tls_test_roots();
    let client_config_root = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = tls_test_env(&client_config_root, &server_trust);

    // Generate TLS certificates
    let gen_cert_output = run_roco_with_env(
        &["server", "--gen-cert"],
        chocolatey_home,
        None,
        &shared_env,
    );
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stderr={:?}",
        stderr(&gen_cert_output)
    );

    // Set up client trust for server certificate
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

    // Enroll client automatically
    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    let port = reserve_free_port();
    let mut server = start_server_with_env(
        port,
        chocolatey_home,
        &shared_env,
    );

    // Wait for TLS server to be ready
    let ready_output = wait_for_remote_tls_success(
        chocolatey_home,
        port,
        &shared_env,
    );
    assert!(
        ready_output.status.success(),
        "server did not become ready with TLS; stderr: {}",
        stderr(&ready_output)
    );

    // Run local command (no server, no TLS needed)
    let local = run_roco(args, chocolatey_home, None);

    // Run remote command (against TLS server with enrolled client)
    let remote = run_roco_with_env(
        args,
        chocolatey_home,
        Some(port),
        &shared_env,
    );

    let local_stdout = stdout(&local);
    let remote_stdout = stdout(&remote);
    let local_stderr = stderr(&local);
    let remote_stderr = stderr(&remote);

    assert!(local.status.success(), "local command failed: {:?}", local);
    assert!(
        remote.status.success(),
        "server command failed: stderr={:?}",
        remote_stderr
    );
    assert_eq!(
        local_stdout,
        remote_stdout,
        "local and server output diverged\nlocal stderr: {:?}\nremote stderr: {:?}",
        local_stderr,
        remote_stderr
    );
    assert_eq!(has_ansi_escape(&local_stdout), expect_ansi);
    assert_eq!(has_ansi_escape(&remote_stdout), expect_ansi);

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn list_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(&chocolatey_home, &["--color", "always", "list"], true);
}

#[test]
fn list_dependency_tree_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(
        &chocolatey_home,
        &["--color", "always", "list", "--dependency-tree"],
        true,
    );
}

#[test]
fn bad_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(&chocolatey_home, &["--color", "always", "bad"], true);
}

#[test]
fn source_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(&chocolatey_home, &["--color", "always", "source"], true);
}

#[test]
fn outdated_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(
        &chocolatey_home,
        &["--color", "always", "outdated", "Firefox"],
        true,
    );
}

#[test]
fn search_server_backed_output_matches_local_renderer() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    assert_local_and_server_match(
        &chocolatey_home,
        &["--color", "always", "search", "Firefox"],
        false,
    );
}

#[test]
fn server_json_and_limitoutput_remain_ansi_free() {
    let chocolatey_home = prepare_fake_chocolatey_home();
    
    // Set up TLS for server (required by secure-only transport model)
    let (_workspace, client_home, server_root, _fake_home, _) = prepare_tls_test_roots();
    let client_config_root = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = tls_test_env(&client_config_root, &server_trust);

    // Generate TLS certificates
    let gen_cert_output = run_roco_with_env(
        &["server", "--gen-cert"],
        &chocolatey_home,
        None,
        &shared_env,
    );
    assert!(gen_cert_output.status.success(), "certificate generation failed");

    // Set up client trust for server certificate
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

    // Enroll client automatically
    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    let port = reserve_free_port();
    let mut server = start_server_with_env(
        port,
        &chocolatey_home,
        &shared_env,
    );

    // Wait for TLS server to be ready
    let ready_output = wait_for_remote_tls_success(
        &chocolatey_home,
        port,
        &shared_env,
    );
    assert!(ready_output.status.success(), "server did not become ready with TLS");

    let json_output = run_roco_with_env(
        &["--color", "always", "list", "--json"],
        &chocolatey_home,
        Some(port),
        &shared_env,
    );
    let limit_output = run_roco_with_env(
        &["--color", "always", "outdated", "Firefox", "-r"],
        &chocolatey_home,
        Some(port),
        &shared_env,
    );

    let json_stdout = stdout(&json_output);
    let limit_stdout = stdout(&limit_output);

    assert!(json_output.status.success(), "json command failed: {:?}", json_output);
    assert!(limit_output.status.success(), "limitoutput command failed: {:?}", limit_output);
    assert!(!has_ansi_escape(&json_stdout), "json output must stay plain text");
    assert!(!has_ansi_escape(&limit_stdout), "limitoutput must stay plain text");
    assert!(json_stdout.contains("\"schema_version\":1"));

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn live_tls_source_request_denies_then_succeeds_after_enrollment() {
    let (_workspace, client_home, server_root, fake_home, _) = prepare_tls_test_roots();

    let client_config_root = client_home.to_string_lossy().to_string();
    let server_trust = server_root.to_string_lossy().to_string();
    let shared_env = tls_test_env(&client_config_root, &server_trust);

    let gen_cert_output = run_roco_with_env(
        &["server", "--gen-cert"],
        &fake_home,
        None,
        &shared_env,
    );
    assert!(
        gen_cert_output.status.success(),
        "certificate generation failed: stdout={:?} stderr={:?}",
        stdout(&gen_cert_output),
        stderr(&gen_cert_output)
    );

    let client_cfg = rocolatey_lib::server::ClientTlsConfig::new(
        client_home
            .join("rocolatey/client/client.crt.pem"),
        client_home
            .join("rocolatey/client/client.key.pem"),
        client_home
            .join("rocolatey/client/known_server_keys"),
    );
    let server_cfg = rocolatey_lib::server::ServerTlsConfig::new(
        server_root.join("server.crt.pem"),
        server_root.join("server.key.pem"),
        server_root.join("authorized_keys"),
    );
    let server_cert = read_server_certificate_pem(&server_cfg.cert_path);
    fs::write(&client_cfg.known_server_keys_path, server_cert).expect("write pinned server cert");

    let port = reserve_free_port();
    let mut server = start_server_with_env(
        port,
        &fake_home,
        &shared_env,
    );

    let deny_output = wait_for_remote_tls_deny(
        &fake_home,
        port,
        &shared_env,
    );
    let deny_stderr = stderr(&deny_output);
    assert!(deny_stderr.contains("NotEnrolledClient") || deny_stderr.contains("empty enrollment mode"));

    let client_fingerprint = read_client_certificate_fingerprint(&client_cfg.cert_path);
    fs::write(&server_cfg.authorized_keys_path, format!("{}\n", client_fingerprint))
        .expect("write enrolled client fingerprint");

    let success_output = wait_for_remote_tls_success(
        &fake_home,
        port,
        &shared_env,
    );
    let success_stdout = stdout(&success_output);
    assert!(success_output.status.success(), "source request should succeed after enrollment");
    assert!(success_stdout.contains("schema_version") || success_stdout.contains("local-dev"));

    let _ = server.kill();
    let _ = server.wait();
}
