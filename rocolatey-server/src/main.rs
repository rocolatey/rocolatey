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
        .version("0.9.2")
        .author("Manfred Wallner <schusterfredl@mwallner.net>")
        .about("provides web access to rocolatey-lib")
        .arg(
            Arg::new("address")
                .long("address")
                .short('a')
                .help("Sets the network address to bind to"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .help("Sets the port to bind to"),
        )
        .get_matches();

    let bind_addr = matches.value_of("address").unwrap_or("127.0.0.1");
    let bind_port: u16 = matches
        .value_of("port")
        .unwrap_or("8081")
        .parse()
        .expect("invalid port number");

    println!(" server binds on ip: {}", bind_addr);
    println!(" server binds on port: {}", bind_port);

    let api_base = warp::path!("rocolatey");

    let warp_filter = api_base
        .and(warp::path!("local"))
        .and_then(|| req_local(false))
        .or(api_base
            .and(warp::path!("local" / "r"))
            .and_then(|| req_local(true)))
        .or(api_base
            .and(warp::path!("bad"))
            .and_then(|| req_local_bad(false)))
        .or(api_base
            .and(warp::path!("bad" / "r"))
            .and_then(|| req_local_bad(true)))
        .or(api_base
            .and(warp::path!("outdated"))
            .and_then(|| req_outdated(false)))
        .or(api_base
            .and(warp::path!("outdated" / "r"))
            .and_then(|| req_outdated(true)));

    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    warp::serve(warp_filter).run((server_ip, bind_port)).await;
}

async fn req_local(limitoutput: bool) -> Result<impl warp::Reply, warp::Rejection> {
    println!("req_local");
    Ok(get_local_packages_text("all", limitoutput))
}

async fn req_local_bad(limitoutput: bool) -> Result<impl warp::Reply, warp::Rejection> {
    println!("req_local_bad");
    Ok(get_local_bad_packages_text(limitoutput))
}

async fn req_outdated(limitoutput: bool) -> Result<impl warp::Reply, warp::Rejection> {
    println!("req_outdated");
    Ok(get_outdated_packages_text("all", limitoutput, false, false, true, true).await)
}
