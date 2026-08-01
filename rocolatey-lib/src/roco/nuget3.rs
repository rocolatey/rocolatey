use crate::{
    println_verbose,
    roco::{Feed, Package},
};

use serde::{Deserialize, Serialize};
use serde_json::{self};

use super::remote::invoke_package_bulk_request;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuGetV3Index {
    pub resources: Option<Vec<NuGetResource>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuGetResource {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub resource_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    data: Option<Vec<QueryResultPackage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pkgs: &[Package],
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    // r"SearchQueryService/3.5.0"
    // GET {@id}?q={QUERY}&prerelease={PRERELEASE}
    // https://azuresearch-usnc.nuget.org/query?q=packageid:chocolatey&prerelease=true
    // https://azuresearch-usnc.nuget.org/query?q=packageid:chocolatey%20packageid:chocolatey.lib&prerelease=true

    let search_query_service: Option<Vec<&NuGetResource>> =
        get_resource(feed, "SearchQueryService");

    if search_query_service.is_none() {
        Err(r"SearchQueryService missing")?
    }

    let latest_filter = match prerelease {
        true => "true",
        false => "false",
    };
    let query_str_delim = " ".to_owned();
    let query_str_end = "".to_owned();

    let services = search_query_service
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect::<Vec<String>>();

    if services.is_empty() {
        Err(r"SearchQueryService missing")?
    }

    let mut last_err: Option<String> = None;

    for service in services {
        println_verbose(&format!("query NuGet v3 '{}' => {}", feed.name, service));
        let query_string_base: String = format!("{}?prerelease={}&q=", service, latest_filter);

        match invoke_package_bulk_request(
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
        {
            Ok(res) => return Ok(res),
            Err(e) => {
                last_err = Some(e.to_string());
                println_verbose(&format!(
                    "query NuGet v3 '{}' failed for {}: {}",
                    feed.name,
                    service,
                    last_err.as_ref().unwrap()
                ));
            }
        }
    }

    Err(last_err
        .unwrap_or_else(|| "all SearchQueryService URLs failed".to_string())
        .into())
}

pub(crate) async fn find_remote_packages(
    search_terms: &Vec<String>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let search_query_service: Option<Vec<&NuGetResource>> =
        get_resource(feed, "SearchQueryService");

    if search_query_service.is_none() {
        Err(r"SearchQueryService missing")?
    }

    let latest_filter = match prerelease {
        true => "true",
        false => "false",
    };
    let query_str_delim = " ".to_owned();
    let query_str_end = "".to_owned();

    // create pseudo-packages from search terms
    let search_pkgs: Vec<Package> = search_terms
        .iter()
        .map(|s| Package {
            id: s.clone(),
            version: String::new(),
            pinned: false,
            dependencies: None,
        })
        .collect();

    let services = search_query_service
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect::<Vec<String>>();

    if services.is_empty() {
        Err(r"SearchQueryService missing")?
    }

    let mut last_err: Option<String> = None;

    for service in services {
        println_verbose(&format!("query NuGet v3 '{}' => {}", feed.name, service));
        let query_string_base: String = format!("{}?prerelease={}&q=", service, latest_filter);

        match invoke_package_bulk_request(
            &search_pkgs,
            feed,
            &query_string_base,
            100,
            |p| p.id.clone(),
            &query_str_delim,
            &query_str_end,
            |pkgs, batch_str| -> () {
                extract_packages(pkgs, batch_str);
            },
        )
        .await
        {
            Ok(res) => return Ok(res),
            Err(e) => {
                last_err = Some(e.to_string());
                println_verbose(&format!(
                    "query NuGet v3 '{}' failed for {}: {}",
                    feed.name,
                    service,
                    last_err.as_ref().unwrap()
                ));
            }
        }
    }

    Err(last_err
        .unwrap_or_else(|| "all SearchQueryService URLs failed".to_string())
        .into())
}

pub fn extract_packages(pkgs_res: &mut Vec<Package>, resp: &String) {
    let query_result: QueryResult = serde_json::from_str(resp).unwrap();
    match query_result.data {
        Some(pkgs) => {
            pkgs.iter().for_each(|p| {
                pkgs_res.push(Package {
                    id: p.id.clone(),
                    version: p.version.clone(),
                    pinned: false,
                    dependencies: None,
                })
            });
        }
        None => {}
    };
}

pub fn read_service_index(index_json: serde_json::Value) -> Option<NuGetV3Index> {
    match serde_json::from_value(index_json) {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}
