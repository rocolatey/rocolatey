use rocolatey_lib::roco::{get_choco_sources, Feed};
use rocolatey_lib::server::RocoServerFeedsResponse;

use crate::output_style;
use crate::server_contract::validate_schema_version;
use crate::server_deny::print_deny_if_present;

pub async fn source(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        source_remote(r, json).await;
    } else {
        source_local(r, json);
    }
}

async fn source_remote(limitoutput: bool, json: bool) {
    // Always fetch structured JSON from server; use the shared local renderer for output.
    let res =
        rocolatey_lib::roco::roco_server::run_on_server_simple_get("/source/json", "{}").await;

    match res {
        Some(s) => {
            if print_deny_if_present("/source/json", &s) {
                return;
            }
            if json {
                println!("{}", s);
                return;
            }
            match serde_json::from_str::<RocoServerFeedsResponse>(&s) {
                Ok(response) => {
                    if let Err(err) = validate_schema_version(response.schema_version, "/source/json") {
                        eprintln!("{}", err);
                        return;
                    }
                    let sources = response.data;
                    let mode = if limitoutput {
                        output_style::ColorMode::Never
                    } else {
                        output_style::current()
                    };
                    print!("{}", render_sources_output(&sources, limitoutput, mode));
                }
                Err(e) => eprintln!("Error parsing sources response from server: {}", e),
            }
        }
        None => eprintln!("Error fetching sources from server"),
    }
}

fn source_local(limitoutput: bool, json: bool) {
    let sources = get_choco_sources().unwrap();

    if json {
        match serde_json::to_string(&sources) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }

    let mode = if limitoutput {
        output_style::ColorMode::Never
    } else {
        output_style::current()
    };

    print!("{}", render_sources_output(&sources, limitoutput, mode));
}

fn c_bool(v: bool) -> &'static str {
    match v {
        true => "True",
        false => "False",
    }
}

fn render_sources_output(
    sources: &[Feed],
    limit_output: bool,
    mode: output_style::ColorMode,
) -> String {
    let mut res = String::new();
    let num_iterations = sources.len().saturating_sub(1);

    for (i, f) in sources.iter().enumerate() {
        let row = if limit_output {
            let user = match &f.credential {
                Some(auth) => auth.user.clone(),
                None => String::new(),
            };
            let certificate = (f.certificate.as_ref().unwrap_or(&String::new())).clone();
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                f.name,
                f.url,
                c_bool(f.disabled),
                user,
                certificate,
                f.priority,
                c_bool(f.bypass_proxy),
                c_bool(f.self_service),
                c_bool(f.admin_only),
            )
        } else {
            let name_display = match f.disabled {
                true => format!("{} [Disabled]", f.name),
                false => f.name.clone(),
            };
            let url_display = match &f.credential {
                Some(_) => format!("{} (Authenticated)", f.url),
                None => format!("{} ", f.url),
            };
            format!(
                "{} - {}| Priority {}|Bypass Proxy - {}|Self-Service - {}|Admin Only - {}.",
                name_display,
                url_display,
                f.priority,
                c_bool(f.bypass_proxy),
                c_bool(f.self_service),
                c_bool(f.admin_only)
            )
        };

        let styled_row = if f.disabled {
            output_style::warning(&row, mode)
        } else {
            output_style::package(&row, mode)
        };

        res.push_str(&styled_row);
        if i < num_iterations {
            res.push_str("\r\n");
        }
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocolatey_lib::roco::FeedType;

    fn make_feed(name: &str, url: &str, disabled: bool) -> Feed {
        Feed {
            name: name.to_string(),
            url: url.to_string(),
            credential: None,
            proxy: None,
            disabled,
            certificate: None,
            bypass_proxy: false,
            self_service: false,
            admin_only: false,
            priority: 0,
            feed_type: FeedType::NuGetV2,
            service_index: None,
        }
    }

    #[test]
    fn render_sources_normal_no_color() {
        let sources = vec![
            make_feed("chocolatey", "https://community.chocolatey.org/api/v2/", false),
            make_feed("internal", "https://internal.example.com/", true),
        ];
        let out = render_sources_output(&sources, false, output_style::ColorMode::Never);
        assert!(out.contains("chocolatey - https://community.chocolatey.org/api/v2/"));
        assert!(out.contains("internal [Disabled]"));
        assert!(out.contains("Priority 0"));
    }

    #[test]
    fn render_sources_limit_no_color() {
        let sources = vec![make_feed(
            "chocolatey",
            "https://community.chocolatey.org/api/v2/",
            false,
        )];
        let out = render_sources_output(&sources, true, output_style::ColorMode::Never);
        assert!(out.contains("chocolatey|https://community.chocolatey.org/api/v2/|False"));
    }

    #[test]
    fn render_sources_disabled_no_color() {
        let sources = vec![make_feed("bad-source", "https://dead.example.com/", true)];
        let out = render_sources_output(&sources, false, output_style::ColorMode::Never);
        assert!(out.contains("bad-source [Disabled]"));
    }
}
