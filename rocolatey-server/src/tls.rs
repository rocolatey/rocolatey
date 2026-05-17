use std::path::Path;
use rocolatey_lib::server::ServerTlsConfig;

/// Placeholder for TLS validation.
/// Phase 2: warp's tls().cert_path().key_path() methods handle loading.
/// We just validate that files exist and are readable.
pub fn load_server_tls_config(
    cert_path: &Path,
    key_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate cert file is readable
    std::fs::read(cert_path)?;
    // Validate key file is readable
    std::fs::read(key_path)?;
    Ok(())
}

/// Check if TLS materials exist (cert and key files).
pub fn tls_materials_exist(cert_path: &Path, key_path: &Path) -> bool {
    cert_path.exists() && key_path.exists()
}

/// Ensure server TLS materials exist, bootstrapping if necessary.
/// This is called during server startup to guarantee TLS materials are available.
pub fn ensure_server_tls_materials() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerTlsConfig::default();
    rocolatey_lib::bootstrap::bootstrap_server_tls(&config)
}

