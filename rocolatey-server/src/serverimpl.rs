use std::net::SocketAddr;
use uuid::Uuid;
use warp::http::StatusCode;
use warp::Filter;

use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use rocolatey_lib::roco::{
    get_choco_sources,
    local::{
        get_local_bad_packages, get_local_bad_packages_text, get_local_packages_json,
        get_local_packages_text, get_sources_text,
    },
    remote::{get_outdated_packages, get_outdated_packages_text},
};
use rocolatey_lib::server::JobState;
use rocolatey_lib::server::JobStatus;
use rocolatey_lib::server::RocoServerChocoCommandRequest;

pub type JobStore = Arc<RwLock<HashMap<Uuid, JobState>>>;

pub(crate) fn create_warp_filter(
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let api_base = warp::path("rocolatey");
    // job store for background choco commands
    let store: JobStore = Arc::new(RwLock::new(HashMap::new()));
    let store_filter = warp::any().map(move || store.clone());

    let local = api_base
        .and(warp::path("local"))
        .and(warp::path::end())
        .map(|| get_local_packages_text("all", false));

    let local_r = api_base
        .and(warp::path!("local" / "r"))
        .and(warp::path::end())
        .map(|| get_local_packages_text("all", true));

    let local_json = api_base
        .and(warp::path!("local" / "json"))
        .and(warp::path::end())
        .map(|| get_local_packages_json("all"));

    let bad = api_base
        .and(warp::path("bad"))
        .and(warp::path::end())
        .map(|| get_local_bad_packages_text(false));

    let bad_r = api_base
        .and(warp::path!("bad" / "r"))
        .and(warp::path::end())
        .map(|| get_local_bad_packages_text(true));

    let bad_json = api_base
        .and(warp::path!("bad" / "json"))
        .and(warp::path::end())
        .map(|| match get_local_bad_packages() {
            Ok(data) => warp::reply::json(&tokio::task::block_in_place(|| data)),
            Err(err) => {
                let body = serde_json::json!({ "error": format!("Error: {}", err) });
                warp::reply::json(&body)
            }
        });

    let source = api_base
        .and(warp::path("source"))
        .and(warp::path::end())
        .map(|| get_sources_text(false));

    let source_r = api_base
        .and(warp::path!("source" / "r"))
        .and(warp::path::end())
        .map(|| get_sources_text(true));

    let source_json = api_base
        .and(warp::path!("source" / "json"))
        .and(warp::path::end())
        .map(|| match get_choco_sources() {
            Ok(data) => warp::reply::json(&tokio::task::block_in_place(|| data)),
            Err(err) => {
                let body = serde_json::json!({ "error": format!("Error: {}", err) });
                warp::reply::json(&body)
            }
        });

    // TODO: parse body for filter options (pre, ignore_pinned, ignore_unfound) instead of hardcoding them here
    /*
        let request_body = serde_json::json!({
            "pkg": pkg,
            "pre": pre,
            "ignore_pinned": ignore_pinned,
            "ignore_unfound": ignore_unfound
        })
        .to_string();
    */
    let outdated = api_base
        .and(warp::path("outdated"))
        .and(warp::path::end())
        .and_then(|| req_outdated(false, false));

    let outdated_r = api_base
        .and(warp::path!("outdated" / "r"))
        .and(warp::path::end())
        .and_then(|| req_outdated(true, false));

    let outdated_l = api_base
        .and(warp::path!("outdated" / "l"))
        .and(warp::path::end())
        .and_then(|| req_outdated(false, true));

    let outdated_json = api_base
        .and(warp::path!("outdated" / "json"))
        .and(warp::path::end())
        .and_then(|| req_outdated_json());

    let runchoco = api_base
        .and(warp::path!("choco"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and(store_filter.clone())
        .and_then(
            |cmd: RocoServerChocoCommandRequest, remote: Option<SocketAddr>, store: JobStore| async move {
                // only allow calls from loopback addresses
                // TODO: add authentication mechanism to allow remote calls

                let allowed = match remote {
                    Some(addr) => match addr.ip() {
                        std::net::IpAddr::V4(v4) => v4.is_loopback(),
                        std::net::IpAddr::V6(v6) => v6.is_loopback(),
                    },
                    None => false,
                };

                if !allowed {
                    let body = serde_json::json!({ "error": "Forbidden: only localhost may call this endpoint" });
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&body),
                        StatusCode::FORBIDDEN,
                    ));
                }

                let allowed_commands = [
                    "install",
                    "upgrade",
                    "uninstall",
                    "list",
                    "search",
                    "info",
                    "outdated",
                ];

                if !allowed_commands.contains(&cmd.command.as_str()) {
                    let body = serde_json::json!({ "error": format!("Forbidden choco command: {}", cmd.command) });
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&body),
                        StatusCode::BAD_REQUEST,
                    ));
                }

                let id = Uuid::new_v4();
                let job = JobState {
                    id,
                    status: JobStatus::Pending,
                    created_at: std::time::SystemTime::now(),
                    logs: Vec::new(),
                };

                {
                    let mut map = store.write().await;
                    map.insert(id, job.clone());
                }

                tokio::spawn(run_choco_background(id, cmd, store.clone()));

                // return 202 with lookup id
                let body = serde_json::json!({ "id": id.to_string(), "status": "accepted" });
                Ok(warp::reply::with_status(
                    warp::reply::json(&body),
                    warp::http::StatusCode::ACCEPTED,
                ))
            },
        );

    let status_get = api_base
        .and(warp::path!("choco" / "status" / String))
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(move |id_str: String, store: JobStore| async move {
            match Uuid::parse_str(&id_str) {
                Ok(id) => {
                    let map = store.read().await;
                    match map.get(&id) {
                        Some(job) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&job),
                            warp::http::StatusCode::OK,
                        )),
                        None => Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "Not found" })),
                            warp::http::StatusCode::NOT_FOUND,
                        )),
                    }
                }
                Err(_) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "Bad id" })),
                    warp::http::StatusCode::BAD_REQUEST,
                )),
            }
        });

    let routes = local
        .or(local_r)
        .or(local_json)
        .or(bad)
        .or(bad_r)
        .or(bad_json)
        .or(source)
        .or(source_r)
        .or(source_json)
        .or(outdated)
        .or(outdated_r)
        .or(outdated_l)
        .or(outdated_json)
        .or(runchoco)
        .or(status_get);

    routes
        .with(warp::log::custom(|info| {
            println!(
                "{} - Received request: {} {} from {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                info.method(),
                info.path(),
                info.remote_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
        }))
        .with(warp::log("rocolatey_server"))
}

