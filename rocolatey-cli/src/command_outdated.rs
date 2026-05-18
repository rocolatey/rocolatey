use rocolatey_lib::roco::{remote::get_outdated_packages, OutdatedInfo};
use rocolatey_lib::server::RocoServerOutdatedRequest;
use rocolatey_lib::server::RocoServerOutdatedResponse;

use crate::output_style;
use crate::server_contract::validate_schema_version;
use crate::server_deny::print_deny_if_present;

pub async fn outdated(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    rocolatey_lib::set_ssl_enabled(matches.get_flag("ssl-validation-enabled"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");
    let l: bool = matches.get_flag("listoutput");
    let pre = matches.get_flag("prerelease");
    let choco_compat = matches.get_flag("choco-compat");
    let ignore_pinned = !choco_compat || matches.get_flag("ignore-pinned");
    let ignore_unfound = !choco_compat || matches.get_flag("ignore-unfound");
    let pkg = matches.get_one::<String>("pkg").unwrap();

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        outdated_remote(r, json, l, pre, ignore_pinned, ignore_unfound, pkg).await;
    } else {
        outdated_local(r, json, l, pre, ignore_pinned, ignore_unfound, pkg).await;
    }
}

async fn outdated_remote(
    r: bool,
    json: bool,
    l: bool,
    pre: bool,
    ignore_pinned: bool,
    ignore_unfound: bool,
    pkg: &String,
) {
    let request_body = serde_json::to_string(&RocoServerOutdatedRequest {
        pkg: pkg.clone(),
        pre,
        ignore_pinned,
        ignore_unfound,
    })
    .expect("serialize outdated request");

    // Always fetch structured JSON from server; use the same local renderer for output.
    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_post(
        "/outdated/json",
        &request_body,
    )
    .await;

    match res {
        Some(s) => {
            if print_deny_if_present("/outdated/json", &s) {
                return;
            }
            if json {
                println!("{}", s);
                return;
            }
            match serde_json::from_str::<RocoServerOutdatedResponse>(&s) {
                Ok(response) => {
                    if let Err(err) = validate_schema_version(response.schema_version, "/outdated/json") {
                        eprintln!("{}", err);
                        return;
                    }
                    let outdated_pkgs = response.data;
                    let warning_count = outdated_pkgs
                        .iter()
                        .filter(|o| !o.exists_on_remote)
                        .count() as i32;
                    let mode = if r {
                        output_style::ColorMode::Never
                    } else {
                        output_style::current()
                    };
                    print!("{}", render_outdated_output(warning_count, &outdated_pkgs, r, l, mode));
                }
                Err(e) => eprintln!("Error parsing outdated response from server: {}", e),
            }
        }
        None => eprintln!("Error fetching outdated packages from server"),
    }
}

async fn outdated_local(
    r: bool,
    json: bool,
    l: bool,
    pre: bool,
    ignore_pinned: bool,
    ignore_unfound: bool,
    pkg: &String,
) {
    if json {
        let (_, outdated_pkgs) =
            get_outdated_packages(pkg, r, pre, ignore_pinned, ignore_unfound).await;

        match serde_json::to_string(&outdated_pkgs) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    let (warning_count, outdated_pkgs) =
        get_outdated_packages(pkg, r, pre, ignore_pinned, ignore_unfound).await;

    // Keep automation mode uncolored (`-r`) while enabling interactive color mode.
    let mode = if r {
        output_style::ColorMode::Never
    } else {
        output_style::current()
    };
    print!("{}", render_outdated_output(warning_count, &outdated_pkgs, r, l, mode));
}

fn render_outdated_output(
    warning_count: i32,
    outdated_pkgs: &[OutdatedInfo],
    limit_output: bool,
    list_output: bool,
    mode: output_style::ColorMode,
) -> String {
    let mut warnings = String::new();
    let mut res = String::new();

    if !limit_output {
        res.push_str(&output_style::header("Outdated Packages", mode));
        res.push('\n');
        if !list_output {
            res.push_str(&output_style::info(
                " Output is package name | current version | available version | pinned?",
                mode,
            ));
            res.push_str("\n\n");
        }
    }

    let mut outdated_packages = 0;
    for o in outdated_pkgs {
        if o.outdated {
            outdated_packages += 1;
        }
        if list_output {
            res.push_str(&output_style::package(&format!("{} ", o.id), mode));
        } else {
            let remote_version = render_version_token(&o.remote_version, mode);
            let row = format!(
                "{}|{}|{}|{}",
                o.id, o.local_version, remote_version, o.pinned
            );
            res.push_str(&output_style::package(&row, mode));
            res.push('\n');
        }
        if !o.exists_on_remote {
            warnings.push_str(&output_style::warning(&format!(" - {}", o.id), mode));
            warnings.push('\n');
        }
    }

    if !limit_output {
        res.push('\n');
        res.push_str(&output_style::info(
            &format!(
                "Rocolatey has determined {} package(s) are outdated.",
                outdated_packages
            ),
            mode,
        ));
        res.push('\n');

        if warning_count > 0 {
            res.push_str(&output_style::warning(
                &format!(" {} package(s) had warnings.", warning_count),
                mode,
            ));
            res.push('\n');
            res.push_str(&output_style::warning("Warnings:", mode));
            res.push('\n');
            res.push_str(&warnings);
        }
    }

    res
}

fn render_version_token(remote_version: &str, mode: output_style::ColorMode) -> String {
    if remote_version.contains('-') {
        output_style::version_prerelease(remote_version, mode)
    } else {
        output_style::version_stable(remote_version, mode)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn oi(
        id: &str,
        local_version: &str,
        remote_version: &str,
        outdated: bool,
        exists_on_remote: bool,
    ) -> OutdatedInfo {
        OutdatedInfo {
            id: id.to_string(),
            local_version: local_version.to_string(),
            remote_version: remote_version.to_string(),
            pinned: false,
            outdated,
            exists_on_remote,
        }
    }

    #[test]
    fn renders_full_output_with_warning_text_identical() {
        let pkgs = vec![
            oi("alpha", "1.0.0", "1.1.0", true, true),
            oi("beta", "2.0.0", "2.0.0", false, false),
        ];

        let out = render_outdated_output(1, &pkgs, false, false, output_style::ColorMode::Never);

        assert_eq!(
            out,
            "Outdated Packages\n Output is package name | current version | available version | pinned?\n\nalpha|1.0.0|1.1.0|false\nbeta|2.0.0|2.0.0|false\n\nRocolatey has determined 1 package(s) are outdated.\n 1 package(s) had warnings.\nWarnings:\n - beta\n"
        );
    }

    #[test]
    fn renders_limit_list_output_compact() {
        let pkgs = vec![
            oi("alpha", "1.0.0", "1.1.0", true, true),
            oi("beta", "2.0.0", "2.0.0", false, true),
        ];

        let out = render_outdated_output(0, &pkgs, true, true, output_style::ColorMode::Never);
        assert_eq!(out, "alpha beta ");
    }

    #[test]
    fn remote_version_token_uses_stable_style() {
        let token = render_version_token("2.0.0", output_style::ColorMode::Always);
        assert!(token.contains("2.0.0"));
        assert!(token.contains('\x1b'));
        assert_eq!(
            token,
            output_style::version_stable("2.0.0", output_style::ColorMode::Always)
        );
    }

    #[test]
    fn remote_version_token_uses_prerelease_style() {
        let token = render_version_token("2.0.0-beta1", output_style::ColorMode::Always);
        assert!(token.contains("2.0.0-beta1"));
        assert!(token.contains('\x1b'));
        assert_eq!(
            token,
            output_style::version_prerelease("2.0.0-beta1", output_style::ColorMode::Always)
        );
    }

    #[test]
    fn limit_output_rows_remain_plain_even_when_mode_always() {
        let pkgs = vec![oi("alpha", "1.0.0", "1.1.0-beta", true, true)];

        let out = render_outdated_output(0, &pkgs, true, false, output_style::ColorMode::Never);
        assert_eq!(out, "alpha|1.0.0|1.1.0-beta|false\n");
        assert!(!out.contains('\x1b'));
    }
}
