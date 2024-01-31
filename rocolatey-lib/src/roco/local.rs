use quick_xml::events::Event;
use quick_xml::Reader;

use std::path::PathBuf;

use crate::println_verbose;
use crate::roco::{get_choco_sources, get_chocolatey_dir, Feed, NuspecTag, Package};

pub fn get_local_packages() -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut pkgs: Vec<Package> = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap_or_else(|_| String::from("."));
    let mut pkg_dir = PathBuf::from(choco_dir);
    pkg_dir.push("lib");
    pkg_dir.push("*/*.nuspec");
    for entry in glob::glob(&pkg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => pkgs.push(get_package_from_nuspec(&path)),
            Err(e) => println!("{:?}", e),
        }
    }
    Ok(pkgs)
}

pub fn get_local_bad_packages() -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut pkgs: Vec<Package> = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap_or_else(|_| String::from("."));
    let mut pkg_dir = PathBuf::from(choco_dir);
    pkg_dir.push("lib-bad");
    // NOTE: using a '**' glob may be a bad idea
    // (recursive search may be slow + what if someone includes nuspec in a package?)
    pkg_dir.push("**/*.nuspec");
    for entry in glob::glob(&pkg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
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

pub fn get_sources_text(limitoutput: bool) -> String {
    let mut res = String::new();
    let sources = get_choco_sources().unwrap();

    fn c_bool(v: bool) -> &'static str {
        match v {
            true => "True",
            false => "False",
        }
    }

    let num_iterations = sources.len() - 1;
    for (i, f) in sources.iter().enumerate() {
        res.push_str(
            &(match limitoutput {
                true => {
                    let user = match &f.credential {
                        Some(auth) => auth.user.clone(),
                        None => String::new(),
                    };
                    let certificate = (f.certificate.as_ref().unwrap_or(&String::new())).clone();
                    format!(
                        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                        f.name,
                        f.url,
                        c_bool(f.disabled),
                        user,
                        certificate,
                        f.priority,
                        c_bool(f.bypass_proxy),
                        c_bool(f.self_service),
                        c_bool(f.admin_only),
                    )
                }
                false => {
                    let name_1 = match f.disabled {
                        true => format!("{} [Disabled]", f.name),
                        false => f.name.clone(),
                    };
                    let name_2 = match &f.credential {
                        Some(_) => format!("{} (Authenticated)", f.url),
                        None => format!("{} ", f.url),
                    };
                    format!(
                        "{} - {}| Priority {}|Bypass Proxy - {}|Self-Service - {}|Admin Only - {}.",
                        name_1,
                        name_2,
                        f.priority,
                        c_bool(f.bypass_proxy),
                        c_bool(f.self_service),
                        c_bool(f.admin_only)
                    )
                }
            }),
        );
        if i < num_iterations {
            res.push_str("\r\n");
        }
    }

    res
}

fn get_package_list_text(packages: Vec<Package>, limitoutput: bool) -> String {
    let mut res = String::new();
    let num_iterations = packages.len();
    let sep = if limitoutput { "|" } else { " " };
    for (i, p) in packages.iter().enumerate() {
        res.push_str(&format!("{}{}{}", p.id(), sep, p.version()));
        if i < (num_iterations - 1) {
            res.push_str("\r\n");
        }
    }
    res
}

