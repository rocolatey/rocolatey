use quick_xml::events::Event;
use quick_xml::Reader;

use crate::println_verbose;
use crate::roco::remote::{build_reqwest, invoke_package_bulk_request};
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

pub(crate) async fn get_remote_packages(
    progress_bar: &indicatif::ProgressBar,
    pkgs: &[Package],
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {

    let latest_filter = match prerelease {
        true => "IsAbsoluteLatestVersion",
        false => "IsLatestVersion",
    };
    let query_string_base: String = format!("{}/Packages?$filter={} and (", feed.url, latest_filter);
    let total_pkgs = pkgs.len();

    // https://chocolatey.org/api/v2/Packages?$filter=IsLatestVersion and (Id eq 'Chocolatey' or Id eq 'Boxstarter' or Id eq 'vscode' or Id eq 'notepadplusplus')

    // NOTE: some feeds may have pagination (such as choco community repo)
    // determine number of packages returned by single request and use it as batch size for this repo from now on
    let (max_batch_size, _) = if total_pkgs == 1 {
        (1 as u32, "".to_string())
    } else {
        receive_package_delta(feed, 0, 0, prerelease).await
    };

    let query_str_delim = " or ".to_owned();
    let query_str_end = ")".to_owned();

    invoke_package_bulk_request(
        progress_bar,
        pkgs,
        feed,
        &query_string_base,
        max_batch_size,
        |p| format!("(tolower(Id) eq '{}')", p.id.to_lowercase()),
        &query_str_delim,
        &query_str_end,
        |pkgs, batch_str| -> () {
            pkgs.extend(get_packages_from_odata(batch_str));
        },
    )
    .await
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