async fn req_outdated(
    limit_output: bool,
    list_output: bool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let result =
        get_outdated_packages_text("all", limit_output, list_output, false, true, true).await;
    Ok(result)
}

async fn req_outdated_json() -> Result<impl warp::Reply, warp::Rejection> {
    let result = get_outdated_packages("all", false, false, false, true).await;
    Ok(warp::reply::json(&result))
}

async fn run_choco_background(id: Uuid, cmd: RocoServerChocoCommandRequest, store: JobStore) {
    // mark Running
    {
        let mut map = store.write().await;
        if let Some(job) = map.get_mut(&id) {
            job.status = JobStatus::Running;
            /*
            job.logs.push(format!(
                "Started at {}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ));
            */
            println!(" - Started job {}: {:?} {:?}", id, cmd.command, cmd.args);
        }
    }

    // spawn process and stream stdout/stderr linewise into job.logs
    use std::process::Stdio;
    use tokio::process::Command as TokioCommand;

    let mut child = match TokioCommand::new("choco.exe")
        .arg(&cmd.command)
        .args(&cmd.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Failed {
                    error: format!("spawn error: {}", e),
                };
                job.logs.push(format!("spawn error: {}", e));
            }
            return;
        }
    };

    // merge stdout and stderr reading tasks (simple approach: read both concurrently)
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(read_stream_into_logs(
            stdout,
            id,
            store.clone(),
            "", // "stdout",
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(read_stream_into_logs(
            stderr,
            id,
            store.clone(),
            "", // "stderr",
        )));
    }

    // wait for child to exit
    let status = child.wait().await;

    // wait for readers to finish
    for r in readers {
        let _ = r.await;
    }

    match status {
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Completed { exit_code: code };
                // job.logs.push(format!("Process exited with code {}", code));
            }
        }
        Err(e) => {
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Failed {
                    error: format!("wait error: {}", e),
                };
                job.logs.push(format!("wait error: {}", e));
            }
        }
    }
}

// helper to read stdout/stderr lines and append to job logs
async fn read_stream_into_logs<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    stream: R,
    id: Uuid,
    store: JobStore,
    label: &str,
) {
    let mut reader = tokio::io::BufReader::new(stream);
    loop {
        let mut buf = Vec::new();
        match tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut buf).await {
            Ok(0) => break, // stream closed
            Ok(_) => {
                // convert bytes to string, replacing invalid UTF-8 sequences
                let s = String::from_utf8_lossy(&buf);
                let line = s.trim_end_matches(|c| c == '\n' || c == '\r');

                let mut map = store.write().await;
                if let Some(job) = map.get_mut(&id) {
                    let entry = if label.is_empty() {
                        line.to_string()
                    } else {
                        format!("[{}] {}", label, line)
                    };

                    job.logs.push(entry);
                    if job.logs.len() > 1000 {
                        job.logs.drain(0..(job.logs.len() - 1000));
                    }
                }
            }
            Err(e) => {
                let mut map = store.write().await;
                if let Some(job) = map.get_mut(&id) {
                    job.logs.push(format!("stream read error: {}", e));
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;

    #[tokio::test]
    async fn test_local_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/local")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("local packages"));
    }

    #[tokio::test]
    async fn test_bad_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/bad")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("bad packages"));
    }

    #[tokio::test]
    async fn test_outdated_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/outdated")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("outdated packages"));
    }

    #[tokio::test]
    async fn test_unmatched_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/unknown")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 404);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("Not Found"));
    }
}
