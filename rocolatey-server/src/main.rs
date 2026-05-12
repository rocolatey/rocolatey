use clap::{Arg, Command};
mod serverimpl;

use std::env;
use std::error::Error;
use std::fs::OpenOptions;
use std::path::PathBuf;

#[cfg(windows)]
static SERVICE_BIND_ADDR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
#[cfg(windows)]
static SERVICE_BIND_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

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

    let warp_filter = serverimpl::create_warp_filter();
    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    warp::serve(warp_filter).run((server_ip, bind_port)).await;
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
                match service_control_handler::register("RocolateyServer", move |control_event| {
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
                let warp_filter = serverimpl::create_warp_filter();
                let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
                let socket_addr =
                    std::net::SocketAddr::new(std::net::IpAddr::V4(server_ip), bind_port);
                let (_addr, server_future) =
                    warp::serve(warp_filter).bind_with_graceful_shutdown(socket_addr, async move {
                        while running.load(Ordering::SeqCst) {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                    });
                server_future.await;
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

        match service_dispatcher::start("RocolateyServer", service_main_dispatcher) {
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
