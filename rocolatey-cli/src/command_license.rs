use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct LicenseInfo {
    license: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Library {
    package_name: String,
    license: String,
    licenses: Vec<LicenseInfo>,
}

#[derive(Debug, Deserialize)]
struct Root {
    third_party_libraries: Vec<Library>,
}

include!(concat!(env!("OUT_DIR"), "/licenses_json.rs"));

fn parse_json(data: &str) -> Result<Root, serde_json::Error> {
    serde_json::from_str(data)
}

fn normalize_license(license: &str) -> String {
    let separators = [" OR ", "/"];
    let mut parts: Vec<&str> = Vec::new();

    for sep in &separators {
        if license.contains(sep) {
            parts = license.split(sep).collect();
            break;
        }
    }

    if parts.is_empty() {
        parts.push(license);
    }

    parts.sort();
    parts.join(" OR ")
}

pub fn license(matches: &clap::ArgMatches) {
    println!("Rocolatey is licensed under the {}", ROCO_LICENSE_JSON);
    println!("------------------------------------------------");
    println!(" Rocolatey is built using the following crates: ");
    println!("------------------------------------------------");

    let root: Root = parse_json(JSON_LICENSE_DATA).expect("Failed to parse JSON");

    // Check if the 'full' flag is set
    let full = matches.get_flag("full");

    if full {
        // Print all packages with their full license text
        for library in root.third_party_libraries {
            println!("Package: {}", library.package_name);
            for license_info in library.licenses {
                println!("License: {}", license_info.license);
                println!("{}", license_info.text);
            }
            println!("------------------------------------------------");
        }
    } else {
        // Create a HashMap to group packages by license
        let mut license_map: HashMap<String, Vec<String>> = HashMap::new();

        // Populate the HashMap
        for library in root.third_party_libraries {
            let normalized_license = normalize_license(&library.license);
            license_map
                .entry(normalized_license)
                .or_insert_with(Vec::new)
                .push(library.package_name.clone());
        }

        // Print the licenses and their respective packages
        for (license, packages) in license_map {
            println!("License: {}", license);
            println!("Packages: {}", packages.join(", "));
            println!("------------------------------------------------");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_license() {
        assert_eq!(normalize_license("MIT OR Apache-2.0"), "Apache-2.0 OR MIT");
        assert_eq!(normalize_license("Apache-2.0 OR MIT"), "Apache-2.0 OR MIT");
        assert_eq!(normalize_license("MIT/Apache-2.0"), "Apache-2.0 OR MIT");
        assert_eq!(normalize_license("Apache-2.0/MIT"), "Apache-2.0 OR MIT");
        assert_eq!(normalize_license("MIT"), "MIT");
    }
}
