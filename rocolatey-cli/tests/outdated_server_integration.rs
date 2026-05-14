use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

fn wait_for_http_ok(socket_addr: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        if Instant::now() > deadline {
            panic!("server did not become ready at {}", socket_addr);
        }

        match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(250)) {
            Ok(mut stream) => {
                let request = format!(
                    "GET /rocolatey/source HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                    socket_addr
                );
                if stream.write_all(request.as_bytes()).is_ok() {
                    let mut response = String::new();
                    if stream.read_to_string(&mut response).is_ok()
                        && response.contains("local-dev")
                    {
                        return;
                    }
                }
            }
            Err(_) => {}
        }

        thread::sleep(Duration::from_millis(250));
    }
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

fn server_binary_path() -> PathBuf {
    // Derive the path to the pre-compiled rocolatey-server binary.
    // Using the binary directly (instead of `cargo run`) avoids contention on
    // Cargo's global file-lock when several test threads start servers in parallel.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("target"))
        .join("debug");

    let binary_name = if cfg!(windows) {
        "rocolatey-server.exe"
    } else {
        "rocolatey-server"
    };

    target_dir.join(binary_name)
}

fn start_server(port: u16, chocolatey_home: &Path) -> Child {
    Command::new(server_binary_path())
        .env("ChocolateyInstall", chocolatey_home)
        .args(["--address", "127.0.0.1", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start rocolatey-server")
}

fn run_roco(args: &[&str], chocolatey_home: &Path, port: Option<u16>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roco"));
    cmd.env("ChocolateyInstall", chocolatey_home).args(args);
    if let Some(port) = port {
        cmd.env("ROCO_SERVER_IP", "127.0.0.1")
            .env("ROCO_SERVER_PORT", port.to_string());
    }
    cmd.output().expect("run roco command")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout must be valid UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr must be valid UTF-8")
}

fn assert_local_and_server_match(chocolatey_home: &Path, args: &[&str], expect_ansi: bool) {
    let port = reserve_free_port();
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut server = start_server(port, chocolatey_home);
    wait_for_http_ok(socket_addr);

    let local = run_roco(args, chocolatey_home, None);
    let remote = run_roco(args, chocolatey_home, Some(port));

    let local_stdout = stdout(&local);
    let remote_stdout = stdout(&remote);
    let local_stderr = stderr(&local);
    let remote_stderr = stderr(&remote);

    assert!(local.status.success(), "local command failed: {:?}", local);
    assert!(remote.status.success(), "server command failed: {:?}", remote);
    assert_eq!(
        local_stdout,
        remote_stdout,
        "local and server output diverged\nlocal stderr: {:?}\nremote stderr: {:?}",
        local_stderr,
        remote_stderr,
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
    let port = reserve_free_port();
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut server = start_server(port, &chocolatey_home);
    wait_for_http_ok(socket_addr);

    let json_output = run_roco(
        &["--color", "always", "list", "--json"],
        &chocolatey_home,
        Some(port),
    );
    let limit_output = run_roco(
        &["--color", "always", "outdated", "Firefox", "-r"],
        &chocolatey_home,
        Some(port),
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
