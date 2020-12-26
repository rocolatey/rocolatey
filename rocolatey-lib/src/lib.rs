use glob::glob;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

enum NuspecTag {
    Null,
    Id,
    Version,
}

pub struct Package {
    id: String,
    version: String,
    pinned: bool,
}

impl Package {
    // access all the members via getters (immutable refs + copies only)
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn pinned(&self) -> bool {
        self.pinned
    }
}

pub fn get_chocolatey_dir() -> Result<String, std::env::VarError> {
    let key = "ChocolateyInstall";
    match env::var(key) {
        Ok(val) => Ok(String::from(val)),
        Err(e) => Err(e),
    }
}

pub fn get_local_packages() -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut pkgs: Vec<Package> = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut pkg_dir = PathBuf::from(choco_dir);
    pkg_dir.push("lib");
    pkg_dir.push("*/*.nuspec");
    for entry in glob(&pkg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => pkgs.push(get_package_from_nuspec(&path)),
            Err(e) => println!("{:?}", e),
        }
    }
    Ok(pkgs)
}

pub fn get_local_bad_packages() -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut pkgs: Vec<Package> = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut pkg_dir = PathBuf::from(choco_dir);
    pkg_dir.push("lib-bad");
    pkg_dir.push("*/*.nuspec");
    for entry in glob(&pkg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => pkgs.push(get_package_from_nuspec(&path)),
            Err(e) => println!("{:?}", e),
        }
    }
    Ok(pkgs)
}

pub fn get_local_packages_text(limitoutput: bool) -> String {
    let packages = get_local_packages().unwrap();
    let num_packages = packages.len();
    let mut res = String::new();
    res.push_str(get_package_list_text(packages, limitoutput).as_ref());
    if !limitoutput {
        res.push_str(&format!("\r\n{} packages installed.", num_packages));
    }
    res
}

pub fn get_local_bad_packages_text(limitoutput: bool) -> String {
    let packages = get_local_bad_packages().unwrap();
    let num_packages = packages.len();
    let mut res = String::new();
    res.push_str(get_package_list_text(packages, limitoutput).as_ref());
    if !limitoutput {
        res.push_str(&format!("\r\n{} packages in lib-bad.", num_packages));
    }
    res
}

fn get_package_list_text(packages: Vec<Package>, limitoutput: bool) -> String {
    let mut res = String::new();
    let num_iterations = packages.len() - 1;
    let sep = if limitoutput { "|" } else { " " };
    for (i, p) in packages.iter().enumerate() {
        res.push_str(&format!("{}{}{}", p.id(), sep, p.version()));
        if i < num_iterations {
            res.push_str("\r\n");
        }
    }
    res
}

fn get_package_from_nuspec(pkgs_path: &std::path::PathBuf) -> Package {
    assert_eq!(true, pkgs_path.is_file());
    let mut pkg_name = String::new();
    let mut pkg_version = String::new();

    let mut reader = Reader::from_file(pkgs_path).expect("failed to init xml reader");
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut tag: NuspecTag = NuspecTag::Null;

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"id" => tag = NuspecTag::Id,
                b"version" => tag = NuspecTag::Version,
                _ => tag = NuspecTag::Null,
            },
            Ok(Event::Text(e)) => match tag {
                NuspecTag::Id => pkg_name = String::from_utf8(e.to_vec()).unwrap(),
                NuspecTag::Version => pkg_version = String::from_utf8(e.to_vec()).unwrap(),
                _ => (),
            },
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    Package {
        id: pkg_name.to_string(),
        version: pkg_version.to_string(),
        pinned: false,
    }
}

pub struct Feed {
    name: String,
    url: String,
}

fn get_choco_sources() -> Result<Vec<Feed>, std::io::Error> {
    let mut s = Vec::new();
    s.push(Feed {
        name: String::from("chocolatey"),
        url: String::from("https://chocolatey.org/api/v2"),
    });
    Ok(s)
}

async fn get_package_count_on_feed(f: &Feed) -> u32 {
    let latest_filter = "$filter=IsAbsoluteLatestVersion";
    let rs = format!("{}/Packages()/$count?{}", f.url, latest_filter);
    let resp_pkg_count = reqwest::get(&rs).await;
    let total_pkg_count = resp_pkg_count.unwrap().text().await.unwrap();
    let total_pkg_count = total_pkg_count.parse::<u32>().unwrap();
    total_pkg_count
}

async fn receive_package_delta(feed: &Feed, batch_size: u32, batch_offset: u32) -> (u32, String) {
    let base_uri = format!("{}/Packages()", feed.url);
    let latest_filter = "$filter=IsAbsoluteLatestVersion";
    let rs = match batch_size {
        0 => {
            format!("{}?{}&$skip={}", base_uri, latest_filter, batch_offset)
        }
        _ => {
            format!(
                "{}?{}&$top={}&$skip={}",
                base_uri, latest_filter, batch_size, batch_offset
            )
        }
    };
    // println!("q: {}", rs);
    let resp = reqwest::get(&rs).await;
    let query_res = resp.unwrap().text().await.unwrap();
    let c = query_res.matches("</entry>").count();
    (c as u32, query_res)
}

async fn update_feed_index(feed: &Feed, limitoutput: bool) -> String {
    let total_pkg_count = get_package_count_on_feed(feed).await;
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
        let (a, req_res) = receive_package_delta(feed, batch_size, received_packages).await;
        if a != batch_size {
            println!("receiving packages in batches of {} per request", a);
        }
        batch_size = a;
        f.write_all(req_res.as_bytes())
            .expect("unable to write data");

        received_packages += batch_size;
        progress_bar.set_position(received_packages as u64);
    }
    progress_bar.finish();
    f.flush().expect("failed to flush file buffer");
    // TODO - "shrink" pkg index files - only keep id + version (faster lookup later on)

    String::from("not implemented")
}

pub async fn update_package_index(limitoutput: bool) -> String {
    // foreach configured source, download package index
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html
    // https://joelverhagen.github.io/NuGetUndocs/
    // http://docs.oasis-open.org/odata/odata/v4.0/errata03/os/complete/part1-protocol/odata-v4.0-errata03-os-part1-protocol-complete.html
    // https://chocolatey.org/api/v2/Packages/$count
    // https://chocolatey.org/api/v2/Packages?$top=40&$skip=500
    // https://chocolatey.org/api/v2/Packages()/$count?$filter=IsAbsoluteLatestVersion
    // https://chocolatey.org/api/v2/Packages()?$filter=IsAbsoluteLatestVersion
    let mut s = String::new();
    let feeds = get_choco_sources().expect("failed to get choco sources");
    for f in feeds {
        s.push_str(&update_feed_index(&f, limitoutput).await);
    }
    s
}

pub fn get_outdated_packages(limitoutput: bool) -> String {
    // deserialize remote package index, only keep latest version per id
    // get local package info
    // foreach local package info, check if newer version exists on remote
    String::from("not implemented")
}
