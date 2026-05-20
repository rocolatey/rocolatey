use serde::Deserialize;
use std::collections::HashMap;

use crate::output_style;

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
    let json = matches.get_flag("json-output");
    if json {
        anstream::println!("{}", JSON_LICENSE_DATA);
        return;
    }

    let mode = output_style::current();
    let separator = output_style::info("------------------------------------------------", mode);

    anstream::println!("Rocolatey is licensed under the {}", ROCO_LICENSE_JSON);
    anstream::println!("{}", separator);
    anstream::println!(
        "{}",
        output_style::header(" Rocolatey is built using the following crates: ", mode)
    );
    anstream::println!("{}", separator);

    let root: Root = parse_json(JSON_LICENSE_DATA).expect("Failed to parse JSON");

    // Check if the 'full' flag is set
    let full = matches.get_flag("full");

    if full {
        // Print all packages with their full license text
        for library in root.third_party_libraries {
            anstream::println!(
                "{} {}",
                output_style::header("Package:", mode),
                library.package_name
            );
            for license_info in library.licenses {
                anstream::println!(
                    "{} {}",
                    output_style::header("License:", mode),
                    license_info.license
                );
                anstream::println!("{}", license_info.text);
            }
            anstream::println!("{}", separator);
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
            anstream::println!("{} {}", output_style::header("License:", mode), license);
            anstream::println!(
                "{} {}",
                output_style::header("Packages:", mode),
                packages.join(", ")
            );
            anstream::println!("{}", separator);
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
