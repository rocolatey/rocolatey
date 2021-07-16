extern crate clap;
use clap::{App, Arg};

pub fn build_cli() -> App<'static> {
  let common_arg_limitoutput = Arg::new("limitoutput")
    .short('r')
    .long("limitoutput")
    .about("limit the output to essential information");
  let common_arg_verbose = Arg::new("verbose")
    .short('v')
    .long("verbose")
    .about("be verbose");
  let common_arg_prerelease = Arg::new("prerelease")
    .short('p')
    .long("pre")
    .about("include prerelease versions");

  App::new("Rocolatey")
    .version("0.5.4")
    .author("Manfred Wallner <schusterfredl@mwallner.net>")
    .about("provides a basic interface for rocolatey-lib")
    .subcommand(
      App::new("generate-shell-completions")
        .about("create tab completions for various shell environments")
        .arg(
          Arg::new("powershell")
            .long("powershell")
            .about("create powershell tab completions"),
        )
        .arg(
          Arg::new("bash")
            .long("bash")
            .about("create bash tab completions"),
        )
        .arg(
          Arg::new("zsh")
            .long("zsh")
            .about("create zsh tab completions"),
        ),
    )
    .subcommand(
      App::new("list")
        .about("list local installed packages")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      App::new("bad")
        .about("list packages in lib-bad/")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      App::new("outdated")
        .about("Returns a list of outdated packages.")
        .arg(
          Arg::new("ignore-pinned")
            .long("ignore-pinned")
            .about("ignore any pinned packages"),
        )
        .arg(
          Arg::new("ignore-unfound")
            .long("ignore-unfound")
            .about("ignore any unfound packages"),
        )
        .arg(&common_arg_prerelease)
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      App::new("source")
        .about("list choco sources")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
  /*
  .subcommand(
    App::new("index")
      .about("crate package index")
      .arg(&common_arg_limitoutput)
      .arg(&common_arg_verbose)
      .arg(&common_arg_prerelease),
  )`*/
}
