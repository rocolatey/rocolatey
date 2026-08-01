use rocolatey_lib::roco::pin;

use crate::output_style;

pub async fn pin(matches: &clap::ArgMatches) {
    match matches.subcommand() {
        Some(("list", sub_matches)) => cmd_list(sub_matches).await,
        Some(("add", sub_matches)) => cmd_add(sub_matches).await,
        Some(("remove", sub_matches)) => cmd_remove(sub_matches).await,
        _ => {
            anstream::eprintln!("Unknown pin subcommand. Use 'roco pin --help'.");
        }
    }
}

async fn cmd_list(matches: &clap::ArgMatches) {
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");

    let pinned = pin::list_pins();

    if json {
        match serde_json::to_string(&pinned) {
            Ok(s) => anstream::println!("{}", s),
            Err(e) => anstream::eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    let mode = if r {
        output_style::ColorMode::Never
    } else {
        output_style::current()
    };

    if pinned.is_empty() {
        if !r {
            anstream::println!("{}", output_style::info("No pinned packages.", mode));
        }
        return;
    }

    for (i, p) in pinned.iter().enumerate() {
        let line = match &p.version {
            Some(v) => format!("{}|{}", p.id, v),
            None => format!("{}|(all versions)", p.id),
        };
        anstream::print!("{}", output_style::package(&line, mode));
        if i < pinned.len() - 1 {
            anstream::println!();
        }
    }
    anstream::println!();

    if !r {
        let total = pinned.len();
        anstream::println!(
            "{}",
            output_style::info(&format!("{} pinned packages.", total), mode)
        );
    }
}

async fn cmd_add(matches: &clap::ArgMatches) {
    let pkg = matches.get_one::<String>("pkg").unwrap();
    let version = matches.get_one::<String>("version");

    match pin::add_pin(pkg, version.map(String::as_str)) {
        Ok(()) => {
            match version {
                Some(v) => anstream::println!("Pinned {} version {}.", pkg, v),
                None => anstream::println!("Pinned {} (all versions).", pkg),
            }
        }
        Err(e) => {
            anstream::eprintln!("Failed to pin {}: {}", pkg, e);
            std::process::exit(1);
        }
    }
}

async fn cmd_remove(matches: &clap::ArgMatches) {
    let pkg = matches.get_one::<String>("pkg").unwrap();
    let version = matches.get_one::<String>("version");

    match pin::remove_pin(pkg, version.map(String::as_str)) {
        Ok(()) => {
            match version {
                Some(v) => anstream::println!("Removed pin for {} version {}.", pkg, v),
                None => anstream::println!("Removed pin for {} (all versions).", pkg),
            }
        }
        Err(e) => {
            anstream::eprintln!("Failed to remove pin for {}: {}", pkg, e);
            std::process::exit(1);
        }
    }
}
