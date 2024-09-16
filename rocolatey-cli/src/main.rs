extern crate clap;
mod cli;

use rocolatey_lib::roco::local::{
    get_dependency_tree_text, get_local_bad_packages_text, get_local_packages_text,
    get_sources_text,
};
use rocolatey_lib::roco::remote::get_outdated_packages;

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    match matches.subcommand() {
        Some(("list", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
            let r = matches.get_flag("limitoutput");
            let filter = matches.get_one::<String>("filter").unwrap();
            if matches.get_flag("deptree") {
                print!("{}", get_dependency_tree_text(filter));
            } else {
                print!("{}", get_local_packages_text(filter, r));
            }
        }
        Some(("bad", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
            let r = matches.get_flag("limitoutput");
            print!("{}", get_local_bad_packages_text(r));
        }
        Some(("outdated", matches)) => {
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
        Some(("source", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
            let r = matches.get_flag("limitoutput");
            print!("{}", get_sources_text(r));
        }
        _ => {
            print!(
                "{}{}\n",
                cli::build_cli().render_long_version(),
                "Please run 'roco help' or 'roco <command> --help' for help menu."
            );
        }
    }
}
