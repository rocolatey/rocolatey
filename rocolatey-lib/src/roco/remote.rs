use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::println_verbose;
use crate::roco::local::get_local_packages;
use crate::roco::{get_choco_sources, semver_is_newer, Feed, OutdatedInfo, Package};

// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html
// https://joelverhagen.github.io/NuGetUndocs/
// http://docs.oasis-open.org/odata/odata/v4.0/errata03/os/complete/part1-protocol/odata-v4.0-errata03-os-part1-protocol-complete.html

fn build_reqwest(feed: &Feed) -> reqwest::Client {
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

async fn get_package_count_on_feed(f: &Feed, prerelease: bool) -> u32 {
    let latest_filter = match prerelease {
        true => "$filter=IsAbsoluteLatestVersion",
        false => "$filter=IsLatestVersion",
    };
    let rs = format!("{}/Packages()/$count?{}", f.url, latest_filter);

    let client = build_reqwest(&f);
    let resp_pkg_count = client.get(&rs).send().await;
    let total_pkg_count = resp_pkg_count.unwrap().text().await.unwrap();
    let total_pkg_count = total_pkg_count.parse::<u32>().unwrap();
    total_pkg_count
}

async fn receive_package_delta(
    feed: &Feed,
    batch_size: u32,
    batch_offset: u32,
    prerelease: bool,
) -> (u32, String) {
    let base_uri = format!("{}/Packages()", feed.url);
    let latest_filter = match prerelease {
        true => "$filter=IsAbsoluteLatestVersion",
        false => "$filter=IsLatestVersion",
    };
    let rs = match batch_size {
        0 => format!("{}?{}&$skip={}", base_uri, latest_filter, batch_offset),
        _ => format!(
            "{}?{}&$top={}&$skip={}",
            base_uri, latest_filter, batch_size, batch_offset
        ),
    };

    let client = build_reqwest(&feed);
    let resp = client.get(&rs).send().await;
    let query_res = resp.unwrap().text().await.unwrap();
    let c = query_res.matches("</entry>").count();
    (c as u32, query_res)
}

async fn update_feed_index(feed: &Feed, limitoutput: bool, prerelease: bool) -> String {
    let total_pkg_count = get_package_count_on_feed(feed, prerelease).await;
    println!(
        "there are a total of {} packages on feed {}",
        total_pkg_count, feed.name
    );
    let f = File::create(format!("{}_dl.xml", feed.name)).expect("Unable to create file");
    let mut f = BufWriter::new(f);
    let mut batch_size = 0;
    let mut received_packages = 0;
    let progress_bar = match limitoutput {
        true => indicatif::ProgressBar::hidden(),
        false => indicatif::ProgressBar::new(total_pkg_count as u64),
    };
    while received_packages < total_pkg_count {
        let (a, req_res) =
            receive_package_delta(feed, batch_size, received_packages, prerelease).await;
        if a != batch_size {
            println!("receiving packages in batches of {} per request", a);
        }
        batch_size = a;
        f.write_all(req_res.as_bytes())
            .expect("unable to write data");
        if 0 == batch_size {
            println!("failed to receive further packages... stop!");
            break;
        }
        received_packages += batch_size;
        progress_bar.set_position(received_packages as u64);
    }
    progress_bar.finish();
    f.flush().expect("failed to flush file buffer");
    // TODO - "shrink" pkg index files - only keep id + version (faster lookup later on)

    String::from("update_feed_index -> not implemented")
}

pub async fn update_package_index(limitoutput: bool, prerelease: bool) -> String {
    let mut s = String::new();
    let feeds = get_choco_sources().expect("failed to get choco sources");
    for f in feeds.into_iter().filter(|f| f.disabled == false) {
        s.push_str(&update_feed_index(&f, limitoutput, prerelease).await);
    }
    s
}

async fn get_latest_remote_packages_on_feed(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    progress_bar.set_message(format!("receive packages from '{}'", feed.name));
    // else - recurse file search + filename analysis
    let https_regex = regex::Regex::new(r"^https?://.+").unwrap();
    match https_regex.is_match(&feed.url) {
        true => {
            let odata_xml = get_odata_xml_packages(progress_bar, pkgs, feed, prerelease)
                .await
                .expect("failed to receive odata for packages");
            Ok(get_packages_from_odata(&odata_xml))
        }
        false => {
            let nupkg_files = get_nupkgs_from_path(pkgs, feed, prerelease)
                .expect("failed to read package info from file system");
            Ok(nupkg_files)
        }
    }
}

fn get_package_from_nupkg(filename: &str) -> Option<Package> {
    // println!(" .. pkg from filename: {}", filename);
    // TODO - is this sufficient? / do we need to extract the nuspec from the nupkg in order to get the id / version ?
    let semver_regex = regex::Regex::new(r#"^(.+?)\.(((\d+\.?)+)(-.+)?)\.nupkg$"#).unwrap();
    match semver_regex.captures(filename) {
        Some(captures) => {
            // println!("{:#?}", captures);
            Some(Package {
                id: captures
                    .get(1)
                    .map_or(String::from(""), |m| String::from(m.as_str())),
                version: captures
                    .get(2)
                    .map_or(String::from(""), |m| String::from(m.as_str())),
                pinned: false,
            })
        }
        None => {
            println!("ERROR: failed to get package from filename '{}'", filename);
            None
        }
    }
}

fn get_nupkgs_from_path(
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut feed_dir = PathBuf::from(&feed.url);
    feed_dir.push("**/*.nupkg");

    let mut packages: Vec<Package> = Vec::new();
    for entry in glob::glob(&feed_dir.to_string_lossy())? {
        match get_package_from_nupkg(entry?.file_name().unwrap().to_str().unwrap()) {
            Some(p) => {
                let version = semver::Version::parse(&p.version).unwrap();
                if !prerelease && !version.pre.is_empty() {
                    continue;
                }
                if pkgs
                    .iter()
                    .any(|s| s.id.to_lowercase() == p.id.to_lowercase())
                {
                    packages.push(p);
                }
            }
            None => {}
        }
    }

    Ok(packages)
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
                if !semver_is_newer(&p.version, remote_version) {
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
    limitoutput: bool,
    prerelease: bool,
    ignore_pinned: bool,
    ignore_unfound: bool,
) -> String {
    // foreach local package, compare remote version number
    let local_packages = get_local_packages().expect("failed to get local package list");
    let remote_feeds = get_choco_sources().expect("failed to get choco feeds");
    let remote_feeds = remote_feeds
        .into_iter()
        .filter(|f| f.disabled == false)
        .collect();
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
                if semver_is_newer(&u.version, &l.version) {
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

async fn get_odata_xml_packages(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut query_res = String::new();
    let latest_filter = match prerelease {
        true => "IsAbsoluteLatestVersion",
        false => "IsLatestVersion",
    };
    let query_string_base: String = format!("{}/Packages?$filter={}", feed.url, latest_filter);
    let total_pkgs = pkgs.len();
    let mut curr_pkg_idx = 0;

    // https://chocolatey.org/api/v2/Packages?$filter=IsLatestVersion and (Id eq 'Chocolatey' or Id eq 'Boxstarter' or Id eq 'vscode' or Id eq 'notepadplusplus')

    let mut max_url_len = 2047;

    // NOTE: some feeds may have pagination (such as choco community repo)
    // determine number of packages returned by single request and use it as batch size for this repo from now on 
    let (mut max_batch_size, _) = receive_package_delta(feed, 0, 0, prerelease).await;

    while curr_pkg_idx < total_pkgs {
        // max_batch_size and max_url_len get reduced when communication with the repository fails
        if max_batch_size == 0 || max_url_len < 100 {
            panic!("failed to read from repository '{}'", feed.name)
        }

        let mut query_string = format!("{} and (", query_string_base);
        let mut batch_size = 0;
        let last_query_package_idx = curr_pkg_idx;

        loop {
            let curr_pkg = pkgs.get(curr_pkg_idx).unwrap();
            // query_string.push_str(&format!("(Id eq '{}' or Id eq '{}')", curr_pkg.id, curr_pkg.id.to_lowercase()));
            query_string.push_str(&format!(
                "(tolower(Id) eq '{}')",
                curr_pkg.id.to_lowercase()
            ));
            curr_pkg_idx += 1;
            batch_size += 1;

            let url = reqwest::Url::parse(&query_string);
            if (url.unwrap().as_str().len() > max_url_len)
                || curr_pkg_idx == pkgs.len()
                || batch_size >= max_batch_size
            {
                query_string.push_str(")");
                break;
            }
            query_string.push_str(" or ");
        }

        println_verbose(&format!(" -> GET: {}", query_string));
        let client = build_reqwest(&feed);
        let resp_odata = client.get(&query_string).send().await?;

        if !resp_odata.status().is_success() {
            println_verbose(&format!("  HTTP STATUS {}", resp_odata.status().as_str()));
        }

        // if we get a client err response - try reducing url length (first)
        if resp_odata.status().is_client_error() {
            max_url_len = max_url_len / 2;
            println_verbose(&format!("  reduced max url length: {}", max_url_len));
            curr_pkg_idx = last_query_package_idx;
            continue;
        }

        let resp_odata = resp_odata.text().await.unwrap_or_default();

        // if we still get an invalid response - try reducing the batch query size...
        if resp_odata.is_empty() {
            max_batch_size -= 1;
            println_verbose(&format!("  reduced receive batch size: {}", max_batch_size));
            curr_pkg_idx = last_query_package_idx;
            continue;
        }
        query_res.push_str(&resp_odata);
        progress_bar.set_position(curr_pkg_idx as u64);
    }

    Ok(query_res)
}

fn get_packages_from_odata(odata_xml: &str) -> Vec<Package> {
    let mut packages = Vec::new();
    let mut pkg_name = String::new();
    let mut pkg_version = String::new();

    let mut reader = Reader::from_str(odata_xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    // entry/title -> id
    // entry/m:properties/d:Version -> Version

    enum ODataParserState {
        LookingForEntry,
        InEntry,
        InEntryId,
        InEntryVersion,
    }

    let mut state = ODataParserState::LookingForEntry;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"entry" => state = ODataParserState::InEntry,
                b"title" => match state {
                    ODataParserState::InEntry => {
                        state = ODataParserState::InEntryId;
                    }
                    _ => {}
                },
                b"d:Version" => match state {
                    ODataParserState::InEntry => {
                        state = ODataParserState::InEntryVersion;
                    }
                    _ => {}
                },
                _ => {}
            },
            Ok(Event::Text(e)) => match state {
                ODataParserState::InEntryId => {
                    pkg_name = String::from_utf8(e.to_vec()).unwrap();
                }
                ODataParserState::InEntryVersion => {
                    pkg_version = String::from_utf8(e.to_vec()).unwrap()
                }
                _ => (),
            },
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"entry" => {
                    println_verbose(&format!(
                        "  package_from_odata: {}, version={}",
                        pkg_name, pkg_version
                    ));
                    packages.push(Package {
                        id: pkg_name.to_string(),
                        version: pkg_version.to_string(),
                        pinned: false,
                    });
                    state = ODataParserState::LookingForEntry;
                }
                b"title" => match state {
                    ODataParserState::InEntryId => {
                        state = ODataParserState::InEntry;
                    }
                    _ => {}
                },
                b"d:Version" => match state {
                    ODataParserState::InEntryVersion => {
                        state = ODataParserState::InEntry;
                    }
                    _ => {}
                },
                _ => {}
            },
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    packages
}
