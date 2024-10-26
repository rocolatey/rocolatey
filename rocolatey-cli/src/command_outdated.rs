use rocolatey_lib::roco::remote::get_outdated_packages;


pub async fn outdated(matches: &clap::ArgMatches) {
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
