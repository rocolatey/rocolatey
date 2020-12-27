extern crate clap;
use clap::{App, Arg, SubCommand};

#[tokio::main]
async fn main() {
    let matches = App::new("Rocolatey")
        .version("0.2.0")
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
            SubCommand::with_name("update")
                .about("update package index")
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
            SubCommand::with_name("outdated")
                .about("compare local installed package versions with package index")
                .arg(
                    Arg::with_name("limitoutput")
                        .short("r")
                        .help("limit the output to essential information"),
                )
                .arg(
                    Arg::with_name("fetch")
                        .short("f")
                        .help("fetch remote package index ('update')"),
                )
                .arg(
                    Arg::with_name("prerelease")
                        .short("pre")
                        .help("include prerelease versions"),
                ),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        let r = matches.is_present("limitoutput");
        println!("{}", rocolatey_lib::get_local_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("bad") {
        let r = matches.is_present("limitoutput");
        println!("{}", rocolatey_lib::get_local_bad_packages_text(r));
    } else if let Some(matches) = matches.subcommand_matches("update") {
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        println!("{}", rocolatey_lib::update_package_index(r, pre).await);
    } else if let Some(matches) = matches.subcommand_matches("outdated") {
        let r = matches.is_present("limitoutput");
        let pre = matches.is_present("prerelease");
        if matches.is_present("fetch") {
            println!("{}", rocolatey_lib::update_package_index(r, pre).await);
        }
        println!("{}", rocolatey_lib::get_outdated_packages(r, pre).await);
    }
}
