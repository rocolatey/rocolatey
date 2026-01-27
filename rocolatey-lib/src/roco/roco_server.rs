use reqwest::header::CONTENT_TYPE;
use serde_json;

use crate::server::{JobStatus, RocoServerChocoCommandRequest};
use reqwest::ClientBuilder;

pub async fn run_on_server_simple_get(request_path: &str, request_body: &str) -> Option<String> {
    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let url = format!("http://{}:{}/rocolatey{}", ip, port, request_path);

    let client = ClientBuilder::new().build().expect("http client");
    match client
        .get(&url)
        .header(CONTENT_TYPE, "application/json")
        .body(request_body.to_string())
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(txt) => Some(txt),
            Err(e) => {
                eprintln!("Request to {} failed: {}", url, e);
                None
            }
        },
        Err(e) => {
            eprintln!("Request to {} failed: {}", url, e);
            None
        }
    }
}

/// Run a Chocolatey command on a remote rocolatey server and poll for completion.
///
/// What it does:
/// - Builds a JSON `RocoServerChocoCommandRequest` from the provided
///   `choco_args` and `package_names` and POSTs it to the server at
///   `http://<ROCO_SERVER_IP>:<ROCO_SERVER_PORT>/rocolatey/choco`.
/// - Expects the server to respond with a job id. It then polls
///   `GET /rocolatey/choco/status/<id>` to fetch
///   the job state and logs.
/// - New log lines received from the server are printed immediately
///   with `println!`, so they are streamed to the caller's stdout as
///   they arrive (they are not captured or returned by this function).
/// - Returns `true` when the server reports `JobStatus::Completed` and
///   `false` for `JobStatus::Failed` or persistent errors.
///
/// Error handling and retries:
/// - If the initial POST or JSON parsing fails the function logs an
///   error to stderr and returns `false`.
/// - While polling, transient failures to fetch or parse status are
///   retried; persistent failures increment an internal `err_count`.
///   Once `err_count` reaches 5 the function returns `false`.
///
/// Notes:
/// - Logs are printed incrementally using `println!`, so they appear
///   in real time on stdout; stderr from the remote job is expected to
///   be included in the server-provided `logs` list if applicable.
pub async fn run_on_server_poll(choco_args: &[&str], package_names: &[&str]) -> i32 {
    let (_, ip) = crate::server::get_server_ip();
    let (_, port) = crate::server::get_server_port();
    let base = format!("http://{}:{}/rocolatey", ip, port);
    let poll_millis = crate::server::get_server_poll_interval_millis();

    let command = choco_args
        .get(0)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upgrade".to_string());
    let mut args: Vec<String> = choco_args.iter().skip(1).map(|s| s.to_string()).collect();
    args.extend(package_names.iter().map(|s| s.to_string()));
    let body = RocoServerChocoCommandRequest { command, args };

    let client = ClientBuilder::new().build().expect("http client");

    // send POST -> get id
    let body_json = serde_json::to_string(&body).expect("serialize body");
    let resp = client
        .post(&format!("{}/choco", base))
        .header(CONTENT_TYPE, "application/json")
        .body(body_json)
        .send()
        .await;
    let id = match resp {
        Ok(r) => {
            let txt = match r.text().await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("request failed: {}", e);
                    return -1;
                }
            };
            match serde_json::from_str::<crate::server::RocoServerChocoJobIdResponse>(&txt) {
                Ok(j) => j.id,
                Err(e) => {
                    eprintln!("request failed: {}", e);
                    return -1;
                }
            }
        }
        Err(e) => {
            eprintln!("request failed: {}", e);
            return -1;
        }
    };

    let mut err_count = 0;
    // poll
    let mut last_log_idx = 0usize;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(poll_millis)).await;
        let status_resp = client
            .get(&format!("{}/choco/status/{}", base, id))
            .send()
            .await;
        match status_resp {
            Ok(r) => {
                let txt = match r.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("status query failed: {}", e);
                        err_count += 1;
                        if err_count >= 5 {
                            return -1;
                        }
                        continue;
                    }
                };
                match serde_json::from_str::<crate::server::JobState>(&txt) {
                    Ok(js) => {
                        // print new logs
                        for line in js.logs.iter().skip(last_log_idx) {
                            println!("{}", line);
                        }
                        last_log_idx = js.logs.len();
                        match js.status {
                            JobStatus::Pending | JobStatus::Running => continue,
                            JobStatus::Completed { exit_code } => return exit_code,
                            JobStatus::Failed { .. } => return -1,
                        }
                    }
                    Err(e) => {
                        eprintln!("ERROR: failed to parse json: {} - {}\n", txt, e);
                        err_count += 1;
                        if err_count >= 5 {
                            return -1;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("ERROR: status query failed: {}", e);
                err_count += 1;
                if err_count >= 5 {
                    return -1;
                }
            }
        }
    }
}
