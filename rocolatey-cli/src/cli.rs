extern crate clap;
use clap::{Arg, ArgAction, Command};

pub fn build_cli() -> Command {
    let common_arg_limitoutput = Arg::new("limitoutput")
        .short('r')
        .long("limitoutput")
        .action(ArgAction::SetTrue)
        .help("limit the output to essential information");
    let common_arg_verbose = Arg::new("verbose")
        .short('v')
        .long("verbose")
        .action(ArgAction::SetTrue)
        .help("be verbose");
    let common_arg_prerelease = Arg::new("prerelease")
        .short('p')
        .long("pre")
        .action(ArgAction::SetTrue)
        .help("include prerelease versions");
    let common_arg_enable_cert_validation: Arg = Arg::new("ssl-validation-enabled")
        .long("sslcheck")
        .action(ArgAction::SetTrue)
        .help("require https/ssl-validation");

    Command::new("Rocolatey")
    .version("0.9.3")
    .author("Manfred Wallner <schusterfredl@mwallner.net>")
    .about("provides a basic interface for rocolatey-lib")
    .subcommand(
      Command::new("list")
        .about("list local installed packages")
        .arg(Arg::new("filter").default_value("all"))
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose)
        .arg(Arg::new("deptree").long("dependency-tree").action(ArgAction::SetTrue).help("list dependencies")),
    )
    .subcommand(
      Command::new("bad")
        .about("list packages in lib-bad/")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      Command::new("outdated")
        .about("Returns a list of outdated packages.")
        .arg(
          Arg::new("pkg")
          .default_value("all")
        )
        .arg(
          Arg::new("choco-compat")
          .long("choco-mode")
          .action(ArgAction::SetTrue)
          .help("enables 'ignore-pinned' and 'ignore-unfound' \n(otherwise they are true by default, even if not set)")
        )
        .arg(
          Arg::new("ignore-pinned")
            .long("ignore-pinned")
            .action(ArgAction::SetTrue)
            .help("ignore any pinned packages \n(default, unless 'choco-mode' is set)"),
        )
        .arg(
          Arg::new("ignore-unfound")
            .long("ignore-unfound")
            .action(ArgAction::SetTrue)
            .help("ignore any unfound packages \n(default, unless 'choco-mode' is set)"),
        )
        .arg(
          Arg::new("listoutput")
            .short('l')
            .action(ArgAction::SetTrue)
            .help("output a whitespace-separated list of results"),
        )
        .arg(&common_arg_prerelease)
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose)
        .arg(&common_arg_enable_cert_validation),
    )
    .subcommand(
      Command::new("source")
        .about("list choco sources")
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose),
    )
    .subcommand(
      Command::new("license").about("display license information").arg(
        Arg::new("full")
          .short('f')
          .long("full")
          .action(ArgAction::SetTrue)
          .help("display full license information"),
      )
    )
    .subcommand(
      Command::new("upgrade").about("upgrade outdated choco packages (using choco.exe)")
        .arg(
          Arg::new("pkg")
          .default_value("all")
        )
        .arg(&common_arg_prerelease)
        .arg(&common_arg_limitoutput)
        .arg(&common_arg_verbose)
        .arg(&common_arg_enable_cert_validation),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        build_cli().debug_assert();
    }

    #[test]
    fn test_list_command() {
        let matches = build_cli()
            .try_get_matches_from(vec![
                "rocolatey",
                "list",
                "--dependency-tree",
                "--limitoutput",
                "--verbose",
            ])
            .unwrap();

        assert!(matches.subcommand_matches("list").is_some());
        let sub_matches = matches.subcommand_matches("list").unwrap();
        assert!(sub_matches.contains_id("deptree"));
        assert!(sub_matches.contains_id("limitoutput"));
        assert!(sub_matches.contains_id("verbose"));
    }

    #[test]
    fn test_source_command() {
        let matches = build_cli()
            .try_get_matches_from(vec!["rocolatey", "source", "--limitoutput", "--verbose"])
            .unwrap();

        assert!(matches.subcommand_matches("source").is_some());
        let sub_matches = matches.subcommand_matches("source").unwrap();
        assert!(sub_matches.contains_id("limitoutput"));
        assert!(sub_matches.contains_id("verbose"));
    }

    #[test]
    fn test_license_command() {
        let matches = build_cli()
            .try_get_matches_from(vec!["rocolatey", "license", "--full"])
            .unwrap();

        assert!(matches.subcommand_matches("license").is_some());
        let sub_matches = matches.subcommand_matches("license").unwrap();
        assert!(sub_matches.contains_id("full"));
    }

    #[test]
    fn test_upgrade_command() {
        let matches = build_cli()
            .try_get_matches_from(vec![
                "rocolatey",
                "upgrade",
                "--pre",
                "--limitoutput",
                "--verbose",
                "--sslcheck",
            ])
            .unwrap();

        assert!(matches.subcommand_matches("upgrade").is_some());
        let sub_matches = matches.subcommand_matches("upgrade").unwrap();
        assert_eq!(
            sub_matches
                .get_one::<String>("pkg")
                .map(|s| s.as_str())
                .unwrap(),
            "all"
        );
        assert!(sub_matches.contains_id("prerelease"));
        assert!(sub_matches.contains_id("limitoutput"));
        assert!(sub_matches.contains_id("verbose"));
        assert!(sub_matches.contains_id("ssl-validation-enabled"));
    }
}
