use crate::{
    println_verbose,
    roco::{Feed, Package},
};

use serde::Deserialize;
use serde_json::{self};

use crate::roco::remote::build_reqwest;

#[derive(Debug, Clone, Deserialize)]
pub struct NuGetV3Index {
    version: String,
    resources: Option<Vec<NuGetResource>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuGetResource {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@type")]
    resource_type: String,
    comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResult {
    #[serde(rename = "totalHits")]
    total_hits: u32,
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

    let mut pkgs_res: Vec<Package> = Vec::new();

    println_verbose(&format!("query NuGet v3 '{}' => {}", feed.name, service));

    let latest_filter = match prerelease {
        true => "true",
        false => "false",
    };
    let query_string_base: String = format!("{}/?prerelease={}&q=", service, latest_filter);

    //TODO: cleanup! - following bulk-optimizer algorithm is a dupe of nuget2.rs
    //TODO: check this if these values are good defaults
    let mut max_url_len = 2047;
    let mut max_batch_size = 100;
    let mut curr_pkg_idx = 0;
    let total_pkgs = pkgs.len();

    while (curr_pkg_idx < total_pkgs) {
        // max_batch_size and max_url_len get reduced when communication with the repository fails
        if max_batch_size == 0 || max_url_len < 100 {
            panic!("failed to read from repository '{}'", feed.name)
        }

        let mut query_string = format!("{}", query_string_base);
        let mut batch_size = 0;
        let last_query_package_idx = curr_pkg_idx;

        loop {
            let curr_pkg = pkgs.get(curr_pkg_idx).unwrap();

            query_string.push_str(&format!("packageid:{}", curr_pkg.id));

            curr_pkg_idx += 1;
            batch_size += 1;

            let url = reqwest::Url::parse(&query_string);
            if (url.unwrap().as_str().len() > max_url_len)
                || curr_pkg_idx == pkgs.len()
                || batch_size >= max_batch_size
            {
                break;
            }
            query_string.push_str(" ");
        }

        println_verbose(&format!(" -> GET: {}", query_string));
        let client = build_reqwest(&feed);
        let resp = client.get(&query_string).send().await?;

        if !resp.status().is_success() {
            println_verbose(&format!("  HTTP STATUS {}", resp.status().as_str()));
        }

        // if we get a client err response - try reducing url length (first)
        if resp.status().is_client_error() {
            max_url_len = max_url_len / 2;
            println_verbose(&format!("  reduced max url length: {}", max_url_len));
            curr_pkg_idx = last_query_package_idx;
            continue;
        }

        let resp = resp.text().await.unwrap_or_default();

        // if we still get an invalid response - try reducing the batch query size...
        if resp.is_empty() {
            max_batch_size -= 1;
            println_verbose(&format!("  reduced receive batch size: {}", max_batch_size));
            curr_pkg_idx = last_query_package_idx;
            continue;
        }

        extract_newer_packages(&mut pkgs_res, resp);

        progress_bar.set_position(curr_pkg_idx as u64);
    }

    Ok(pkgs_res)
}

fn extract_newer_packages(pkgs_res: &mut Vec<Package>, resp: String) {
    let query_result: QueryResult = serde_json::from_str(&resp).unwrap();
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
