use quick_xml::events::Event;
use quick_xml::Reader;

use crate::println_verbose;
use crate::roco::remote::build_reqwest;
use crate::roco::{Feed, Package};

// https://joelverhagen.github.io/NuGetUndocs/
// http://docs.oasis-open.org/odata/odata/v4.0/errata03/os/complete/part1-protocol/odata-v4.0-errata03-os-part1-protocol-complete.html

/*
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
*/

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

pub(crate) async fn get_odata_xml_packages(
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
    let (mut max_batch_size, _) = if total_pkgs == 1 {
        (1 as u32, "".to_string())
    } else {
        receive_package_delta(feed, 0, 0, prerelease).await
    };

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

pub(crate) fn get_packages_from_odata(odata_xml: &str) -> Vec<Package> {
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
