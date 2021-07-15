extern crate clap;
use clap::{App, Arg};

pub fn build_cli() -> App<'static> {
  App::new("Rocolatey")
    .version("0.5.4")
    .author("Manfred Wallner <schusterfredl@mwallner.net>")
    .about("provides a basic interface for rocolatey-lib")
    .subcommand(App::new("generate-bash-completions").about("create bash completions"))
    .subcommand(App::new("generate-pwsh-completions").about("create powershell completions"))
    .subcommand(
      App::new("list")
        .about("list local installed packages")
        .arg(
          Arg::new("limitoutput")
            .short('r')
            .long("limitoutput")
            .about("limit the output to essential information"),
        )
        .arg(
          Arg::new("verbose")
            .short('v')
            .long("verbose")
            .about("be verbose"),
        ),
    )
    .subcommand(
      App::new("bad")
        .about("list packages in lib-bad/")
        .arg(
          Arg::new("limitoutput")
            .short('r')
            .long("limitoutput")
            .about("limit the output to essential information"),
        )
        .arg(
          Arg::new("verbose")
            .short('v')
            .long("verbose")
            .about("be verbose"),
        ),
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
        .arg(
          Arg::new("limitoutput")
            .short('r')
            .long("limitoutput")
            .about("limit the output to essential information"),
        )
        .arg(
          Arg::new("prerelease")
            .short('p')
            .long("pre")
            .about("include prerelease versions"),
        )
        .arg(
          Arg::new("verbose")
            .short('v')
            .long("verbose")
            .about("be verbose"),
        ),
    )
    .subcommand(
      App::new("source")
        .about("list choco sources")
        .arg(
          Arg::new("limitoutput")
            .short('r')
            .long("limitoutput")
            .about("limit the output to essential information"),
        )
        .arg(
          Arg::new("verbose")
            .short('v')
            .long("verbose")
            .about("be verbose"),
        ),
    )
  /*.subcommand(
      App::new("index")
          .about("crate package index")
          .arg(
              Arg::new("limitoutput")
                  .short("r")
                  .help("limit the output to essential information"),
          )
          .arg(
              Arg::new("prerelease")
                  .short("pre")
                  .help("include prerelease versions"),
          ),
  )*/
}
