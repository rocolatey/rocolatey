use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::{Path, PathBuf};
use std::fs;

use crate::roco::roco_server;

pub mod roco;

pub static ROCO_VERBOSE: AtomicBool = AtomicBool::new(false);
pub static ROCO_REQUIRE_SSL: AtomicBool = AtomicBool::new(false);

pub mod server {
    use crate::roco::{Feed, OutdatedInfo, Package};
    use std::path::PathBuf;

    pub mod authorization;
    pub mod audit;

    pub const ROCO_SERVER_DEFAULT_PORT: &str = "29295"; // derived from "ro" = 0x726F;
    pub const ROCO_SERVER_SCHEMA_VERSION: u32 = 1;
    pub const ROCO_TRUST_DIR_NAME: &str = "rocolatey";
    pub const ROCO_CLIENT_TRUST_DIR_NAME: &str = "client";
    pub const ROCO_SERVER_TRUST_DIR_NAME: &str = "server";
    pub const ROCO_SERVER_CERT_FILE: &str = "server.crt.pem";
    pub const ROCO_SERVER_KEY_FILE: &str = "server.key.pem";
    pub const ROCO_SERVER_AUTHORIZED_KEYS_FILE: &str = "authorized_keys";
    pub const ROCO_SERVER_ROTATION_STATE_FILE: &str = "server_rotation_state.json";
    pub const ROCO_SERVER_CONTINUITY_PROOF_FILE: &str = "server_key_continuity.json";
    pub const ROCO_SERVER_EMERGENCY_OVERRIDE_FILE: &str = "emergency_override.json";
    pub const ROCO_CLIENT_CERT_FILE: &str = "client.crt.pem";
    pub const ROCO_CLIENT_KEY_FILE: &str = "client.key.pem";
    pub const ROCO_CLIENT_KNOWN_SERVER_KEYS_FILE: &str = "known_server_keys";
    pub const ROCO_CLIENT_ROTATION_STATE_FILE: &str = "client_rotation_state.json";

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum RocoServerDenyCode {
        NotEnrolledClient,
        ClientCertificateMissing,
        ClientCertificateInvalid,
        ClientCertificateExpired,
        TimeSkewExceeded,
        ServerTrustMismatch,
        InvalidRequest,
        ForbiddenOperation,
        ResourceNotFound,
        InternalAuthorizationError,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    pub struct RocoServerDenyResponse {
        pub schema_version: u32,
        pub request_id: String,
        pub code: RocoServerDenyCode,
        pub message: String,
        pub enrollment_hint: Option<String>,
        pub short_fingerprint: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum RocoServerTrustMode {
        EmptyEnrollment,
        Enforced,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    pub struct RocoServerTrustStateResponse {
        pub schema_version: u32,
        pub request_id: String,
        pub trust_mode: RocoServerTrustMode,
        pub server_key_present: bool,
        pub server_cert_present: bool,
        pub authorized_key_count: usize,
        pub authorized_keys_watcher_healthy: bool,
        pub emergency_override_active: bool,
    }

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

    #[cfg(windows)]
    fn default_windows_config_root() -> PathBuf {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|p| p.join("AppData").join("Roaming"))
            })
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
    }

