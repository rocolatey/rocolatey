extern crate clap;
mod cli;

mod command_bad;
mod command_install;
mod command_license;
mod command_list;
mod command_outdated;
mod command_search;
mod command_source;
mod command_uninstall;
mod command_upgrade;

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    match matches.subcommand() {
        Some(("bad", matches)) => command_bad::bad(matches).await,
        Some(("install", matches)) => command_install::install(matches).await,
        Some(("license", matches)) => command_license::license(matches),
        Some(("list", matches)) => command_list::list(matches).await,
        Some(("outdated", matches)) => command_outdated::outdated(matches).await,
        Some(("source", matches)) => command_source::source(matches).await,
        Some(("search", matches)) => command_search::search(matches).await,
        Some(("uninstall", matches)) => command_uninstall::uninstall(matches).await,
        Some(("upgrade", matches)) => command_upgrade::upgrade(matches).await,
        _ => {
            if let Err(e) = cli::build_cli().print_help() {
                eprintln!("Error printing help: {}", e);
            }
            println!(); // Add a newline after the help text
        }
    }
    // todo newline after everything?
}
