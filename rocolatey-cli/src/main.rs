extern crate clap;
mod cli;

use serde::Deserialize;
use serde_json::Result;
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

use rocolatey_lib::roco::{
    local::{
        get_dependency_tree_text, get_local_bad_packages_text, get_local_packages_text,
        get_sources_text,
    },
    remote::get_outdated_packages,
};

fn handle_list_command(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let filter = matches.get_one::<String>("filter").unwrap();
    if matches.get_flag("deptree") {
        print!("{}", get_dependency_tree_text(filter));
    } else {
        print!("{}", get_local_packages_text(filter, r));
    }
}

fn handle_bad_command(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    print!("{}", get_local_bad_packages_text(r));
}

fn parse_json(data: &str) -> Result<Root> {
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

fn handle_license_command(matches: &clap::ArgMatches) {
    println!("Rocolatey is licensed under the {}", ROCO_LICENSE_JSON);
    println!("------------------------------------------------");
    println!(" Rocolatey is built using the following crates: ");
    println!("------------------------------------------------");

    let root: Root = parse_json(JSON_LICENSE_DATA).expect("Failed to parse JSON");

    if matches.get_flag("full") {
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
        let mut license_map: HashMap<String, Vec<String>> = HashMap::new();

        for library in root.third_party_libraries {
            let normalized_license = normalize_license(&library.license);
            license_map
                .entry(normalized_license)
                .or_insert_with(Vec::new)
                .push(library.package_name.clone());
        }

        for (license, packages) in license_map {
            println!("License: {}", license);
            println!("Packages: {}", packages.join(", "));
            println!("------------------------------------------------");
        }
    }
}

async fn handle_outdated_command(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    rocolatey_lib::set_ssl_enabled(matches.get_flag("ssl-validation-enabled"));
    let r = matches.get_flag("limitoutput");
    let l: bool = matches.get_flag("listoutput");
    let pre = matches.get_flag("prerelease");
    let choco_compat = matches.get_flag("choco-compat");
    let ignore_pinned = !choco_compat || matches.get_flag("ignore-pinned");
    let ignore_unfound = !choco_compat || matches.get_flag("ignore-unfound");
    let pkg = matches.get_one::<String>("pkg").unwrap();
    print!(
        "{}",
        get_outdated_packages(pkg, r, l, pre, ignore_pinned, ignore_unfound).await
    );
}

fn handle_source_command(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    print!("{}", get_sources_text(r));
}

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    match matches.subcommand() {
        Some(("list", matches)) => handle_list_command(matches),
        Some(("bad", matches)) => handle_bad_command(matches),
        Some(("license", matches)) => handle_license_command(matches),
        Some(("outdated", matches)) => handle_outdated_command(matches).await,
        Some(("source", matches)) => handle_source_command(matches),
        _ => {
            let _ = cli::build_cli().print_help();
            println!();
        }
    }
}
