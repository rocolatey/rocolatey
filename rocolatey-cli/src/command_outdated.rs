use rocolatey_lib::roco::remote::{get_outdated_packages, get_outdated_packages_text};

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
    let mut request_path = "/outdated";
    if r {
        request_path = "/outdated/r";
    }
    if json {
        request_path = "/outdated/json";
    }
    if l {
        request_path = "/outdated/l";
    }

    let request_body = serde_json::json!({
        "pkg": pkg,
        "pre": pre,
        "ignore_pinned": ignore_pinned,
        "ignore_unfound": ignore_unfound
    })
    .to_string();

    let res =
        rocolatey_lib::roco::roco_server::run_on_server_simple_get(request_path, &request_body)
            .await;

    match res {
        Some(s) => {
            println!("{}", s);
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
    print!(
        "{}",
        get_outdated_packages_text(pkg, r, l, pre, ignore_pinned, ignore_unfound).await
    );
}
