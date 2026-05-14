use rocolatey_lib::roco::local::get_local_bad_packages;
use rocolatey_lib::roco::Package;
use rocolatey_lib::server::RocoServerPackagesResponse;

use crate::output_style;
use crate::server_contract::validate_schema_version;

pub async fn bad(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        bad_remote(r, json).await;
    } else {
        bad_local(r, json);
    }
}

async fn bad_remote(limitoutput: bool, json: bool) {
    // Always fetch structured JSON from server; use the shared local renderer for output.
    let res =
        rocolatey_lib::roco::roco_server::run_on_server_simple_get("/bad/json", "{}").await;

    match res {
        Some(s) => {
            if json {
                println!("{}", s);
                return;
            }
            match serde_json::from_str::<RocoServerPackagesResponse>(&s) {
                Ok(response) => {
                    if let Err(err) = validate_schema_version(response.schema_version, "/bad/json") {
                        eprintln!("{}", err);
                        return;
                    }
                    let packages = response.data;
                    let mode = if limitoutput {
                        output_style::ColorMode::Never
                    } else {
                        output_style::current()
                    };
                    print!("{}", render_bad_output(&packages, limitoutput, mode));
                }
                Err(e) => eprintln!("Error parsing bad-packages response from server: {}", e),
            }
        }
        None => eprintln!("Error fetching bad packages from server"),
    }
}

fn bad_local(r: bool, json: bool) {
    let packages = get_local_bad_packages().unwrap();

    if json {
        match serde_json::to_string(&packages) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    let mode = if r {
        output_style::ColorMode::Never
    } else {
        output_style::current()
    };

    print!("{}", render_bad_output(&packages, r, mode));
}

fn render_version_token(version: &str, mode: output_style::ColorMode) -> String {
    if version.contains('-') {
        output_style::version_prerelease(version, mode)
    } else {
        output_style::version_stable(version, mode)
    }
}

fn render_bad_output(
    packages: &[Package],
    limit_output: bool,
    mode: output_style::ColorMode,
) -> String {
    let mut res = String::new();
    let sep = if limit_output { "|" } else { " " };
    let num_packages = packages.len();

    for (i, p) in packages.iter().enumerate() {
        let version_token = render_version_token(p.version(), mode);
        let row = format!("{}{}{}", p.id(), sep, version_token);
        res.push_str(&output_style::package(&row, mode));
        if i < (num_packages - 1) {
            res.push_str("\r\n");
        }
    }

    if !limit_output {
        res.push_str(&format!(
            "\r\n{}",
            output_style::info(&format!("{} packages in lib-bad.", num_packages), mode)
        ));
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(id: &str, version: &str) -> Package {
        Package {
            id: id.to_string(),
            version: version.to_string(),
            pinned: false,
            dependencies: None,
        }
    }

    #[test]
    fn render_bad_output_normal_no_color() {
        let packages = vec![pkg("broken-pkg", "1.0.0"), pkg("other-bad", "0.5.0")];
        let out = render_bad_output(&packages, false, output_style::ColorMode::Never);
        assert!(out.contains("broken-pkg 1.0.0"));
        assert!(out.contains("other-bad 0.5.0"));
        assert!(out.contains("2 packages in lib-bad."));
    }

    #[test]
    fn render_bad_output_limit_no_color() {
        let packages = vec![pkg("broken-pkg", "1.0.0")];
        let out = render_bad_output(&packages, true, output_style::ColorMode::Never);
        assert!(out.contains("broken-pkg|1.0.0"));
        assert!(!out.contains("packages in lib-bad."));
    }
}
