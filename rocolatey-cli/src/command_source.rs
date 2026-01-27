use rocolatey_lib::roco::{get_choco_sources, local::get_sources_text};

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
    let mut request_path = "/source";
    if limitoutput {
        request_path = "/source/r";
    }
    if json {
        request_path = "/source/json";
    }

    let res = rocolatey_lib::roco::roco_server::run_on_server_simple_get(request_path, "{}").await;

    match res {
        Some(s) => {
            println!("{}", s);
        }
        None => eprintln!("Error fetching sources from server"),
    }
}

fn source_local(limitoutput: bool, json: bool) {
    if json {
        let sources = get_choco_sources().unwrap();
        match serde_json::to_string(&sources) {
            Ok(s) => {
                println!("{}", s);
            }
            Err(e) => eprintln!("Error converting to JSON: {}", e),
        }
    } else {
        print!("{}", get_sources_text(limitoutput));
    }
}
