extern crate clap;
use clap::{App, Arg, SubCommand};

fn main() {
    let matches = App::new("Rocolatey")
        .version("0.1.0")
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
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        let r = matches.is_present("limitoutput");
        println!("{}", rocolatey_lib::get_local_packages_text(r));
    }
    else if let Some(matches) = matches.subcommand_matches("bad") {
        let r = matches.is_present("limitoutput");
        println!("{}", rocolatey_lib::get_local_bad_packages_text(r));
    }
}
