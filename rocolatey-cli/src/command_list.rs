use rocolatey_lib::roco::local::{get_dependency_tree_text, get_local_packages};
use rocolatey_lib::roco::Package;
use rocolatey_lib::server::DependencyTreeNode;
use rocolatey_lib::server::RocoServerDependencyTreeResponse;
use rocolatey_lib::server::RocoServerListRequest;
use rocolatey_lib::server::RocoServerPackagesResponse;

use crate::output_style;
use crate::server_contract::validate_schema_version;
use crate::server_deny::print_deny_if_present;

pub async fn list(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");
    let filter = matches.get_one::<String>("filter").unwrap();

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        if matches.get_flag("deptree") {
            list_remote_deptree(r, json, filter).await;
        } else {
            list_remote(r, json, filter).await;
        }
    } else {
        list_local(matches, r, json, filter);
    }
}

async fn list_remote(limitoutput: bool, json: bool, filter: &str) {
    let request_body = serde_json::to_string(&RocoServerListRequest {
        filter: filter.to_string(),
    })
    .expect("serialize list request");

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_post(
        "/local/json",
        &request_body,
    )
    .await;
    match res {
        Some(s) => {
            if print_deny_if_present("/local/json", &s) {
                return;
            }
            if json {
                anstream::println!("{}", s);
                return;
            }
            match serde_json::from_str::<RocoServerPackagesResponse>(&s) {
                Ok(response) => {
                    if let Err(err) = validate_schema_version(response.schema_version, "/local/json") {
                        anstream::eprintln!("{}", err);
                        return;
                    }
                    let packages = response.data;
                    let total_pkgs = response.total_count.unwrap_or(packages.len());
                    let mode = if limitoutput {
                        output_style::ColorMode::Never
                    } else {
                        output_style::current()
                    };
                    anstream::print!("{}", render_list_output(&packages, total_pkgs, limitoutput, mode));
                }
                Err(e) => anstream::eprintln!("Error parsing list response from server: {}", e),
            }
        }
        None => anstream::eprintln!("Error fetching packages from server"),
    }
}

async fn list_remote_deptree(limitoutput: bool, json: bool, filter: &str) {
    let request_body = serde_json::to_string(&RocoServerListRequest {
        filter: filter.to_string(),
    })
    .expect("serialize dependency-tree request");

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_post(
        "/local/deptree/json",
        &request_body,
    )
    .await;
    match res {
        Some(s) => {
            if print_deny_if_present("/local/deptree/json", &s) {
                return;
            }
            if json {
                anstream::println!("{}", s);
                return;
            }
            let response = match serde_json::from_str::<RocoServerDependencyTreeResponse>(&s) {
                Ok(response) => response,
                Err(e) => {
                    anstream::eprintln!("Error parsing dependency tree response from server: {}", e);
                    return;
                }
            };
            if let Err(err) = validate_schema_version(response.schema_version, "/local/deptree/json") {
                anstream::eprintln!("{}", err);
                return;
            }
            let tree_text = dependency_tree_nodes_to_text(&response.data);
            let mode = if limitoutput {
                output_style::ColorMode::Never
            } else {
                output_style::current()
            };
            anstream::print!("{}", render_dependency_tree_output(&tree_text, mode));
        }
        None => anstream::eprintln!("Error fetching dependency tree from server"),
    }
}

