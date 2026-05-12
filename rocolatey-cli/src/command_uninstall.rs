use rocolatey_lib::println_verbose;

pub async fn uninstall(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let mut choco_args = vec!["uninstall", "--ignore-http-cache", "-y"];

    if r {
        choco_args.push("-r");
    }

    if rocolatey_lib::is_verbose_mode() {
        choco_args.push("-v");
    }

    let exit_code = rocolatey_lib::run_choco(&choco_args, &[pkg.as_str()]).await;

    if exit_code == 0 {
        println_verbose(&format!("Successfully uninstalled package: {}", pkg));
    } else {
        println_verbose(&format!(
            "Failed to uninstall package: {} (exit code={})",
            pkg, exit_code
        ));
    }

    std::process::exit(exit_code);
}
