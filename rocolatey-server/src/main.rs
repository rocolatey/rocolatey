use warp::Filter;
extern crate clap;
use clap::{Arg, Command};

use rocolatey_lib::roco::local::{get_local_bad_packages_text, get_local_packages_text};

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
        .map(|| req_local(false))
        .or(api_base
            .and(warp::path!("local" / "r"))
            .map(|| req_local(true)))
        .or(api_base
            .and(warp::path!("bad"))
            .map(|| req_local_bad(false)))
        .or(api_base
            .and(warp::path!("bad" / "r"))
            .map(|| req_local_bad(true)));
    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    warp::serve(warp_filter).run((server_ip, bind_port)).await;
}

fn req_local(limitoutput: bool) -> String {
    get_local_packages_text("all", limitoutput)
}

fn req_local_bad(limitoutput: bool) -> String {
    get_local_bad_packages_text(limitoutput)
}
