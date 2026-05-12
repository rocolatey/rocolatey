use rocolatey_lib::roco::local::get_dependency_tree_text;
use rocolatey_lib::roco::local::{get_local_packages, get_package_list_text};

pub async fn list(matches: &clap::ArgMatches) {
    rocolatey_lib::set_verbose_mode(matches.get_flag("verbose"));
    let r = matches.get_flag("limitoutput");
    let json = matches.get_flag("json-output");
    let filter = matches.get_one::<String>("filter").unwrap();

    let (use_server, _) = rocolatey_lib::server::get_server_ip();

    if use_server {
        list_remote(r, json).await;
    } else {
        list_local(matches, r, json, filter);
    }
}

async fn list_remote(limitoutput: bool, json: bool) {
    let mut request_path = "/local";
    if limitoutput {
        request_path = "/local/r";
    }
    if json {
        request_path = "/local/json";
    }

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_get(request_path, "{}").await;

    match res {
        Some(s) => {
            println!("{}", s);
        }
        None => eprintln!("Error fetching bad packages from server"),
    }
}

fn list_local(matches: &clap::ArgMatches, r: bool, json: bool, filter: &String) {
    if matches.get_flag("deptree") {
        print!("{}", get_dependency_tree_text(filter));
    } else {
        let (packages, total_pkgs) = get_local_packages(filter).unwrap();

        let mut res = String::new();
        if json {
            match serde_json::to_string(&packages) {
                Ok(s) => {
                    res = s;
                }
                Err(e) => eprintln!("Error converting to JSON: {}", e),
            }
        } else {
            res.push_str(get_package_list_text(packages, r).as_ref());
            if !r {
                res.push_str(&format!("\r\n{} packages installed.", total_pkgs));
            }
        }
        print!("{}", res);
    }
}