fn list_local(matches: &clap::ArgMatches, r: bool, json: bool, filter: &String) {
    if matches.get_flag("deptree") {
        let mode = if r {
            output_style::ColorMode::Never
        } else {
            output_style::current()
        };

        let tree_text = get_dependency_tree_text(filter);
        anstream::print!("{}", render_dependency_tree_output(&tree_text, mode));
        return;
    }

    let (packages, total_pkgs) = get_local_packages(filter).unwrap();

    if json {
        match serde_json::to_string(&packages) {
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

    anstream::print!("{}", render_list_output(&packages, total_pkgs, r, mode));
}

fn render_version_token(version: &str, mode: output_style::ColorMode) -> String {
    if version.contains('-') {
        output_style::version_prerelease(version, mode)
    } else {
        output_style::version_stable(version, mode)
    }
}

fn render_dependency_tree_output(tree_text: &str, mode: output_style::ColorMode) -> String {
    let mut out = String::new();
    let mut group_idx: usize = 0;

    for raw_line in tree_text.split_inclusive('\n') {
        let (line, newline) = if let Some(stripped) = raw_line.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = raw_line.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (raw_line, "")
        };

        if is_dependency_tree_root_line(line) {
            group_idx = group_idx.saturating_add(1);
        }

        let styled_line = if line.starts_with("ERROR:") {
            output_style::error(line, mode)
        } else {
            style_tree_line_with_version(line, mode, group_idx)
        };

        out.push_str(&styled_line);
        out.push_str(newline);
    }

    out
}

fn is_dependency_tree_root_line(line: &str) -> bool {
    !line.is_empty() && !line.starts_with(" |") && !line.starts_with("ERROR:")
}

fn style_tree_line_with_version(
    line: &str,
    mode: output_style::ColorMode,
    group_idx: usize,
) -> String {
    if let Some((prefix, version, suffix)) = split_line_trailing_parenthesized_token(line) {
        let version_token = render_version_token(version, mode);
        return format!(
            "{}({}){}",
            output_style::dependency_group(prefix, group_idx, mode),
            version_token,
            output_style::dependency_group(suffix, group_idx, mode)
        );
    }

    output_style::dependency_group(line, group_idx, mode)
}

fn split_line_trailing_parenthesized_token(line: &str) -> Option<(&str, &str, &str)> {
    let end = line.rfind(')')?;
    if end != line.len() - 1 {
        return None;
    }

    let start = line[..end].rfind('(')?;
    let version = &line[start + 1..end];
    if version.is_empty() {
        return None;
    }

    Some((&line[..start], version, &line[end + 1..]))
}

fn dependency_tree_nodes_to_text(nodes: &[DependencyTreeNode]) -> String {
    let mut out = String::new();

    for node in nodes {
        if node.depth == 0 {
            out.push_str(&format!("{} ({})\r\n", node.id, node.version));
            continue;
        }

        let version = if node.version.is_empty() {
            String::new()
        } else {
            format!("({})", node.version)
        };
        out.push_str(&format!("{}-{} {}\r\n", " |".repeat(node.depth), node.id, version));
        if node.missing {
            out.push_str(&format!(
                "ERROR: failed to locate {} among local packages\r\n",
                node.id
            ));
        }
    }

    out
}

fn render_list_output(
    packages: &[Package],
    total_pkgs: usize,
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
            output_style::info(&format!("{} packages installed.", total_pkgs), mode)
        ));
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocolatey_lib::server::RocoServerPackagesResponse;

    fn pkg(id: &str, version: &str) -> Package {
        Package {
            id: id.to_string(),
            version: version.to_string(),
            pinned: false,
            dependencies: None,
        }
    }

    #[test]
    fn render_list_output_normal_no_color() {
        let packages = vec![pkg("foo", "1.0.0"), pkg("bar", "2.1.0")];
        let out = render_list_output(&packages, 5, false, output_style::ColorMode::Never);
        assert!(out.contains("foo 1.0.0"));
        assert!(out.contains("bar 2.1.0"));
        assert!(out.contains("5 packages installed."));
    }

    #[test]
    fn render_list_output_limit_no_color() {
        let packages = vec![pkg("foo", "1.0.0")];
        let out = render_list_output(&packages, 1, true, output_style::ColorMode::Never);
        assert!(out.contains("foo|1.0.0"));
        assert!(!out.contains("packages installed."));
    }

    #[test]
    fn render_list_output_prerelease_version_plain() {
        let packages = vec![pkg("foo", "1.0.0-beta")];
        let out = render_list_output(&packages, 1, false, output_style::ColorMode::Never);
        assert!(out.contains("foo 1.0.0-beta"));
    }

    #[test]
    fn render_dependency_tree_output_plain_when_no_color() {
        let input = "foo (1.0.0)\r\n |-bar (2.0.0-beta)\r\nERROR: failed\r\n";
        let out = render_dependency_tree_output(input, output_style::ColorMode::Never);
        assert_eq!(out, input);
    }

    #[test]
    fn render_dependency_tree_output_colored_when_enabled() {
        let input = "foo (1.0.0)\r\n |-bar (2.0.0-beta)\r\n";
        let out = render_dependency_tree_output(input, output_style::ColorMode::Always);
        assert!(out.contains("foo"));
        assert!(out.contains("bar"));
        assert!(out.contains('\x1b'));
    }

    #[test]
    fn render_dependency_tree_output_groups_root_and_children() {
        let input = "alpha (1.0.0)\r\n |-beta (2.0.0)\r\ngamma (3.0.0)\r\n |-delta (4.0.0)\r\n";
        let out = render_dependency_tree_output(input, output_style::ColorMode::Always);

        // Extract the ANSI escape code that immediately precedes a given word.
        fn color_before<'a>(s: &'a str, word: &str) -> &'a str {
            let pos = s.find(word).expect("word not found");
            let prefix = &s[..pos];
            let esc_start = prefix.rfind('\x1b').expect("no ANSI code before word");
            let rel_end = prefix[esc_start..].find('m').expect("malformed ANSI") + 1;
            &prefix[esc_start..esc_start + rel_end]
        }

        let alpha_color = color_before(&out, "alpha");
        let beta_color = color_before(&out, "beta");
        let gamma_color = color_before(&out, "gamma");

        // alpha and beta are in the same group; gamma starts a new group.
        assert_eq!(alpha_color, beta_color, "alpha and beta should share a color group");
        assert_ne!(alpha_color, gamma_color, "gamma should use a different color group");
    }

    #[test]
    fn dependency_tree_nodes_round_trip_to_existing_renderer_input() {
        let nodes = vec![
            DependencyTreeNode {
                id: "alpha".to_string(),
                version: "1.0.0".to_string(),
                depth: 0,
                parent_id: None,
                missing: false,
            },
            DependencyTreeNode {
                id: "beta".to_string(),
                version: "2.0.0".to_string(),
                depth: 1,
                parent_id: Some("alpha".to_string()),
                missing: false,
            },
            DependencyTreeNode {
                id: "missing".to_string(),
                version: String::new(),
                depth: 1,
                parent_id: Some("alpha".to_string()),
                missing: true,
            },
        ];

        let tree_text = dependency_tree_nodes_to_text(&nodes);

        assert!(tree_text.contains("alpha (1.0.0)"));
        assert!(tree_text.contains(" |-beta (2.0.0)"));
        assert!(tree_text.contains("ERROR: failed to locate missing among local packages"));
    }

    #[test]
    fn server_list_response_deserializes_to_renderer_input() {
        let json = r#"{"schema_version":1,"total_count":5,"data":[{"id":"foo","version":"1.0.0","pinned":false,"dependencies":null}]}"#;
        let response: RocoServerPackagesResponse =
            serde_json::from_str(json).expect("deserialize server list response");

        assert_eq!(response.total_count, Some(5));
        let out = render_list_output(
            &response.data,
            response.total_count.unwrap_or(response.data.len()),
            false,
            output_style::ColorMode::Never,
        );
        assert!(out.contains("foo 1.0.0"));
        assert!(out.contains("5 packages installed."));
    }
}
