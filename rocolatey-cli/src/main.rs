extern crate clap;
mod cli;

use rocolatey_lib::roco::local::{
    get_local_bad_packages_text, get_local_packages_text, get_sources_text,
};
use rocolatey_lib::roco::remote::{get_outdated_packages, update_package_index};

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    match matches.subcommand() {
        Some(("list", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
            let r = matches.is_present("limitoutput");
            print!("{}", get_local_packages_text(r));
        }
        Some(("bad", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
            let r = matches.is_present("limitoutput");
            print!("{}", get_local_bad_packages_text(r));
        }
        Some(("index", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
            let r = matches.is_present("limitoutput");
            let pre = matches.is_present("prerelease");
            println!("{}", update_package_index(r, pre).await);
        }
        Some(("outdated", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
            let r = matches.is_present("limitoutput");
            let pre = matches.is_present("prerelease");
            let ignore_pinned = matches.is_present("ignore-pinned");
            let ignore_unfound = matches.is_present("ignore-unfound");
            let pkg  = matches.value_of("pkg").unwrap();
            print!(
                "{}",
                get_outdated_packages(pkg, r, pre, ignore_pinned, ignore_unfound).await
            );
        }
        Some(("source", matches)) => {
            rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
            let r = matches.is_present("limitoutput");
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