    fn default_linux_config_root() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|p| p.join(".config"))
            })
            .unwrap_or_else(|| PathBuf::from("/etc"))
    }

    pub fn default_client_trust_dir() -> PathBuf {
        #[cfg(windows)]
        {
            return default_windows_config_root()
                .join(ROCO_TRUST_DIR_NAME)
                .join(ROCO_CLIENT_TRUST_DIR_NAME);
        }

        #[cfg(not(windows))]
        {
            default_linux_config_root()
                .join(ROCO_TRUST_DIR_NAME)
                .join(ROCO_CLIENT_TRUST_DIR_NAME)
        }
    }

    pub fn default_server_trust_dir() -> PathBuf {
        if let Some(override_dir) = std::env::var_os("ROCO_SERVER_TRUST_DIR") {
            return PathBuf::from(override_dir);
        }

        #[cfg(windows)]
        {
            return std::env::var_os("PROGRAMDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
                .join(ROCO_TRUST_DIR_NAME)
                .join(ROCO_SERVER_TRUST_DIR_NAME);
        }

        #[cfg(not(windows))]
        {
            PathBuf::from("/etc")
                .join(ROCO_TRUST_DIR_NAME)
                .join(ROCO_SERVER_TRUST_DIR_NAME)
        }
    }

    pub fn default_client_cert_path() -> PathBuf {
        default_client_trust_dir().join(ROCO_CLIENT_CERT_FILE)
    }

    pub fn default_client_key_path() -> PathBuf {
        default_client_trust_dir().join(ROCO_CLIENT_KEY_FILE)
    }

    pub fn default_client_known_server_keys_path() -> PathBuf {
        default_client_trust_dir().join(ROCO_CLIENT_KNOWN_SERVER_KEYS_FILE)
    }

    pub fn default_server_cert_path() -> PathBuf {
        default_server_trust_dir().join(ROCO_SERVER_CERT_FILE)
    }

    pub fn default_server_key_path() -> PathBuf {
        default_server_trust_dir().join(ROCO_SERVER_KEY_FILE)
    }

    pub fn default_server_authorized_keys_path() -> PathBuf {
        default_server_trust_dir().join(ROCO_SERVER_AUTHORIZED_KEYS_FILE)
    }

    pub fn default_server_rotation_state_path() -> PathBuf {
        default_server_trust_dir().join(ROCO_SERVER_ROTATION_STATE_FILE)
    }

    pub fn default_server_continuity_proof_path() -> PathBuf {
        default_server_trust_dir().join(ROCO_SERVER_CONTINUITY_PROOF_FILE)
    }

    pub fn default_client_rotation_state_path() -> PathBuf {
        default_client_trust_dir().join(ROCO_CLIENT_ROTATION_STATE_FILE)
    }

    #[derive(Debug, Clone)]
    pub struct ServerTlsConfig {
        pub cert_path: PathBuf,
        pub key_path: PathBuf,
        pub authorized_keys_path: PathBuf,
    }

    impl ServerTlsConfig {
        pub fn new(cert_path: PathBuf, key_path: PathBuf, authorized_keys_path: PathBuf) -> Self {
            ServerTlsConfig {
                cert_path,
                key_path,
                authorized_keys_path,
            }
        }

        pub fn default() -> Self {
            ServerTlsConfig {
                cert_path: default_server_cert_path(),
                key_path: default_server_key_path(),
                authorized_keys_path: default_server_authorized_keys_path(),
            }
        }

        pub fn cert_exists(&self) -> bool {
            self.cert_path.exists()
        }

        pub fn key_exists(&self) -> bool {
            self.key_path.exists()
        }

        pub fn authorized_keys_exist(&self) -> bool {
            self.authorized_keys_path.exists()
        }

        pub fn read_cert_pem(&self) -> Result<Vec<u8>, std::io::Error> {
            std::fs::read(&self.cert_path)
        }

        pub fn read_key_pem(&self) -> Result<Vec<u8>, std::io::Error> {
            std::fs::read(&self.key_path)
        }

        pub fn read_authorized_keys(&self) -> Result<String, std::io::Error> {
            std::fs::read_to_string(&self.authorized_keys_path)
        }

        pub fn rotation_state_path(&self) -> PathBuf {
            self.cert_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(ROCO_SERVER_ROTATION_STATE_FILE)
        }

        pub fn continuity_proof_path(&self) -> PathBuf {
            self.cert_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(ROCO_SERVER_CONTINUITY_PROOF_FILE)
        }

        pub fn emergency_override_path(&self) -> PathBuf {
            self.cert_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(ROCO_SERVER_EMERGENCY_OVERRIDE_FILE)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ClientTlsConfig {
        pub cert_path: PathBuf,
        pub key_path: PathBuf,
        pub known_server_keys_path: PathBuf,
    }

    impl ClientTlsConfig {
        pub fn new(cert_path: PathBuf, key_path: PathBuf, known_server_keys_path: PathBuf) -> Self {
            ClientTlsConfig {
                cert_path,
                key_path,
                known_server_keys_path,
            }
        }

        pub fn default() -> Self {
            ClientTlsConfig {
                cert_path: default_client_cert_path(),
                key_path: default_client_key_path(),
                known_server_keys_path: default_client_known_server_keys_path(),
            }
        }

        pub fn cert_exists(&self) -> bool {
            self.cert_path.exists()
        }

        pub fn key_exists(&self) -> bool {
            self.key_path.exists()
        }

        pub fn known_server_keys_exist(&self) -> bool {
            self.known_server_keys_path.exists()
        }

        pub fn read_cert_pem(&self) -> Result<Vec<u8>, std::io::Error> {
            std::fs::read(&self.cert_path)
        }

        pub fn read_key_pem(&self) -> Result<Vec<u8>, std::io::Error> {
            std::fs::read(&self.key_path)
        }

        pub fn read_known_server_keys(&self) -> Result<String, std::io::Error> {
            std::fs::read_to_string(&self.known_server_keys_path)
        }

        pub fn rotation_state_path(&self) -> PathBuf {
            self.cert_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(ROCO_CLIENT_ROTATION_STATE_FILE)
        }
    }

    /// Server TLS diagnostics for setup and troubleshooting
    #[derive(Debug, Clone)]
    pub struct ServerTlsDiagnostics {
        pub cert_path: PathBuf,
        pub cert_exists: bool,
        pub key_path: PathBuf,
        pub key_exists: bool,
        pub authorized_keys_path: PathBuf,
        pub authorized_keys_exist: bool,
        pub authorized_keys_valid: bool,
        pub authorized_keys_entry_count: usize,
        pub materials_ready: bool,
        pub in_empty_enrollment_mode: bool,
    }
}

/// Bootstrap and key generation utilities for TLS certificate setup
pub mod bootstrap {
    use super::*;
    use sha2::Digest;
    use sha2::Sha256;
    use base64::Engine;
    use serde::{Deserialize, Serialize};

    /// Certificate validity period in days
    const CERT_VALIDITY_DAYS: u32 = 360;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RotationStateRecord {
        pub identity_fingerprint: String,
        pub issued_at_utc: chrono::DateTime<chrono::Utc>,
        pub rotate_after_utc: chrono::DateTime<chrono::Utc>,
        pub overlap_until_utc: chrono::DateTime<chrono::Utc>,
        pub jitter_seconds: i64,
        pub previous_identity_fingerprint: Option<String>,
        pub continuity_proof: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServerContinuityProofRecord {
        pub previous_identity_fingerprint: String,
        pub new_identity_fingerprint: String,
        pub continuity_proof: String,
        pub issued_at_utc: chrono::DateTime<chrono::Utc>,
    }

    const MAX_BACKUPS_PER_TARGET: usize = 3;

    fn write_rotation_state(
        rotation_state_path: &Path,
        cert_pem: &[u8],
        previous_identity_fingerprint: Option<String>,
    ) -> Result<RotationStateRecord, Box<dyn std::error::Error>> {
        let identity_fingerprint = fingerprint_full(cert_pem)?;
        let issued_at_utc = chrono::Utc::now();
        let window = server::authorization::compute_rotation_window(
            issued_at_utc,
            &identity_fingerprint,
        );

        let continuity_proof = previous_identity_fingerprint
            .as_ref()
            .map(|old| server::authorization::build_continuity_proof(old, &identity_fingerprint));

        let record = RotationStateRecord {
            identity_fingerprint,
            issued_at_utc,
            rotate_after_utc: window.rotate_after,
            overlap_until_utc: window.overlap_until,
            jitter_seconds: window.jitter_seconds,
            previous_identity_fingerprint,
            continuity_proof,
        };

        let serialized = serde_json::to_vec_pretty(&record)?;
        write_atomic_with_backup(rotation_state_path, &serialized)?;
        Ok(record)
    }

    pub fn load_rotation_state(
        rotation_state_path: &Path,
    ) -> Result<Option<RotationStateRecord>, Box<dyn std::error::Error>> {
        if !rotation_state_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(rotation_state_path)?;
        let state = serde_json::from_str::<RotationStateRecord>(&content)?;
        Ok(Some(state))
    }

    pub fn auto_rotate_server_tls_if_due(
        config: &server::ServerTlsConfig,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if !config.cert_exists() || !config.key_exists() {
            return Ok(false);
        }

        let state = load_rotation_state(&config.rotation_state_path())?;
        let state = match state {
            Some(v) => v,
            None => {
                let cert_pem = std::fs::read(&config.cert_path)?;
                write_rotation_state(&config.rotation_state_path(), &cert_pem, None)?;
                return Ok(false);
            }
        };

        if chrono::Utc::now() >= state.rotate_after_utc {
            regenerate_server_tls_with_backup(config)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn read_server_continuity_proof(
        config: &server::ServerTlsConfig,
    ) -> Result<Option<ServerContinuityProofRecord>, Box<dyn std::error::Error>> {
        let path = config.continuity_proof_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(path)?;
        let record = serde_json::from_str::<ServerContinuityProofRecord>(&content)?;
        Ok(Some(record))
    }

    pub fn validate_server_continuity_proof(
        config: &server::ServerTlsConfig,
    ) -> Result<Option<ServerContinuityProofRecord>, Box<dyn std::error::Error>> {
        let Some(record) = read_server_continuity_proof(config)? else {
            return Ok(None);
        };

        let expected = server::authorization::build_continuity_proof(
            &record.previous_identity_fingerprint,
            &record.new_identity_fingerprint,
        );

        if expected != record.continuity_proof {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "server continuity proof mismatch",
            )
            .into());
        }

        let current_cert = std::fs::read(&config.cert_path)?;
        let current_fp = fingerprint_full(&current_cert)?;
        if current_fp != record.new_identity_fingerprint {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "server continuity proof does not match current server certificate fingerprint",
            )
            .into());
        }

        Ok(Some(record))
    }

    /// Generate a self-signed certificate and private key for TLS.
    /// Returns (cert_pem, key_pem) tuple.
    pub fn generate_self_signed_cert(
        subject_cn: &str,
        days_valid: u32,
    ) -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
        use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};

        // Generate a simple self-signed certificate
        let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
        
        let mut params = CertificateParams::new(subject_alt_names);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, subject_cn);
        
        // Set validity period
        let not_before = time::OffsetDateTime::now_utc();
        let not_after = not_before + time::Duration::days(days_valid as i64);
        params.not_before = not_before;
        params.not_after = not_after;

        let cert = Certificate::from_params(params)?;
        let cert_pem = cert.serialize_pem()?;
        let key_pem = cert.serialize_private_key_pem();

        Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
    }

    /// Write a file atomically using a temporary file and rename.
    /// If the target file exists, creates a timestamped backup first.
    pub fn write_atomic_with_backup(
        target_path: &Path,
        content: &[u8],
    ) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
        // Create parent directory if needed
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut backup_path = None;

        // If file exists, create timestamped backup
        if target_path.exists() {
            let stem = target_path.file_stem().ok_or("Invalid file path")?;
            let ext = target_path.extension().unwrap_or_default();
            let parent = target_path.parent().unwrap_or_else(|| Path::new("."));

            let now = chrono::Utc::now().timestamp_millis();
            let backup_name = if ext.len() > 0 {
                format!("{}.{}.{}", stem.to_string_lossy(), now, ext.to_string_lossy())
            } else {
                format!("{}.{}", stem.to_string_lossy(), now)
            };

            let backup = parent.join(&backup_name);
            fs::copy(target_path, &backup)?;
            backup_path = Some(backup);

            prune_backups_for_target(target_path, MAX_BACKUPS_PER_TARGET)?;
        }

        // Write to temporary file
        let temp_path = {
            let mut path = target_path.to_path_buf();
            let file_name = path.file_name().ok_or("Invalid file name")?
                .to_string_lossy()
                .to_string();
            path.pop();
            path.push(format!(".tmp.{}", file_name));
            path
        };

        fs::write(&temp_path, content)?;

        // Atomic rename
        fs::rename(&temp_path, target_path)?;

        Ok(backup_path)
    }

    fn prune_backups_for_target(
        target_path: &Path,
        keep_latest: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parent = match target_path.parent() {
            Some(p) => p,
            None => return Ok(()),
        };

        let stem = target_path
            .file_stem()
            .ok_or("Invalid file path")?
            .to_string_lossy()
            .to_string();
        let ext = target_path
            .extension()
            .map(|e| e.to_string_lossy().to_string());

        let prefix = format!("{}.", stem);
        let suffix = ext
            .as_ref()
            .map(|e| format!(".{}", e))
            .unwrap_or_default();

        let mut backups: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();

        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };

            if !file_name.starts_with(&prefix) {
                continue;
            }

            if !suffix.is_empty() && !file_name.ends_with(&suffix) {
                continue;
            }

            let middle_end = if suffix.is_empty() {
                file_name.len()
            } else {
                file_name.len() - suffix.len()
            };

            if middle_end <= prefix.len() {
                continue;
            }

            let middle = &file_name[prefix.len()..middle_end];
            if middle.is_empty() || !middle.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            backups.push((modified, path));
        }

        backups.sort_by(|a, b| b.0.cmp(&a.0));

        for (_, path) in backups.into_iter().skip(keep_latest) {
            let _ = fs::remove_file(path);
        }

        Ok(())
    }

    /// Compute SHA256 fingerprint of a certificate in base64url format (short form)
    pub fn fingerprint_short(cert_pem: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let mut hasher = Sha256::new();
        hasher.update(cert_pem);
        let digest = hasher.finalize();
        let base64_digest = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(digest.as_slice());
        // Return first 12 chars for short fingerprint
        Ok(base64_digest.chars().take(12).collect())
    }

    /// Compute full SHA256 fingerprint in base64url format
    pub fn fingerprint_full(cert_pem: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let mut hasher = Sha256::new();
        hasher.update(cert_pem);
        let digest = hasher.finalize();
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.as_slice()))
    }

    /// Bootstrap client TLS materials if they don't exist
    pub fn bootstrap_client_tls(
        config: &server::ClientTlsConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If both cert and key exist, no bootstrap needed
        if config.cert_exists() && config.key_exists() {
            return Ok(());
        }

        // Generate new certificate
        let (cert_pem, key_pem) = generate_self_signed_cert("roco-client", CERT_VALIDITY_DAYS)?;

        // Write cert and key atomically
        write_atomic_with_backup(&config.cert_path, &cert_pem)?;
        write_atomic_with_backup(&config.key_path, &key_pem)?;
        write_rotation_state(&config.rotation_state_path(), &cert_pem, None)?;

        Ok(())
    }

    /// Bootstrap server TLS materials if they don't exist
    pub fn bootstrap_server_tls(
        config: &server::ServerTlsConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If both cert and key exist, no bootstrap needed
        if config.cert_exists() && config.key_exists() {
            return Ok(());
        }

        // Generate new certificate
        let (cert_pem, key_pem) = generate_self_signed_cert("rocolatey-server", CERT_VALIDITY_DAYS)?;

        // Write cert and key atomically
        write_atomic_with_backup(&config.cert_path, &cert_pem)?;
        write_atomic_with_backup(&config.key_path, &key_pem)?;
        write_rotation_state(&config.rotation_state_path(), &cert_pem, None)?;

        // Ensure authorized_keys file exists (empty is OK)
        if !config.authorized_keys_exist() {
            write_atomic_with_backup(&config.authorized_keys_path, b"")?;
        }

        Ok(())
    }

    /// Regenerate TLS materials with timestamped backup
    pub fn regenerate_server_tls_with_backup(
        config: &server::ServerTlsConfig,
    ) -> Result<(Option<PathBuf>, Option<PathBuf>), Box<dyn std::error::Error>> {
        let previous_identity = std::fs::read(&config.cert_path)
            .ok()
            .and_then(|pem| fingerprint_full(&pem).ok());

        let (cert_pem, key_pem) = generate_self_signed_cert("rocolatey-server", CERT_VALIDITY_DAYS)?;

        let cert_backup = write_atomic_with_backup(&config.cert_path, &cert_pem)?;
        let key_backup = write_atomic_with_backup(&config.key_path, &key_pem)?;

        let record = write_rotation_state(&config.rotation_state_path(), &cert_pem, previous_identity)?;

        if let (Some(old_fp), Some(proof)) = (
            record.previous_identity_fingerprint.clone(),
            record.continuity_proof.clone(),
        ) {
            let continuity = ServerContinuityProofRecord {
                previous_identity_fingerprint: old_fp,
                new_identity_fingerprint: record.identity_fingerprint,
                continuity_proof: proof,
                issued_at_utc: record.issued_at_utc,
            };
            write_atomic_with_backup(
                &config.continuity_proof_path(),
                serde_json::to_string_pretty(&continuity)?.as_bytes(),
            )?;
        }

        Ok((cert_backup, key_backup))
    }

    /// Get server TLS diagnostics for setup and troubleshooting
    pub fn get_server_diagnostics() -> server::ServerTlsDiagnostics {
        let config = server::ServerTlsConfig::default();
        
        let cert_exists = config.cert_exists();
        let key_exists = config.key_exists();
        let authorized_keys_exist = config.authorized_keys_exist();
        let (authorized_keys_valid, authorized_keys_entry_count) = if authorized_keys_exist {
            match validate_authorized_keys_file(&config) {
                Ok(count) => (true, count),
                Err(_) => (false, 0),
            }
        } else {
            (false, 0)
        };

        server::ServerTlsDiagnostics {
            cert_path: config.cert_path.clone(),
            cert_exists,
            key_path: config.key_path.clone(),
            key_exists,
            authorized_keys_path: config.authorized_keys_path.clone(),
            authorized_keys_exist,
            authorized_keys_valid,
            authorized_keys_entry_count,
            materials_ready: cert_exists && key_exists && authorized_keys_valid,
            in_empty_enrollment_mode: authorized_keys_valid && authorized_keys_entry_count == 0,
        }
    }

    /// Validate authorized_keys file and return number of active entries.
    ///
    /// Accepted format:
    /// - one fingerprint token per line
    /// - blank lines and lines starting with '#' are ignored
    /// - fingerprint tokens must use base64url charset ([A-Za-z0-9_-])
    pub fn validate_authorized_keys_file(
        config: &server::ServerTlsConfig,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let parsed = server::authorization::load_authorized_keys_with_warnings(
            &config.authorized_keys_path,
        )?;
        Ok(parsed.fingerprints.len())
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
        anstream::println!("VERBOSE: {}", text);
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

#[cfg(test)]
mod tests {
    use super::bootstrap::*;

    fn backup_files_for(target_path: &std::path::Path) -> Vec<std::path::PathBuf> {
        let parent = target_path.parent().unwrap();
        let stem = target_path.file_stem().unwrap().to_string_lossy().to_string();
        let ext = target_path
            .extension()
            .map(|e| e.to_string_lossy().to_string());
        let prefix = format!("{}.", stem);
        let suffix = ext
            .as_ref()
            .map(|e| format!(".{}", e))
            .unwrap_or_default();

        let mut out = Vec::new();
        for entry in std::fs::read_dir(parent).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if !name.starts_with(&prefix) {
                continue;
            }
            if !suffix.is_empty() && !name.ends_with(&suffix) {
                continue;
            }
            let middle_end = if suffix.is_empty() {
                name.len()
            } else {
                name.len() - suffix.len()
            };
            if middle_end <= prefix.len() {
                continue;
            }
            let middle = &name[prefix.len()..middle_end];
            if middle.chars().all(|c| c.is_ascii_digit()) {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn test_generated_certificate_validity_period() {
        let (cert_pem, _key_pem) = generate_self_signed_cert("test-cert", 360)
            .expect("Failed to generate certificate");

        // Verify we got cert
        assert!(!cert_pem.is_empty(), "Certificate PEM should not be empty");

        // Parse the certificate and verify validity period
        let (_, pem) = x509_parser::pem::parse_x509_pem(&cert_pem)
            .expect("Failed to parse certificate PEM");
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)
            .expect("Failed to parse x509 certificate");

        let not_before_ts = cert.validity().not_before.timestamp();
        let not_after_ts = cert.validity().not_after.timestamp();
        
        // Calculate the difference in days
        let diff_seconds = not_after_ts - not_before_ts;
        let diff_days = diff_seconds / 86400; // seconds per day
        
        // Verify the validity period is 360 days (allowing some tolerance for clock skew)
        assert_eq!(diff_days, 360, "Certificate validity period should be exactly 360 days");
    }

    #[test]
    fn write_atomic_with_backup_prunes_to_last_three() {
        let temp_root = std::env::temp_dir().join(format!(
            "roco-backup-prune-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let target = temp_root.join("state.json");

        write_atomic_with_backup(&target, br#"{"v":1}"#).unwrap();
        for i in 2..=7 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let payload = format!("{{\"v\":{}}}", i);
            write_atomic_with_backup(&target, payload.as_bytes()).unwrap();
        }

        let backups = backup_files_for(&target);
        assert_eq!(backups.len(), 3, "should keep exactly 3 backups");

        let _ = std::fs::remove_dir_all(temp_root);
    }
}
