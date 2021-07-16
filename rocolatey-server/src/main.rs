use warp::Filter;
extern crate clap;
use clap::{App, Arg};

use rocolatey_lib::roco::local::{get_local_bad_packages_text, get_local_packages_text};

#[tokio::main]
async fn main() {
    let matches = App::new("Rocolatey Server")
        .version("0.5.5")
        .author("Manfred Wallner <schusterfredl@mwallner.net>")
        .about("provides web access to rocolatey-lib")
        .arg(
            Arg::with_name("address")
                .help("Sets the network address to bind to")
                .index(1),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .help("Sets the port to bind to")
                .index(2),
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

    let warp_filter = warp::path!("rocolatey" / "local")
        .map(|| req_local(false))
        .or(warp::path!("rocolatey" / "local" / "r").map(|| req_local(true)))
        .or(warp::path!("rocolatey" / "bad").map(|| req_local_bad(false)))
        .or(warp::path!("rocolatey" / "bad" / "r").map(|| req_local_bad(true)));
    let server_ip: std::net::Ipv4Addr = bind_addr.parse().unwrap();
    warp::serve(warp_filter).run((server_ip, bind_port)).await;
}

fn req_local(limitoutput: bool) -> String {
    get_local_packages_text(limitoutput)
}

fn req_local_bad(limitoutput: bool) -> String {
    get_local_bad_packages_text(limitoutput)
}