fn get_package_from_nuspec(pkgs_path: &std::path::PathBuf) -> Package {
    assert_eq!(true, pkgs_path.is_file());
    let mut pkg_name = String::new();
    let mut pkg_version = String::new();

    println_verbose(&format!("parse nuspec '{}'", pkgs_path.to_str().unwrap()));
    let mut reader = Reader::from_file(pkgs_path).expect("failed to init xml reader");
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut tag: NuspecTag = NuspecTag::Null;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"id" => tag = NuspecTag::Id,
                b"version" => tag = NuspecTag::Version,
                _ => tag = NuspecTag::Null,
            },
            Ok(Event::Text(e)) => match tag {
                // always ensure package ids are lowercase
                NuspecTag::Id => pkg_name = String::from_utf8(e.to_vec()).unwrap(),
                NuspecTag::Version => pkg_version = String::from_utf8(e.to_vec()).unwrap(),
                _ => (),
            },
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    // 'pinned': $env:ChocolateyInstall\.chocolatey\<pkg_id>.<pkg_version>\.pin -> exists => true
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut pinned_file = PathBuf::from(choco_dir);
    pinned_file.push(".chocolatey");
    pinned_file.push(format!(
        "{}.{}",
        pkg_name.to_string(),
        pkg_version.to_string()
    ));
    pinned_file.push(".pin");

    Package {
        id: pkg_name.to_string(),
        version: pkg_version.to_string(),
        pinned: pinned_file.exists(),
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

pub fn get_nupkgs_from_path(
    pkgs: &Vec<Package>,
    feed: &Feed,
    prerelease: bool,
) -> Result<Vec<Package>, Box<dyn std::error::Error>> {
    let mut feed_dir = PathBuf::from(&feed.url);
    feed_dir.push("**/*.nupkg");

    let prerelease_regex = regex::Regex::new(r#"^(\d+\.?){1,4}\-.+"#).unwrap();

    let mut packages: Vec<Package> = Vec::new();
    for entry in glob::glob(&feed_dir.to_string_lossy())? {
        match get_package_from_nupkg(entry?.file_name().unwrap().to_str().unwrap()) {
            Some(p) => {
                // only need to check if a version has a prerelease part,
                // no need to validate / parse the whole thing at this point!
                let version_is_prerelease = prerelease_regex.is_match(&p.version);
                if !prerelease && version_is_prerelease {
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

#[cfg(test)]
mod tests {
    use super::*;
    //NOTE: ChocolateyInstall, RocolateyTestRoot env-vars needs to be set in via Cargo [env]

    #[test]
    fn get_package_from_nupkg_test() {
        let pkg = get_package_from_nupkg("googlechrome.80.0.3987.149.nupkg").unwrap();
        assert_eq!(pkg.id, "googlechrome");
        assert_eq!(pkg.version, "80.0.3987.149");
        assert_eq!(pkg.pinned, false);
    }

    #[test]
    fn get_package_from_nuspec_test() {
        // TODO: look up if package ids are case-sensitive or not
        // NOTE: would need to normalize across local/v2/v3 sources
        // iirc ProGet does 'support' case-sensitive package IDs
        let mut pkg_path = PathBuf::from(
            get_chocolatey_dir().expect("env:ChocolateyInstall must be set during tests!"),
        );
        pkg_path.push("lib");
        pkg_path.push("Chocolatey/Chocolatey.nuspec");

        let pkg = get_package_from_nuspec(&pkg_path);
        assert_eq!(pkg.id, "Chocolatey");
        assert_eq!(pkg.version, "2.0");
    }

    #[test]
    fn get_nupkgs_from_path_test() {
        let tests_root = std::env::var("RocolateyTestRoot").unwrap();
        let mut tests_repo = PathBuf::from(tests_root);
        tests_repo.push("fake_repo");
        let test_repo_str = tests_repo.into_os_string().into_string().unwrap();

        let tests_feed = Feed {
            name: "fake_repo".to_string(),
            url: String::from(test_repo_str.clone()),
            admin_only: false,
            bypass_proxy: false,
            credential: None,
            proxy: None,
            disabled: false,
            certificate: None,
            self_service: false,
            priority: 0,
            feed_type: crate::roco::FeedType::LocalFileSystem,
            service_index: None,
        };

        let pkgs = vec![Package {
            id: "Firefox".to_string(),
            version: "81.0.0.0".to_string(),
            pinned: false,
        }];
        let pkgs = get_nupkgs_from_path(&pkgs, &tests_feed, false).unwrap();

        let test_files = glob::glob(&format!("{}/*", test_repo_str)).unwrap();
        assert!(test_files.count() >= 2);
        // although there are additional packages in the feed,
        // we only get the the ones we asked for
        assert!(pkgs.len() == 1);

        // prerelease-package are only picked up when explicitly required
        let pkgs = get_nupkgs_from_path(&pkgs, &tests_feed, true).unwrap();
        assert!(pkgs.len() == 2);
    }
}
