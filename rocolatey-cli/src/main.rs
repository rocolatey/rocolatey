extern crate clap;
use clap::{App, Arg, SubCommand};

use rocolatey_lib::roco::local::{
    get_local_bad_packages_text, get_local_packages_text, get_sources_text,
};
use rocolatey_lib::roco::remote::{get_outdated_packages, update_package_index};

#[tokio::main]
async fn main() {
    let matches = App::new("Rocolatey")
        .version("0.5.2-dev")
        .author("Manfred Wallner <schusterfredl@mwallner.net>")
        .about("provides a basic interface for rocolatey-lib")
        .subcommand(
            SubCommand::with_name("list")
                .about("list local installed packages")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                ),
        )
        .subcommand(
            SubCommand::with_name("bad")
                .about("list packages in lib-bad/")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                ),
        )
        .subcommand(
            SubCommand::with_name("outdated")
                .about("Returns a list of outdated packages.")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                )
                .arg(
                    Arg::with_name("prerelease")
                        .short("pre")
                        .help("include prerelease versions"),
                ),
        )
        .subcommand(
            SubCommand::with_name("source")
                .about("list choco sources")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                ),
        )
        /*.subcommand(
            SubCommand::with_name("index")
                .about("crate package index")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                )
                .arg(
                    Arg::with_name("prerelease")
                        .short("pre")
                        .help("include prerelease versions"),
                ),
        )*/
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        let r = matches.is_present("limitoutput");
        print!("{}", get_local_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("bad") {
        let r = matches.is_present("limitoutput");
        print!("{}", get_local_bad_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("index") {
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        println!("{}", update_package_index(r, pre).await);
    } else if let Some(matches) = matches.subcommand_matches("outdated") {
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        print!("{}", get_outdated_packages(r, pre).await);
    } else if let Some(matches) = matches.subcommand_matches("source") {
        let r = matches.is_present("limitoutput");
        print!("{}", get_sources_text(r));
    }
}
