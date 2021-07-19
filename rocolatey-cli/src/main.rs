extern crate clap;
mod cli;

use rocolatey_lib::roco::local::{
    get_local_bad_packages_text, get_local_packages_text, get_sources_text,
};
use rocolatey_lib::roco::remote::{get_outdated_packages, update_package_index};

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
        let r = matches.is_present("limitoutput");
        print!("{}", get_local_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("bad") {
        rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
        let r = matches.is_present("limitoutput");
        print!("{}", get_local_bad_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("index") {
        rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        println!("{}", update_package_index(r, pre).await);
    } else if let Some(matches) = matches.subcommand_matches("outdated") {
        rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        let ignore_pinned = matches.is_present("ignore-pinned");
        let ignore_unfound = matches.is_present("ignore-unfound");
        print!(
            "{}",
            get_outdated_packages(r, pre, ignore_pinned, ignore_unfound).await
        );
    } else if let Some(matches) = matches.subcommand_matches("source") {
        rocolatey_lib::set_verbose_mode(matches.is_present("verbose"));
        let r = matches.is_present("limitoutput");
        print!("{}", get_sources_text(r));
    }
}
