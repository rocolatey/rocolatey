use rocolatey_lib::roco::remote::get_outdated_packages;
use std::process::{Command, Stdio};

pub async fn upgrade(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    rocolatey_lib::set_ssl_enabled(matches.get_flag("ssl-validation-enabled"));
    let r = matches.get_flag("limitoutput");
    let pre = matches.get_flag("prerelease");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let (_, outdated_packages) = get_outdated_packages(pkg, r, pre, true, true).await;

    let package_names: Vec<&str> = outdated_packages
        .iter()
        .map(|pkg| pkg.id.as_str())
        .collect();

    if package_names.is_empty() {
        println!("No outdated packages found.");
        return;
    }

    let mut choco_args = vec!["upgrade", "--ignore-http-cache", "-y"];

    if pre {
        choco_args.push("--pre");
    }

    if r {
        choco_args.push("-r");
    }

    if rocolatey_lib::is_verbose_mode() {
        choco_args.push("-v");
    }

    let status = Command::new("choco")
        .args(&choco_args)
        .args(&package_names)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to start elevated choco upgrade process")
        .success();

    if status {
        println!(
            "Successfully upgraded packages: {}",
            package_names.join(", ")
        );
    } else {
        eprintln!("Failed to upgrade packages: {}", package_names.join(", "));
    }
}
