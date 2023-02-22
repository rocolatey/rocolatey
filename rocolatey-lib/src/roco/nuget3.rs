use std::collections::HashMap;

use crate::println_verbose;
use crate::roco::remote::build_reqwest;
use crate::roco::{Feed, Package};

use serde::{Deserialize, Serialize};
use serde_json::{self, json};
use serde_with;

// https://learn.microsoft.com/en-us/nuget/api/overview

#[derive(Debug, Deserialize, Serialize)]
struct PackageIndex {
    version: String,
    resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Resource {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@type")]
    restype: String,
    comment: Option<String>,
}

pub(crate) async fn get_remote_packages(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &[Package],
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    // r"SearchQueryService/3.5.0"
    // GET {@id}?q={QUERY}&prerelease={PRERELEASE}
    // https://azuresearch-usnc.nuget.org/query?q=packageid:chocolatey&prerelease=true
    // https://azuresearch-usnc.nuget.org/query?q=packageid:chocolatey%20packageid:chocolatey.lib&prerelease=true

    //progress_bar.set_position(curr_pkg_idx as u64);

    todo!()
}

pub(crate) fn read_service_index(service_index: String) -> Option<HashMap<String, String>> {
    println_verbose(&format!("read_service_index: {}", service_index));
    let PackageIndex { resources, .. } = serde_json::from_str(&service_index).unwrap();
    println_verbose(&format!("{:?}", resources));
    let res: HashMap<String, String> = HashMap::new();
    Some(res)
}
