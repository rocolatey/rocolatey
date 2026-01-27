use rocolatey_lib::{println_verbose, roco::remote::get_outdated_packages};

pub async fn upgrade(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    rocolatey_lib::set_ssl_enabled(matches.get_flag("ssl-validation-enabled"));
    let r = matches.get_flag("limitoutput");
    let pre = matches.get_flag("prerelease");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let (_, outdated_packages) = get_outdated_packages(pkg, r, pre, true, true).await;

    let mut package_names: Vec<&str> = outdated_packages
        .iter()
        .map(|pkg| pkg.id.as_str())
        .collect();

    if pkg == "all" && package_names.is_empty() {
        println!("No outdated packages found.");
        return;
    }

    if pkg != "all" {
        package_names = vec![pkg.as_str()];
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

    let exit_code = rocolatey_lib::run_choco(&choco_args, &package_names).await;

    if exit_code == 0 {
        println_verbose(&format!(
            "Successfully upgraded packages: {}",
            package_names.join(", ")
        ));
    } else {
        println_verbose(&format!(
            "Failed to upgrade packages: {} (exit code={})",
            package_names.join(", "),
            exit_code
        ));
    }

    std::process::exit(exit_code);
}
