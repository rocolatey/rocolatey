use std::collections::HashMap;
use tokio;

use crate::roco::{get_choco_sources, Feed, FeedType, OutdatedInfo, Package};
use crate::roco::{local, nuget2, nuget3, semver};
use crate::{is_ssl_required, println_verbose};

impl Feed {
    pub async fn evaluate_feed_type(&mut self) -> Result<FeedType, Box<dyn std::error::Error>> {
        if self.feed_type != FeedType::Unknown {
            // already evaluated
            return Ok(self.feed_type);
        }

        let https_regex = regex::Regex::new(r"^https?://.+").unwrap();
        // if it's not a http(s)-like url, we default to type local/filesystem/unc
        if !https_regex.is_match(&self.url) {
            self.feed_type = FeedType::LocalFileSystem;
            return Ok(self.feed_type);
        }

        // we have to determine if NuGet version of feed
        // -> setup reqwest, try to fetch index.json -> v3, else: v2
        let service_index = match regex::Regex::new(r"\.json$").unwrap().is_match(&self.url) {
            true => {
                //looks like a v3 feed url
                let request = build_reqwest(self);
                let resp = request.get(&self.url).send().await;
                let resp = match resp.is_ok() {
                    true => resp.unwrap(),
                    false => return Err(resp.err().unwrap().to_string())?,
                };
                if resp.status().is_success() {
                    let content = resp.text().await.unwrap();
                    let v: Result<serde_json::Value, _> = serde_json::from_str(&content);
                    if v.is_ok() {
                        Some(v.unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            false => None,
        };

        if service_index.is_some() {
            println_verbose(&format!("feed {} looks like NuGet V3", self.name));
            self.feed_type = FeedType::NuGetV3;
            self.service_index = nuget3::read_service_index(service_index.unwrap());
        } else {
            println_verbose(&format!("feed {} is most likely NuGet V2", self.name));
            self.feed_type = FeedType::NuGetV2;
        }
        Ok(self.feed_type)
    }
}

// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html

pub(crate) fn build_reqwest(feed: &Feed) -> reqwest::Client {
    let mut builder: reqwest::ClientBuilder = reqwest::Client::builder();
    if feed.proxy.is_some() {
        let proxy_settings = feed.proxy.as_ref().unwrap();
        let mut proxy = reqwest::Proxy::all(&proxy_settings.url).unwrap();
        if proxy_settings.credential.is_some() {
            let credential = proxy_settings.credential.as_ref().unwrap();
            proxy = proxy.basic_auth(&credential.user, &credential.pass);
        }
        builder = builder.proxy(proxy);
    }
    let mut headers = reqwest::header::HeaderMap::new();

    if feed.credential.is_some() {
        let cred = feed.credential.as_ref().unwrap();
        let cred = http_auth_basic::Credentials::new(&cred.user, &cred.pass);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            cred.as_http_header().parse().unwrap(),
        );
    }
    static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
    builder
        .user_agent(APP_USER_AGENT)
        .default_headers(headers)
        .danger_accept_invalid_certs(!is_ssl_required())
        .build()
        .unwrap()
}

async fn get_latest_remote_packages_on_feed(
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let res = match &feed.feed_type {
        FeedType::LocalFileSystem => {
            let nupkg_files = local::get_nupkgs_from_path(pkgs, feed, prerelease);
            match nupkg_files.is_ok() {
                true => Ok(nupkg_files.unwrap()),
                false => Err(format!(
                    "failed to read package info from file system '{}'",
                    feed.url
                ))?,
            }
        }
        FeedType::NuGetV2 => {
            let packages = nuget2::get_remote_packages(pkgs, feed, prerelease).await;
            match packages.is_ok() {
                true => Ok(packages.unwrap()),
                false => Err(format!(
                    "failed to receive packages from NuGet v2 feed '{}'",
                    feed.url
                ))?,
            }
        }
        FeedType::NuGetV3 => {
            let packages = nuget3::get_remote_packages(pkgs, feed, prerelease).await;
            match packages.is_ok() {
                true => Ok(packages.unwrap()),
                false => Err(format!(
                    "failed to receive packages from NuGet v3 feed '{}'",
                    feed.url
                ))?,
            }
        }
        FeedType::Unknown => Err(format!(
            "cannot communicate with unknown feed type, please check feed '{}'",
            feed.name
        ))?,
    };
    res
}

async fn get_latest_remote_packages(
    pkgs: &Vec<Package>,
    limit_output: bool,
    feeds: &Vec<Feed>,
    prerelease: bool,
) -> Result<HashMap<String, Package>, Box<dyn std::error::Error>> {
    let mut remote_pkgs: HashMap<String, Package> = HashMap::new();

    let num_threads = num_cpus::get();
    let num_parts = std::cmp::max(2, std::cmp::min(num_threads, num_threads / feeds.len()));
    let chunk_size = (pkgs.len() + num_parts - 1) / num_parts;

    let mut tasks = vec![];
    let feeds = feeds.clone();

    for f in feeds {
        for chunk in pkgs.chunks(chunk_size) {
            let pkgs = chunk.to_vec();
            let feed = f.clone();
            tasks.push(tokio::spawn(async move {
                let pkgs = get_latest_remote_packages_on_feed(&pkgs, &feed, prerelease)
                    .await
                    .unwrap_or_else(|e| {
                        if !limit_output {
                            eprintln!("failed to fetch packages: {}", e)
                        }
                        vec![]
                    });
                pkgs
            }));
        }
    }

    for t in tasks {
        let pkgs = t.await.unwrap();
        for p in pkgs {
            let lowercase_id = p.id.to_lowercase();
            if remote_pkgs.contains_key(&lowercase_id) {
                let remote_version = &remote_pkgs.get(&lowercase_id).unwrap().version;
                if !semver::is_newer(&p.version, remote_version) {
                    continue;
                }
            }
            remote_pkgs.insert(lowercase_id, p);
        }
    }
    Ok(remote_pkgs)
}

pub async fn get_outdated_packages(
    pkg: &str,
    limit_output: bool,
    list_output: bool,
    prerelease: bool,
    ignore_pinned: bool,
    ignore_unfound: bool,
) -> String {
    // foreach local package, compare remote version number
    let mut local_packages = local::get_local_packages().expect("failed to get local package list");
    if "all" != pkg {
        local_packages = local_packages
            .into_iter()
            .filter(|p| p.id() == pkg)
            .collect();
        if local_packages.len() == 0 {
            panic!("package '{}' not present in local packages.", pkg);
        }
    }
    let remote_feeds = get_choco_sources().expect("failed to get choco feeds");
    let remote_feeds: Vec<Feed> = remote_feeds
        .into_iter()
        .filter(|f| f.disabled == false)
        .collect();

    println_verbose(&format!(
        "ssl checks are {}",
        if is_ssl_required() {
            "required"
        } else {
            "disabled"
        }
    ));

    // call feed.evaluate_feed_type() on each feed in remote_feeds (await!)
    let tasks: Vec<_> = remote_feeds
        .into_iter()
        .map(|mut feed| {
            tokio::spawn(async {
                // TODO: implement error handling
                // -> feed may not be reachable / get it out of the way asap.
                _ = feed.evaluate_feed_type().await;
                feed
            })
        })
        .collect();
    // await the tasks for resolve's to complete and give back our items
    let mut feeds = vec![];
    for task in tasks {
        feeds.push(task.await.unwrap());
    }
    let remote_feeds = feeds;

    let latest_packages =
        get_latest_remote_packages(&local_packages, limit_output, &remote_feeds, prerelease)
            .await
            .expect("failed to get remote package list");

    let mut oi: Vec<OutdatedInfo> = Vec::new();
    let mut warning_count = 0;

    for l in local_packages {
        if ignore_pinned && l.pinned {
            continue;
        }
        match latest_packages.get(&l.id.to_lowercase()) {
            Some(u) => {
                println_verbose(&format!(
                    "  check latest remote pkg {}, version={} against local version={}",
                    l.id, u.version, l.version
                ));
                if semver::is_newer(&u.version, &l.version) {
                    oi.push(OutdatedInfo {
                        id: l.id,
                        local_version: l.version.clone(),
                        remote_version: u.version.clone(),
                        pinned: l.pinned,
                        outdated: true,
                        exists_on_remote: true,
                    });
                }
            }
            None => {
                if !ignore_unfound {
                    warning_count += 1;
                    oi.push(OutdatedInfo {
                        id: l.id,
                        local_version: l.version.clone(),
                        remote_version: l.version.clone(),
                        pinned: l.pinned,
                        outdated: false,
                        exists_on_remote: false,
                    })
                }
            }
        };
    }

    oi.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));

    let mut warnings = String::new();
    let mut res = String::new();

    if !limit_output {
        res.push_str("Outdated Packages\n");
        if !list_output {
            res.push_str(
                " Output is package name | current version | available version | pinned?\n\n",
            );
        }
    }

    let mut outdated_packages = 0;
    for o in oi {
        if o.outdated {
            outdated_packages += 1;
        }
        if list_output {
            res.push_str(&format!("{} ", o.id));
        } else {
            res.push_str(&format!(
                "{}|{}|{}|{}\n",
                o.id, o.local_version, o.remote_version, o.pinned
            ));
        }
        if !o.exists_on_remote {
            warnings.push_str(&format!(" - {}\n", o.id));
        }
    }

    if !limit_output {
        res.push_str(&format!(
            "\nRocolatey has determined {} package(s) are outdated.\n",
            outdated_packages
        ));
        if warning_count > 0 {
            res.push_str(&format!(" {} package(s) had warnings.\n", warning_count));
            res.push_str(&format!("Warnings:\n"));
            res.push_str(&warnings);
        }
    }
    res
}

pub(crate) async fn invoke_package_bulk_request(
    pkgs: &[Package],

    feed: &Feed,
    query_string_base: &String,

    max_batch_size: u32,
    pkg_query_fmt: fn(pkg: &Package) -> String,
    query_str_delim: &String,
    query_str_end: &String,

    batch_res_processor: fn(pkgs: &mut Vec<Package>, batch_res: &String),
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut pkgs_res: Vec<Package> = Vec::new();

    let mut max_batch_size = max_batch_size;
    let mut max_url_len = 2047;
    let mut curr_pkg_idx = 0;
    let total_pkgs = pkgs.len();

    while curr_pkg_idx < total_pkgs {
        // max_batch_size and max_url_len get reduced when communication with the repository fails
        if max_batch_size == 0 || max_url_len < 100 {
            Err("failed to execute bulk query, communication failed.")?
        }

        let mut query_string = format!("{}", query_string_base);
        let mut batch_size = 0;
        let last_query_package_idx = curr_pkg_idx;

        loop {
            let curr_pkg = pkgs.get(curr_pkg_idx).unwrap();

            query_string.push_str(&pkg_query_fmt(curr_pkg));

            curr_pkg_idx += 1;
            batch_size += 1;

            let url = reqwest::Url::parse(&query_string);
            if (url.unwrap().as_str().len() > max_url_len)
                || curr_pkg_idx == pkgs.len()
                || batch_size >= max_batch_size
            {
                query_string.push_str(&query_str_end);
                break;
            }
            query_string.push_str(&query_str_delim);
        }

        println_verbose(&format!(" -> GET: {}", query_string));
        let client = build_reqwest(&feed);
        let resp = client.get(&query_string).send().await?;

        if !resp.status().is_success() {
            println_verbose(&format!("  HTTP STATUS {}", resp.status().as_str()));
            if resp.status() == 406 {
                println_verbose("bulk queries may not be supported by this repository.");
                max_batch_size = 1;
            }
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

        batch_res_processor(&mut pkgs_res, &resp);
    }

    Ok(pkgs_res)
}
