use std::path::Path;
use crate::server::ClientTlsConfig;

/// Placeholder for client TLS configuration.
/// Phase 2 implementation: just switch to HTTPS.
/// Phase 3+ will add mutual TLS with certificate pinning.
pub fn load_client_tls_config(
    _cert_path: &Path,
    _key_path: &Path,
    _known_server_keys_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Phase 2: TLS is supported by reqwest by default with system roots
    // No additional configuration needed for basic HTTPS
    // Phase 3+ will add mutual TLS certificate loading
    Ok(())
}

/// Check if TLS client materials exist (cert and key files).
pub fn client_tls_materials_exist(cert_path: &Path, key_path: &Path) -> bool {
    cert_path.exists() && key_path.exists()
}

/// Ensure client TLS materials exist, bootstrapping if necessary
pub fn ensure_client_tls_materials() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientTlsConfig::default();
    crate::bootstrap::bootstrap_client_tls(&config)
}

/// Get diagnostics about client TLS setup status
pub fn get_client_diagnostics() -> ClientTlsDiagnostics {
    let config = ClientTlsConfig::default();
    
    ClientTlsDiagnostics {
        cert_path: config.cert_path.clone(),
        cert_exists: config.cert_exists(),
        key_path: config.key_path.clone(),
        key_exists: config.key_exists(),
        known_server_keys_path: config.known_server_keys_path.clone(),
        known_server_keys_exist: config.known_server_keys_exist(),
        materials_ready: config.cert_exists() && config.key_exists(),
    }
}

#[derive(Debug, Clone)]
pub struct ClientTlsDiagnostics {
    pub cert_path: std::path::PathBuf,
    pub cert_exists: bool,
    pub key_path: std::path::PathBuf,
    pub key_exists: bool,
    pub known_server_keys_path: std::path::PathBuf,
    pub known_server_keys_exist: bool,
    pub materials_ready: bool,
}
