use crate::{
    println_verbose,
    roco::{Feed, Package},
};

use serde::Deserialize;
use serde_json::{self};

use super::remote::invoke_package_bulk_request;

#[derive(Debug, Clone, Deserialize)]
pub struct NuGetV3Index {
    resources: Option<Vec<NuGetResource>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuGetResource {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@type")]
    resource_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResult {
    data: Option<Vec<QueryResultPackage>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResultPackage {
    id: String,
    version: String,
}

fn get_resource<'f>(feed: &'f Feed, resource_type: &str) -> Option<Vec<&'f NuGetResource>> {
    if feed.service_index.is_none() {
        return None;
    }
    let idx = feed.service_index.as_ref().unwrap();
    if idx.resources.is_none() {
        return None;
    }
    let res = idx.resources.as_ref().unwrap();
    Some(
        res.iter()
            .filter(|e| e.resource_type == resource_type)
            .collect(),
    )
}

// https://learn.microsoft.com/en-us/nuget/api/overview

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
    let search_query_service: Option<Vec<&NuGetResource>> =
        get_resource(feed, "SearchQueryService");

    if search_query_service.is_none() {
        Err(r"SearchQueryService missing")?
    }

    // TODO: handle multiple service URLs (secondary/ fallback?)
    let service = search_query_service.as_ref().unwrap().first();
    let service = &service.unwrap().id;

    println_verbose(&format!("query NuGet v3 '{}' => {}", feed.name, service));

    let latest_filter = match prerelease {
        true => "true",
        false => "false",
    };
    let query_string_base: String = format!("{}/?prerelease={}&q=", service, latest_filter);

    let query_str_delim = " ".to_owned();
    let query_str_end = "".to_owned();

    invoke_package_bulk_request(
        progress_bar,
        pkgs,
        feed,
        &query_string_base,
        100,
        |p| format!("packageid:{}", p.id),
        &query_str_delim,
        &query_str_end,
        |pkgs, batch_str| -> () {
            extract_packages(pkgs, batch_str);
        },
    )
    .await
}

fn extract_packages(pkgs_res: &mut Vec<Package>, resp: &String) {
    let query_result: QueryResult = serde_json::from_str(resp).unwrap();
    match query_result.data {
        Some(pkgs) => {
            pkgs.iter().for_each(|p| {
                pkgs_res.push(Package {
                    id: p.id.clone(),
                    version: p.version.clone(),
                    pinned: false,
                })
            });
        }
        None => {}
    };
}

pub(crate) fn read_service_index(index_json: serde_json::Value) -> Option<NuGetV3Index> {
    match serde_json::from_value(index_json) {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}
