use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::roco::roco_server;

pub mod roco;

pub static ROCO_VERBOSE: AtomicBool = AtomicBool::new(false);
pub static ROCO_REQUIRE_SSL: AtomicBool = AtomicBool::new(false);

pub mod server {
    use crate::roco::{Feed, OutdatedInfo, Package};

    pub const ROCO_SERVER_DEFAULT_PORT: &str = "29295"; // derived from "ro" = 0x726F;
    pub const ROCO_SERVER_SCHEMA_VERSION: u32 = 1;

    #[derive(Serialize, Deserialize, Clone)]
    pub enum JobStatus {
        Pending,
        Running,
        Completed { exit_code: i32 },
        Failed { error: String },
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct JobState {
        pub id: Uuid,
        pub status: JobStatus,
        pub created_at: std::time::SystemTime,
        pub logs: Vec<String>, // store recent log lines
    }

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;
    #[derive(Debug, Deserialize, Serialize)]
    pub struct RocoServerChocoCommandRequest {
        pub command: String,
        pub args: Vec<String>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct RocoServerChocoJobIdResponse {
        pub id: String,
        pub status: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct RocoServerOutdatedRequest {
        pub pkg: String,
        pub pre: bool,
        pub ignore_pinned: bool,
        pub ignore_unfound: bool,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct RocoServerListRequest {
        pub filter: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct RocoServerSearchRequest {
        pub terms: Vec<String>,
        pub prerelease: bool,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct RocoServerPackagesResponse {
        pub schema_version: u32,
        pub total_count: Option<usize>,
        pub data: Vec<Package>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct RocoServerFeedsResponse {
        pub schema_version: u32,
        pub data: Vec<Feed>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct RocoServerOutdatedResponse {
        pub schema_version: u32,
        pub data: Vec<OutdatedInfo>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct RocoServerDependencyTreeResponse {
        pub schema_version: u32,
        pub data: Vec<DependencyTreeNode>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct DependencyTreeNode {
        pub id: String,
        pub version: String,
        pub depth: usize,
        pub parent_id: Option<String>,
        pub missing: bool,
    }


    pub fn get_server_port() -> (bool, String) {
        std::env::var("ROCO_SERVER_PORT")
            .map(|v| (true, v))
            .unwrap_or_else(|_| (false, ROCO_SERVER_DEFAULT_PORT.to_string()))
    }

    pub fn get_server_ip() -> (bool, String) {
        std::env::var("ROCO_SERVER_IP")
            .map(|v| (true, v))
            .unwrap_or_else(|_| (false, "127.0.0.1".into()))
    }

    pub fn get_server_poll_interval_millis() -> u64 {
        std::env::var("ROCO_SERVER_POLL_MILLIS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(500)
    }
}

pub fn set_ssl_enabled(enable_ssl: bool) {
    ROCO_REQUIRE_SSL.store(enable_ssl, Ordering::Relaxed);
}

pub fn is_ssl_required() -> bool {
    ROCO_REQUIRE_SSL.load(Ordering::Relaxed)
}

pub fn set_verbose_mode(verbose: bool) {
    ROCO_VERBOSE.store(verbose, Ordering::Relaxed);
}

pub fn is_verbose_mode() -> bool {
    ROCO_VERBOSE.load(Ordering::Relaxed)
}

pub fn println_verbose(text: &str) {
    if is_verbose_mode() {
        println!("VERBOSE: {}", text);
    }
}

/// Execute a Chocolatey command, choosing between local and server execution.
///
/// Parameters:
/// - `choco_args`: Arguments supplied to the choco command (first element
///   may be treated as the command name).
/// - `package_names`: Package names appended to the command arguments.
///
/// Returns `true` if the invoked execution path (local or server) reports
/// success, `false` on failure.
pub async fn run_choco(choco_args: &[&str], package_names: &[&str]) -> i32 {
    // If we are already elevated, run locally (preferred for admin access).
    if crate::is_elevated().unwrap_or(false) {
        return run_local(choco_args, package_names).await;
    }

    // If we are not elevated, only use the server path when a
    // chocolatey server executable is colocated with the current executable.
    let use_server = match std::env::current_exe() {
        Ok(path) => {
            if let Some(dir) = path.parent() {
                let bases = ["rocolatey-server"];
                bases.iter().any(|b| {
                    let plain = dir.join(b);
                    let with_exe = dir.join(format!("{}.exe", b));
                    plain.exists() || with_exe.exists()
                })
            } else {
                false
            }
        }
        Err(_) => false,
    };

    if use_server {
        roco_server::run_on_server_poll(choco_args, package_names).await
    } else {
        run_local(choco_args, package_names).await
    }
}

/// Run `choco.exe` locally with the given arguments and package names.
///
/// Behavior:
/// - Spawns `choco.exe` with `choco_args` and `package_names`.
/// - Inherits `stdin`, `stdout`, and `stderr` from the parent process,
///   so output and errors are streamed live to the caller's console
///   (they are not captured or returned).
/// - Blocks until the process exits and returns `true` if the
///   process exited successfully (`exit code == 0`).
///
/// Note: inheriting `stdin` allows interactive prompts from `choco.exe`.
async fn run_local(choco_args: &[&str], package_names: &[&str]) -> i32 {
    let status = Command::new("choco.exe")
        .args(choco_args)
        .args(package_names)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to run choco command");

    status.code().unwrap_or(-1)
}

/// Returns Ok(true) if the current user/process is allowed to elevate,
/// Ok(false) if not allowed, Err(...) on unexpected failures.
/// Unix: true if uid==0 or `sudo -n true` succeeds.
/// Windows: true if group list contains Administrators (SID S-1-5-32-544 or name).
pub fn can_elevate() -> Result<bool, String> {
    #[cfg(unix)]
    {
        // fast check: are we already root?
        match Command::new("id").arg("-u").output() {
            Ok(out) if out.status.success() => {
                let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if uid == "0" {
                    return Ok(true);
                }
            }
            Ok(_) => {}
            Err(e) => return Err(format!("failed to run `id -u`: {}", e)),
        }

        // try passwordless sudo (non-interactive)
        match Command::new("sudo").arg("-n").arg("true").output() {
            Ok(out) => Ok(out.status.success()),
            Err(e) => Err(format!("failed to run `sudo -n true`: {}", e)),
        }
    }

    #[cfg(windows)]
    {
        match Command::new("whoami").arg("/groups").output() {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
                // check for Administrators SID or the group name
                if text.contains("s-1-5-32-544") || text.contains("administrators") {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                Err(format!("whoami returned non-zero: {}", err))
            }
            Err(e) => Err(format!("failed to run `whoami /groups`: {}", e)),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err("unsupported platform".into())
    }
}

/// Returns Ok(true) if the current process is already elevated,
/// Ok(false) if not elevated, Err(...) on unexpected failures.
/// Unix: true if uid==0.
/// Windows: uses PowerShell to check if process is running as Administrator.
pub fn is_elevated() -> Result<bool, String> {
    #[cfg(unix)]
    {
        // fast check: are we already root?
        match Command::new("id").arg("-u").output() {
            Ok(out) if out.status.success() => {
                let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
                Ok(uid == "0")
            }
            Ok(_) => Ok(false),
            Err(e) => Err(format!("failed to run `id -u`: {}", e)),
        }
    }

    #[cfg(windows)]
    {
        // Use PowerShell to ask the .NET APIs whether we're running as Administrator.
        // This returns "True" or "False" on stdout.
        let ps_cmd = "(New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
        match Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(ps_cmd)
            .output()
        {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                if text.contains("true") {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Ok(out) => {
                // powerhsell ran but returned non-zero; include stderr
                let err = String::from_utf8_lossy(&out.stderr);
                Err(format!("powershell returned non-zero: {}", err))
            }
            Err(e) => Err(format!("failed to run `powershell -Command ...`: {}", e)),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err("unsupported platform".into())
    }
}
