use rocolatey_lib::roco::remote::find_packages;

pub async fn search(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    // convert the pkg String reference into a Vec<&str> as expected by find_packages
    let pkg_vec = vec![pkg.as_str()];
    let pkgs = find_packages(&pkg_vec, r, false).await;

    let pkgs = match pkgs {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error fetching packages: {}", e);
            return;
        }
    };

    if json {
        match serde_json::to_string(&pkgs) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    if pkgs.is_empty() {
        println!("No packages found matching '{}'.", pkg);
        return;
    }

    for (_, pkg) in pkgs {
        println!("{} [{}]", pkg.id, pkg.version);
    }
}
