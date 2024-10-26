extern crate clap;
mod cli;

mod command_list;
mod command_bad;
mod command_license;
mod command_outdated;
mod command_source;

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    match matches.subcommand() {
        Some(("list", matches)) => command_list::list(matches),
        Some(("bad", matches)) => command_bad::bad(matches),
        Some(("license", matches)) => command_license::license(matches),
        Some(("outdated", matches)) => command_outdated::outdated(matches).await,
        Some(("source", matches)) => command_source::source(matches),
        _ => {
            if let Err(e) = cli::build_cli().print_help() {
                eprintln!("Error printing help: {}", e);
            }
            println!(); // Add a newline after the help text
        }
    }
}
