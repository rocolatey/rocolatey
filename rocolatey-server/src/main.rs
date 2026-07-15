use clap::{Arg, Command};
mod authorized_keys_runtime;
mod serverimpl;
mod tls;

use std::env;
use std::error::Error;
use std::fs::OpenOptions;
use std::path::PathBuf;

#[cfg(windows)]
static SERVICE_BIND_ADDR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
#[cfg(windows)]
static SERVICE_BIND_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
#[cfg(windows)]
const WINDOWS_SERVICE_NAME: &str = "Rocolatey-Server";

/// Redirect stdout and stderr into a logfile under the OS temporary directory.
fn init_log_redirect() {
    // determine temp dir: on Windows use %TEMP%/%TMP%, on Unix use TMPDIR or /tmp
    let mut temp = env::var_os("TEMP").or_else(|| env::var_os("TMP"));
    if temp.is_none() {
        temp = env::var_os("TMPDIR");
    }
    let temp = temp
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_path = temp.join("rocolatey-server.log");

    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            unsafe {
                libc::dup2(fd, libc::STDOUT_FILENO);
                libc::dup2(fd, libc::STDERR_FILENO);
            }
            // keep file alive? after dup2 it's fine to drop file
        }

        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Console::{
                SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
            };

            let handle = file.as_raw_handle();
            unsafe {
                SetStdHandle(STD_OUTPUT_HANDLE, handle);
                SetStdHandle(STD_ERROR_HANDLE, handle);
            }
            // avoid closing the file when `file` is dropped — leak it intentionally so OS keeps handle
            std::mem::forget(file);
        }
    } else {
        // best effort; if opening file fails we silently continue to console
    }
}

fn build_cli() -> Command {
    let default_port = rocolatey_lib::server::ROCO_SERVER_DEFAULT_PORT;

    Command::new("Rocolatey Server")
        .version("0.9.5")
        .author("Manfred Wallner <schusterfredl@mwallner.net>")
        .about("provides web access to rocolatey-lib")
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .help("Sets the port to bind to")
                .value_parser(clap::value_parser!(String))
                .default_value(default_port),
        )
        .arg(
            Arg::new("address")
                .long("address")
                .short('a')
                .help("Sets the address to bind to")
                .value_parser(clap::value_parser!(String))
                .default_value("127.0.0.1"),
        )
}

