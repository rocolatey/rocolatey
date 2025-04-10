use warp::Filter;
extern crate clap;
use clap::{Arg, Command};

use rocolatey_lib::roco::{
    local::{get_local_bad_packages_text, get_local_packages_text},
    remote::get_outdated_packages_text,
};

#[tokio::main]
async fn main() {
    let matches = Command::new("Rocolatey Server")
        .version("0.9.3")
        .author("Manfred Wallner <schusterfredl@mwallner.net>")
        .about("provides web access to rocolatey-lib")
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .help("Sets the port to bind to")
                .takes_value(true)
                .default_value("8081"),
        )
        .arg(
            Arg::new("address")
                .long("address")
                .short('a')
                .help("Sets the address to bind to")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .get_matches();

    let bind_addr: &str = matches.value_of("address").unwrap_or("127.0.0.1");
    let bind_port: u16 = matches
        .value_of("port")
        .unwrap_or("8081")
        .parse()
        .expect("invalid port number");

    println!(" server binds on ip: {}", bind_addr);
    println!(" server binds on port: {}", bind_port);

    let warp_filter = create_warp_filter();
    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    warp::serve(warp_filter).run((server_ip, bind_port)).await;
}

fn create_warp_filter() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    let api_base = warp::path("rocolatey");

    let local = api_base
        .and(warp::path("local"))
        .and(warp::path::end())
        .map(|| req_local(false));

    let local_r = api_base
        .and(warp::path!("local" / "r"))
        .and(warp::path::end())
        .map(|| req_local(true));

    let bad = api_base
        .and(warp::path("bad"))
        .and(warp::path::end())
        .map(|| req_local_bad(false));

    let bad_r = api_base
        .and(warp::path!("bad" / "r"))
        .and(warp::path::end())
        .map(|| req_local_bad(true));

    let outdated = api_base
        .and(warp::path("outdated"))
        .and(warp::path::end())
        .and_then(|| req_outdated(false));

    let outdated_r = api_base
        .and(warp::path!("outdated" / "r"))
        .and(warp::path::end())
        .and_then(|| req_outdated(true));

    let routes = local
        .or(local_r)
        .or(bad)
        .or(bad_r)
        .or(outdated)
        .or(outdated_r);

    routes
        .with(warp::log::custom(|info| {
            println!(
                "Received request: {} {} from {}",
                info.method(),
                info.path(),
                info.remote_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
        }))
        .with(warp::log("rocolatey_server"))
}

fn req_local(limitoutput: bool) -> String {
    get_local_packages_text("all", limitoutput)
}

fn req_local_bad(limitoutput: bool) -> String {
    get_local_bad_packages_text(limitoutput)
}

async fn req_outdated(limit_output: bool) -> Result<impl warp::Reply, warp::Rejection> {
    let result = get_outdated_packages_text("all", limit_output, false, false, true, true).await;
    Ok(result)
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
