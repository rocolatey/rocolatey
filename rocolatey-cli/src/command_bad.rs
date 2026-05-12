use rocolatey_lib::roco::local::get_local_bad_packages_text;

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
    let mut request_path = "/bad";
    if limitoutput {
        request_path = "/bad/r";
    }
    if json {
        request_path = "/bad/json";
    }

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_get(request_path, "{}").await;

    match res {
        Some(s) => {
            println!("{}", s);
        }
        None => eprintln!("Error fetching bad packages from server"),
    }
}

fn bad_local(r: bool, json: bool) {
    if json {
        let bad_pkgs = rocolatey_lib::roco::local::get_local_bad_packages().unwrap();
        match serde_json::to_string(&bad_pkgs) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
        return;
    }
    print!("{}", get_local_bad_packages_text(r));
}
