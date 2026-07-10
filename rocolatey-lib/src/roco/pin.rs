use std::fs;
use std::path::PathBuf;
use crate::roco::get_chocolatey_dir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PinnedPackage {
    pub id: String,
    pub version: Option<String>,
}

pub fn list_pins() -> Vec<PinnedPackage> {
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut pins = Vec::new();
    let mut chocolatey_dir = PathBuf::from(choco_dir);
    chocolatey_dir.push(".chocolatey");

    let entries = match fs::read_dir(&chocolatey_dir) {
        Ok(e) => e,
        Err(_) => return pins,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pin_file = path.join(".pin");
        if !pin_file.exists() {
            continue;
        }
        let dirname = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        if let Some(dot_pos) = dirname.rfind('.') {
            let id = dirname[..dot_pos].to_string();
            let version = dirname[dot_pos + 1..].to_string();
            pins.push(PinnedPackage {
                id,
                version: Some(version),
            });
        } else {
            pins.push(PinnedPackage {
                id: dirname,
                version: None,
            });
        }
    }

    pins
}

pub fn add_pin(id: &str, version: Option<&str>) -> Result<(), String> {
    let choco_dir = get_chocolatey_dir().map_err(|e| format!("ChocolateyInstall not set: {}", e))?;
    let mut pin_dir = PathBuf::from(choco_dir);
    pin_dir.push(".chocolatey");
    pin_dir.push(match version {
        Some(v) => format!("{}.{}", id, v),
        None => id.to_string(),
    });

    fs::create_dir_all(&pin_dir).map_err(|e| format!("failed to create pin directory: {}", e))?;

    let pin_file = pin_dir.join(".pin");
    if pin_file.exists() {
        return Ok(());
    }

    fs::write(&pin_file, "").map_err(|e| format!("failed to write pin file: {}", e))?;
    Ok(())
}

pub fn remove_pin(id: &str, version: Option<&str>) -> Result<(), String> {
    let choco_dir = get_chocolatey_dir().map_err(|e| format!("ChocolateyInstall not set: {}", e))?;
    let mut pin_dir = PathBuf::from(choco_dir);
    pin_dir.push(".chocolatey");
    pin_dir.push(match version {
        Some(v) => format!("{}.{}", id, v),
        None => id.to_string(),
    });

    let pin_file = pin_dir.join(".pin");
    if !pin_file.exists() {
        return Err(format!("pin not found for {} {}", id, version.unwrap_or("(all versions)")));
    }

    fs::remove_file(&pin_file).map_err(|e| format!("failed to remove pin file: {}", e))?;

    if pin_dir.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false) {
        let _ = fs::remove_dir(&pin_dir);
    }

    Ok(())
}
