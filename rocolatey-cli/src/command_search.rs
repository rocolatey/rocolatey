use rocolatey_lib::roco::{remote::find_packages, Package};
use rocolatey_lib::server::RocoServerPackagesResponse;
use rocolatey_lib::server::RocoServerSearchRequest;

use crate::output_style;
use crate::server_contract::validate_schema_version;
use crate::server_deny::print_deny_if_present;

pub async fn search(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        search_remote(r, json, pkg).await;
        return;
    }

    // convert the pkg String reference into a Vec<&str> as expected by find_packages
    let pkg_vec = vec![pkg.as_str()];
    let pkgs = find_packages(&pkg_vec, r, false).await;

    let pkgs = match pkgs {
        Ok(map) => map,
        Err(e) => {
            anstream::eprintln!("Error fetching packages: {}", e);
            return;
        }
    };

    if json {
        match serde_json::to_string(&pkgs) {
            Ok(s) => {
                anstream::println!("{}", s);
            }
            Err(e) => anstream::eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    if pkgs.is_empty() {
        anstream::println!("No packages found matching '{}'.", pkg);
        return;
    }

    let mode = if r {
        output_style::ColorMode::Never
    } else {
        output_style::current()
    };

    let mut sorted: Vec<_> = pkgs.values().collect();
    sorted.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));

    anstream::print!("{}", render_search_output(&sorted, r, mode));
}

async fn search_remote(r: bool, json: bool, pkg: &str) {
    let request_body = serde_json::to_string(&RocoServerSearchRequest {
        terms: vec![pkg.to_string()],
        prerelease: false,
    })
    .expect("serialize search request");

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_post(
        "/search/json",
        &request_body,
    )
    .await;

    match res {
        Some(s) => {
            if print_deny_if_present("/search/json", &s) {
                return;
            }
            if json {
                anstream::println!("{}", s);
                return;
            }
            match serde_json::from_str::<RocoServerPackagesResponse>(&s) {
                Ok(response) => {
                    if let Err(err) = validate_schema_version(response.schema_version, "/search/json") {
                        anstream::eprintln!("{}", err);
                        return;
                    }
                    let pkgs = response.data;
                    if pkgs.is_empty() {
                        anstream::println!("No packages found matching '{}'.", pkg);
                        return;
                    }
                    let mode = if r {
                        output_style::ColorMode::Never
                    } else {
                        output_style::current()
                    };
                    let sorted: Vec<_> = pkgs.iter().collect();
                    anstream::print!("{}", render_search_output(&sorted, r, mode));
                }
                Err(e) => anstream::eprintln!("Error parsing search response from server: {}", e),
            }
        }
        None => anstream::eprintln!("Error fetching search results from server"),
    }
}

fn render_search_output(sorted: &[&Package], r: bool, mode: output_style::ColorMode) -> String {
    let mut res = String::new();
    for p in sorted {
        let version_token = if p.version.contains('-') {
            output_style::version_prerelease(&p.version, mode)
        } else {
            output_style::version_stable(&p.version, mode)
        };
        let row = format!("{} [{}]", p.id, version_token);
        res.push_str(&output_style::package(&row, mode));
        res.push('\n');
    }
    if !r {
        res.push_str(&output_style::info(
            &format!("{} packages found.", sorted.len()),
            mode,
        ));
        res.push('\n');
    }
    res
}
