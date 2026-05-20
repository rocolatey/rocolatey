extern crate clap;
mod cli;
pub mod output_style;

mod command_bad;
mod command_install;
mod command_license;
mod command_list;
mod command_outdated;
mod command_search;
mod command_server;
mod command_source;
mod command_uninstall;
mod command_upgrade;
mod server_contract;
mod server_deny;

use output_style::ColorMode;

/// Resolve `ColorMode` from the parsed top-level matches.
/// CLI flag > NO_COLOR env var > CLICOLOR_FORCE env var > Auto.
fn color_mode_from_matches(matches: &clap::ArgMatches) -> ColorMode {
    if let Some(val) = matches.get_one::<String>("color") {
        match val.to_ascii_lowercase().as_str() {
            "always" => return ColorMode::Always,
            "never"  => return ColorMode::Never,
            _        => return ColorMode::Auto,
        }
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return ColorMode::Never;
    }
    if matches!(std::env::var("CLICOLOR_FORCE").as_deref(), Ok("1")) {
        return ColorMode::Always;
    }
    ColorMode::Auto
}

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    // Resolve and activate color mode before any output.
    let color_mode = color_mode_from_matches(&matches);
    output_style::init(color_mode);

    match matches.subcommand() {
        Some(("bad", matches)) => command_bad::bad(matches).await,
        Some(("install", matches)) => command_install::install(matches).await,
        Some(("license", matches)) => command_license::license(matches),
        Some(("list", matches)) => command_list::list(matches).await,
        Some(("outdated", matches)) => command_outdated::outdated(matches).await,
        Some(("source", matches)) => command_source::source(matches).await,
        Some(("search", matches)) => command_search::search(matches).await,
        Some(("server", matches)) => command_server::server(matches).await,
        Some(("uninstall", matches)) => command_uninstall::uninstall(matches).await,
        Some(("upgrade", matches)) => command_upgrade::upgrade(matches).await,
        _ => {
            if let Err(e) = cli::build_cli().print_help() {
                anstream::eprintln!("Error printing help: {}", e);
            }
            anstream::println!(); // Add a newline after the help text
        }
    }
    // todo newline after everything?
}
