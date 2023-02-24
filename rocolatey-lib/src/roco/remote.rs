use std::collections::HashMap;
use tokio;

use crate::println_verbose;
use crate::roco::{get_choco_sources, Feed, FeedType, OutdatedInfo, Package};
use crate::roco::{local, nuget2, nuget3, semver};

impl Feed {
    pub async fn evaluate_feed_type(&mut self) {
        if self.feed_type != FeedType::Unknown {
            // already evaluated
            return;
        }

        // TODO: actually not sure if this is safe enough
        let https_regex = regex::Regex::new(r"^https?://.+").unwrap();
        // if it's not a http(s)-like url, we default to type local/filesystem/unc
        if !https_regex.is_match(&self.url) {
            self.feed_type = FeedType::LocalFileSystem;
            return;
        }

        // we have to determine if NuGet version of feed
        // -> setup reqwest, try to fetch index.json -> v3, else: v2
        let service_index = match regex::Regex::new(r"\.json$").unwrap().is_match(&self.url) {
            true => {
                //looks like a v3 feed url
                let request = build_reqwest(self);
                let resp = request.get(&self.url).send().await;
                let resp = resp.unwrap();
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
    }
}

// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html

pub(crate) fn build_reqwest(feed: &Feed) -> reqwest::Client {
    let mut rbuilder = reqwest::Client::builder();
    if feed.proxy.is_some() {
        let proxy = feed.proxy.as_ref().unwrap();
        let mut rproxy = reqwest::Proxy::all(&proxy.url).unwrap();
        if proxy.credential.is_some() {
            let credential = proxy.credential.as_ref().unwrap();
            rproxy = rproxy.basic_auth(&credential.user, &credential.pass);
        }
        rbuilder = rbuilder.proxy(rproxy);
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
    rbuilder
        .user_agent(APP_USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap()
}

async fn get_latest_remote_packages_on_feed(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    progress_bar.set_message(format!("receive packages from '{}'", feed.name));
    match &feed.feed_type {
        FeedType::LocalFileSystem => {
            let nupkg_files = local::get_nupkgs_from_path(pkgs, feed, prerelease)
                .expect("failed to read package info from file system");
            Ok(nupkg_files)
        }
        FeedType::NuGetV2 => {
            let odata_xml = nuget2::get_odata_xml_packages(progress_bar, pkgs, feed, prerelease)
                .await
                .expect("failed to receive odata for packages");
            Ok(nuget2::get_packages_from_odata(&odata_xml))
        }
        FeedType::NuGetV3 => {
            let packages = nuget3::get_remote_packages(progress_bar, pkgs, feed, prerelease)
                .await
                .expect("failed to receive packages from NuGet v3 feed");
            Ok(packages)
        }
        FeedType::Unknown => Err("cannot communicate with unknown feed type")?,
    }
}

async fn get_latest_remote_packages(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &Vec<Package>,
    feeds: &Vec<Feed>,
    prerelease: bool,
) -> Result<HashMap<String, Package>, Box<dyn std::error::Error>> {
    let mut remote_pkgs: HashMap<String, Package> = HashMap::new();
    //progress_bar.println("receiving package info from remote feeds...");

    for f in feeds {
        let pkgs = get_latest_remote_packages_on_feed(progress_bar, pkgs, f, prerelease)
            .await
            .expect("failed to get remote packages");
        // println!("{:#?}", pkgs);
        for p in pkgs {
            let lowercase_id = p.id.to_lowercase();
            if remote_pkgs.contains_key(&lowercase_id) {
                let remote_version = &remote_pkgs.get(&lowercase_id).unwrap().version;
                println_verbose(&format!(
                    "  pkg {} also exists on remote {}, version={}",
                    lowercase_id, f.name, p.version
                ));
                if !semver::is_newer(&p.version, remote_version) {
                    println_verbose(&format!(
                        "  skip: already know newer version {}",
                        remote_version
                    ));
                    continue;
                }
            }
            println_verbose(&format!(
                "  using {}, version={} for outdated check",
                lowercase_id, p.version
            ));
            remote_pkgs.insert(lowercase_id, p);
        }
    }

    Ok(remote_pkgs)
}

pub async fn get_outdated_packages(
    pkg: &str,
    limitoutput: bool,
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
    }
    let remote_feeds = get_choco_sources().expect("failed to get choco feeds");
    let remote_feeds: Vec<Feed> = remote_feeds
        .into_iter()
        .filter(|f| f.disabled == false)
        .collect();

    // call feed.evaluate_feed_type() on each feed in remote_feeds (await!)
    let tasks: Vec<_> = remote_feeds
        .into_iter()
        .map(|mut feed| {
            tokio::spawn(async {
                feed.evaluate_feed_type().await;
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

    let progress_bar = match limitoutput {
        true => indicatif::ProgressBar::hidden(),
        false => indicatif::ProgressBar::new(local_packages.len() as u64),
    };
    progress_bar.set_style(
        indicatif::ProgressStyle::with_template(
            "[{elapsed_precise}] {wide_bar:.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    progress_bar.enable_steady_tick(std::time::Duration::from_millis(500));
    let latest_packages =
        get_latest_remote_packages(&progress_bar, &local_packages, &remote_feeds, prerelease)
            .await
            .expect("failed to get remote package list");

    progress_bar.finish_with_message("received remote package information");

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

    if !limitoutput {
        res.push_str("Outdated Packages\n");
        res.push_str(" Output is package name | current version | available version | pinned?\n\n");
    }

    let mut outdated_packages = 0;
    for o in oi {
        if o.outdated {
            outdated_packages += 1;
        }
        res.push_str(&format!(
            "{}|{}|{}|{}\n",
            o.id, o.local_version, o.remote_version, o.pinned
        ));
        if !o.exists_on_remote {
            warnings.push_str(&format!(" - {}\n", o.id));
        }
    }

    if !limitoutput {
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
