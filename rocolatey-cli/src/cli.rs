extern crate clap;
use clap::{Command, Arg};

pub fn build_cli() -> Command<'static> {
  let common_arg_limitoutput = Arg::new("limitoutput")
    .short('r')
    .long("limitoutput")
    .help("limit the output to essential information");
  let common_arg_verbose = Arg::new("verbose")
    .short('v')
    .long("verbose")
    .help("be verbose");
  let common_arg_prerelease = Arg::new("prerelease")
    .short('p')
    .long("pre")
    .help("include prerelease versions");

  Command::new("Rocolatey")
    .version("0.5.4")
    .author("Manfred Wallner <schusterfredl@mwallner.net>")
    .override_help("provides a basic interface for rocolatey-lib")
    .subcommand(
      Command::new("list")
        .override_help("list local installed packages")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      Command::new("bad")
        .override_help("list packages in lib-bad/")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      Command::new("outdated")
        .override_help("Returns a list of outdated packages.")
        .arg(
          Arg::new("ignore-pinned")
            .long("ignore-pinned")
            .help("ignore any pinned packages"),
        )
        .arg(
          Arg::new("ignore-unfound")
            .long("ignore-unfound")
            .help("ignore any unfound packages"),
        )
        .arg(&common_arg_prerelease)
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      Command::new("source")
        .override_help("list choco sources")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
  /*
  .subcommand(
    Command::new("index")
      .about("crate package index")
      .arg(&common_arg_limitoutput)
      .arg(&common_arg_verbose)
      .arg(&common_arg_prerelease),
  )`*/
}
