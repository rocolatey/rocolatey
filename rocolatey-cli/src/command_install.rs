use rocolatey_lib::println_verbose;

pub async fn install(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    rocolatey_lib::set_ssl_enabled(matches.get_flag("ssl-validation-enabled"));
    let r = matches.get_flag("limitoutput");
    let pre = matches.get_flag("prerelease");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let mut choco_args = vec!["install", "--ignore-http-cache", "-y"];

    if pre {
        choco_args.push("--pre");
    }

    if r {
        choco_args.push("-r");
    }

    if rocolatey_lib::is_verbose_mode() {
        choco_args.push("-v");
    }

    let exit_code = rocolatey_lib::run_choco(&choco_args, &[pkg.as_str()]).await;

    if exit_code == 0 {
        println_verbose(&format!("Successfully installed: {}", pkg));
    } else {
        println_verbose(&format!(
            "Failed to install package: {} (exit code={})",
            pkg, exit_code
        ));
    }

    std::process::exit(exit_code);
}