async fn run_server(bind_addr: &str, bind_port: u16) {
    println!(" server binds on ip: {}", bind_addr);
    println!(" server binds on port: {}", bind_port);

    // Bootstrap TLS materials if they don't exist (MANDATORY - fail-closed on TLS errors)
    match tls::ensure_server_tls_materials() {
        Ok(_) => {
            println!(" server TLS materials ready");
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::Bootstrap,
                    "Server TLS materials bootstrapped and ready",
                ),
            );
        }
        Err(e) => {
            eprintln!("Failed to bootstrap TLS materials: {}", e);
            eprintln!("Secure-only transport requires valid TLS materials. Set ROCO_SERVER_TRUST_DIR to a writable directory.");
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::Bootstrap,
                    format!("FAIL-CLOSED: bootstrap failed: {}", e),
                ),
            );
            std::process::exit(1);
        }
    }

    // Prune audit log at startup to enforce 14-day retention policy
    if let Err(e) = rocolatey_lib::server::audit::prune_audit_log_if_due(
        &rocolatey_lib::server::audit::server_audit_log_path(),
    ) {
        eprintln!("[AUDIT] prune failed (non-fatal): {}", e);
    }

    // Check if authorized_keys is valid (empty file is OK, unreadable/invalid is not)
    let tls_config = rocolatey_lib::server::ServerTlsConfig::default();
    match rocolatey_lib::bootstrap::validate_authorized_keys_file(&tls_config) {
        Ok(entry_count) if entry_count == 0 => {
            println!(" server in EMPTY ENROLLMENT MODE (no authorized clients)");
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::Enrollment,
                    "Server started in empty enrollment mode (0 authorized clients)",
                ),
            );
        }
        Ok(entry_count) => {
            println!(" server has {} authorized client(s)", entry_count);
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::Enrollment,
                    format!("Server started with {} authorized client(s)", entry_count),
                ),
            );
        }
        Err(e) => {
            eprintln!("Failed to validate authorized_keys file: {}", e);
            eprintln!("Path: {}", tls_config.authorized_keys_path.display());
            std::process::exit(1);
        }
    }

    match rocolatey_lib::bootstrap::auto_rotate_server_tls_if_due(&tls_config) {
        Ok(true) => {
            println!(
                " server identity auto-rotated from deterministic schedule (90d + jitter), overlap window active"
            );
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::Rotation,
                    "Server identity auto-rotated per deterministic schedule (90d + jitter). Overlap window active.",
                ),
            );
        }
        Ok(false) => {}
        Err(e) => {
            eprintln!("Fail-closed: automatic rotation check failed: {}", e);
            std::process::exit(1);
        }
    }

    match rocolatey_lib::bootstrap::validate_server_continuity_proof(&tls_config) {
        Ok(Some(proof)) => {
            let old_short = rocolatey_lib::server::authorization::short_fingerprint(
                &proof.previous_identity_fingerprint,
            );
            let new_short = rocolatey_lib::server::authorization::short_fingerprint(
                &proof.new_identity_fingerprint,
            );
            println!(
                " server continuity proof verified: {} -> {}",
                old_short, new_short
            );
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::ContinuityCheck,
                    format!(
                        "Continuity proof verified: {} -> {} (full: {} -> {})",
                        old_short,
                        new_short,
                        proof.previous_identity_fingerprint,
                        proof.new_identity_fingerprint
                    ),
                ),
            );
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("Fail-closed: continuity proof validation failed: {}", e);
            std::process::exit(1);
        }
    }

    match std::fs::read(&tls_config.cert_path)
        .ok()
        .map(|pem| rocolatey_lib::server::authorization::evaluate_certificate_time_now(&pem))
    {
        Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::Valid)) => {}
        Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::Expired)) => {
            eprintln!("Fail-closed: server certificate expired.");
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::ExpiryFailure,
                    "FAIL-CLOSED: server certificate expired at startup. Service cannot start.",
                ),
            );
            std::process::exit(1);
        }
        Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::TimeSkewExceeded)) => {
            eprintln!("Fail-closed: system clock skew exceeds 8 minutes.");
            rocolatey_lib::server::audit::emit_audit_event(
                &rocolatey_lib::server::audit::server_audit_log_path(),
                &rocolatey_lib::server::audit::AuditEvent::new(
                    rocolatey_lib::server::audit::AuditEventKind::ExpiryFailure,
                    "FAIL-CLOSED: system clock skew exceeds 8 minutes at startup. Check NTP synchronization.",
                ),
            );
            std::process::exit(1);
        }
        Some(Err(e)) => {
            eprintln!("Fail-closed: could not validate server certificate time: {}", e);
            std::process::exit(1);
        }
        None => {}
    }

    let auth_runtime = match authorized_keys_runtime::AuthorizedKeysRuntime::from_file(
        &tls_config.authorized_keys_path,
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Failed to initialize authorized_keys runtime from {}: {}",
                tls_config.authorized_keys_path.display(),
                e
            );
            std::process::exit(1);
        }
    };
    auth_runtime.start_watcher();
    serverimpl::install_authorization_runtime(auth_runtime);

    // Check if we're in the overlap window and start renewal server if so
    match rocolatey_lib::bootstrap::check_overlap_window(&tls_config) {
        Ok(Some((prev_cert, _prev_key))) => {
            let state = rocolatey_lib::bootstrap::load_rotation_state(
                &tls_config.rotation_state_path(),
            )
            .unwrap_or(None);
            if let Some(state) = state {
                let renew_port = bind_port + 1;
                let renew_addr: std::net::Ipv4Addr = bind_addr.parse().unwrap();
                let renew_socket = std::net::SocketAddr::new(
                    std::net::IpAddr::V4(renew_addr),
                    renew_port,
                );

                let new_cert_pem = std::fs::read_to_string(&tls_config.cert_path)
                    .unwrap_or_default();
                let prev_fingerprint = rocolatey_lib::server::authorization::fingerprint_from_cert(&prev_cert)
                    .unwrap_or_default();
                let new_fingerprint = state.identity_fingerprint.clone();
                let continuity_proof = state.continuity_proof.clone().unwrap_or_default();
                let issued_at = state.issued_at_utc.to_rfc3339();

                let renewal_filter = serverimpl::create_renewal_filter(
                    new_cert_pem,
                    continuity_proof,
                    prev_fingerprint,
                    new_fingerprint,
                    issued_at,
                );

                let prev_cert_path = tls_config.prev_cert_path();
                let prev_key_path = tls_config.prev_key_path();

                tokio::spawn(async move {
                    println!(
                        " renewal server binding on port {} (overlap window active)",
                        renew_port
                    );
                    rocolatey_lib::server::audit::emit_audit_event(
                        &rocolatey_lib::server::audit::server_audit_log_path(),
                        &rocolatey_lib::server::audit::AuditEvent::new(
                            rocolatey_lib::server::audit::AuditEventKind::Bootstrap,
                            format!(
                                "Renewal server started on port {} for overlap window",
                                renew_port
                            ),
                        ),
                    );
                    warp::serve(renewal_filter)
                        .tls()
                        .cert_path(&prev_cert_path)
                        .key_path(&prev_key_path)
                        .run(renew_socket)
                        .await;
                });
            }
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("[WARN] Failed to check overlap window: {}", e);
        }
    }

    let warp_filter = serverimpl::create_warp_filter();
    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    let socket_addr = std::net::SocketAddr::new(std::net::IpAddr::V4(server_ip), bind_port);

    // Secure-only transport: TLS materials must exist and load successfully.
    if tls::tls_materials_exist(&tls_config.cert_path, &tls_config.key_path) {
        println!(" server using TLS (secure transport)");
        match tls::load_server_tls_config(&tls_config.cert_path, &tls_config.key_path) {
            Ok(_) => {
                warp::serve(warp_filter)
                    .tls()
                    .cert_path(&tls_config.cert_path)
                    .key_path(&tls_config.key_path)
                    .run(socket_addr)
                    .await;
            }
            Err(e) => {
                eprintln!("Failed to load TLS config: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!(
            "Fail-closed: TLS materials not found at {} and {}",
            tls_config.cert_path.display(),
            tls_config.key_path.display()
        );
        std::process::exit(1);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // redirect all stdout/stderr to logfile
    init_log_redirect();

    let matches = build_cli().get_matches();

    let default_port = rocolatey_lib::server::ROCO_SERVER_DEFAULT_PORT;
    let bind_addr: &str = matches
        .get_one::<String>("address")
        .map(String::as_str)
        .unwrap_or("127.0.0.1");
    let bind_port: u16 = matches
        .get_one::<String>("port")
        .map(String::as_str)
        .unwrap_or(default_port)
        .parse()
        .expect("invalid port number");

    // If the user asked to run as service on Windows, dispatch to the service subsystem.
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use windows_service::service_dispatcher;

        // Service entry — the service framework will call our `service_main` function.
        fn service_main(_arguments: Vec<OsString>) {
            use std::sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            };
            use std::time::Duration;
            use windows_service::service::{ServiceControl, ServiceState, ServiceStatus};
            use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();

            let status_handle =
                match service_control_handler::register(WINDOWS_SERVICE_NAME, move |control_event| {
                    match control_event {
                        ServiceControl::Stop => {
                            running_clone.store(false, Ordering::SeqCst);
                            ServiceControlHandlerResult::NoError
                        }
                        _ => ServiceControlHandlerResult::NotImplemented,
                    }
                }) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("service handler registration failed: {:?}", e);
                        return;
                    }
                };

            // update status to running
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: windows_service::service::ServiceType::OWN_PROCESS,
                current_state: ServiceState::Running,
                controls_accepted: windows_service::service::ServiceControlAccept::STOP,
                exit_code: windows_service::service::ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(1),
                process_id: Some(std::process::id()),
            });

            // Build a runtime and run the server until the stop signal is received.
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("failed to build tokio runtime: {:?}", e);
                    return;
                }
            };

            let default_bind_addr = SERVICE_BIND_ADDR
                .get()
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string());
            let default_bind_port = *SERVICE_BIND_PORT.get().unwrap_or(
                &rocolatey_lib::server::ROCO_SERVER_DEFAULT_PORT
                    .parse::<u16>()
                    .expect("invalid default port number"),
            );
            let (use_env_bind_addr, env_bind_addr) = rocolatey_lib::server::get_server_ip();
            let bind_addr = if use_env_bind_addr {
                env_bind_addr
            } else {
                default_bind_addr
            };
            let (use_env_bind_port, env_bind_port) = rocolatey_lib::server::get_server_port();
            let bind_port = if use_env_bind_port {
                env_bind_port
                    .parse::<u16>()
                    .expect("invalid port number")
            } else {
                default_bind_port
            };

            rt.block_on(async move {
                let tls_config = rocolatey_lib::server::ServerTlsConfig::default();

                match rocolatey_lib::bootstrap::validate_authorized_keys_file(&tls_config) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Fail-closed: invalid authorized_keys file: {}", e);
                        return;
                    }
                }

                match rocolatey_lib::bootstrap::auto_rotate_server_tls_if_due(&tls_config) {
                    Ok(true) => {
                        eprintln!(" service identity auto-rotated from deterministic schedule");
                    }
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!("Fail-closed: automatic rotation check failed: {}", e);
                        return;
                    }
                }

                match rocolatey_lib::bootstrap::validate_server_continuity_proof(&tls_config) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Fail-closed: continuity proof validation failed: {}", e);
                        return;
                    }
                }

                match std::fs::read(&tls_config.cert_path)
                    .ok()
                    .map(|pem| rocolatey_lib::server::authorization::evaluate_certificate_time_now(&pem))
                {
                    Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::Valid)) => {}
                    Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::Expired)) => {
                        eprintln!("Fail-closed: server certificate expired.");
                        return;
                    }
                    Some(Ok(rocolatey_lib::server::authorization::CertificateTimeStatus::TimeSkewExceeded)) => {
                        eprintln!("Fail-closed: system clock skew exceeds 8 minutes.");
                        return;
                    }
                    Some(Err(e)) => {
                        eprintln!("Fail-closed: could not validate server certificate time: {}", e);
                        return;
                    }
                    None => {}
                }

                if let Ok(auth_runtime) =
                    authorized_keys_runtime::AuthorizedKeysRuntime::from_file(&tls_config.authorized_keys_path)
                {
                    auth_runtime.start_watcher();
                    serverimpl::install_authorization_runtime(auth_runtime);
                } else {
                    eprintln!(
                        "[HIGH] service startup could not initialize authorized_keys watcher runtime. Failing closed."
                    );
                    return;
                }

                let warp_filter = serverimpl::create_warp_filter();
                let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
                let socket_addr =
                    std::net::SocketAddr::new(std::net::IpAddr::V4(server_ip), bind_port);

                // Secure-only transport: TLS materials must exist and load successfully.
                if tls::tls_materials_exist(&tls_config.cert_path, &tls_config.key_path) {
                    eprintln!(" service using TLS (secure transport)");
                    match tls::load_server_tls_config(&tls_config.cert_path, &tls_config.key_path) {
                        Ok(_) => {
                            let (_addr, server_future) = warp::serve(warp_filter)
                                .tls()
                                .cert_path(&tls_config.cert_path)
                                .key_path(&tls_config.key_path)
                                .bind_with_graceful_shutdown(socket_addr, async move {
                                    while running.load(Ordering::SeqCst) {
                                        tokio::time::sleep(Duration::from_millis(200)).await;
                                    }
                                });
                            server_future.await;
                        }
                        Err(e) => {
                            eprintln!("Failed to load TLS config: {}", e);
                            return;
                        }
                    }
                } else {
                    eprintln!(
                        "Fail-closed: service TLS materials not found at {} and {}",
                        tls_config.cert_path.display(),
                        tls_config.key_path.display()
                    );
                    return;
                }
            });
        }

        // start service dispatcher (this call will block and transfer control to SCM)
        extern "system" fn service_main_dispatcher(argc: u32, argv: *mut *mut u16) {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;

            let mut args: Vec<OsString> = Vec::new();

            if argv.is_null() {
                // no arguments passed; call the Rust service entry
                service_main(args);
                return;
            }

            unsafe {
                for i in 0..(argc as isize) {
                    let ptr = *argv.offset(i);
                    if ptr.is_null() {
                        args.push(OsString::new());
                        continue;
                    }
                    let mut len: usize = 0;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    args.push(OsString::from_wide(slice));
                }
            }

            service_main(args);
        }

        let _ = SERVICE_BIND_ADDR.set(bind_addr.to_string());
        let _ = SERVICE_BIND_PORT.set(bind_port);

        match service_dispatcher::start(WINDOWS_SERVICE_NAME, service_main_dispatcher) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!(
                    "service_dispatcher failed: {:?}. Falling back to foreground run.",
                    e
                );
            }
        }

        #[cfg(not(windows))]
        {
            println!("--service ignored on non-Windows platforms; running in foreground instead");
        }
    }

    // Normal foreground run
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_server(bind_addr, bind_port));

    Ok(())
}
