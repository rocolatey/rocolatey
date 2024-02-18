use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod local;
pub mod nuget2;
pub mod nuget3;
pub mod remote;
pub mod semver;
use crate::println_verbose;

#[derive(Debug)]
pub enum NuspecTag {
    Null,
    Id,
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedType {
    Unknown,
    LocalFileSystem,
    NuGetV2,
    NuGetV3,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub id: String,
    pub version: String,
    pub pinned: bool,
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

#[derive(Debug, Clone)]
pub struct Feed {
    pub name: String,
    pub url: String,
    pub credential: Option<Credential>,
    pub proxy: Option<ProxySettings>,
    pub disabled: bool,
    pub certificate: Option<String>,
    pub bypass_proxy: bool,
    pub self_service: bool,
    pub admin_only: bool,
    pub priority: i64,
    pub feed_type: FeedType,
    pub service_index: Option<nuget3::NuGetV3Index>,
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Clone)]
pub struct ProxySettings {
    pub url: String,
    pub credential: Option<Credential>,
}

#[derive(Debug, Clone)]
pub struct OutdatedInfo {
    pub id: String,
    pub local_version: String,
    pub remote_version: String,
    pub pinned: bool,
    pub outdated: bool,
    pub exists_on_remote: bool,
}

fn xml_attribs_to_map(
    attrs: &mut quick_xml::events::attributes::Attributes,
) -> HashMap<String, String> {
    attrs
        .map(|a| {
            let a = a.unwrap();
            (
                String::from_utf8(Vec::from(a.key.as_ref())).unwrap(),
                String::from_utf8(Vec::from(a.value)).unwrap(),
            )
        })
        .collect::<HashMap<String, String>>()
}

fn get_feed_from_source_attribs(
    attrs: &mut quick_xml::events::attributes::Attributes,
) -> Option<Feed> {
    let attrib_map = xml_attribs_to_map(attrs);

    let disabled = match attrib_map.get("disabled") {
        Some(attrib_value) => attrib_value == "true",
        None => false,
    };

    let name = attrib_map.get("id").unwrap();
    let url = attrib_map.get("value").unwrap();
    let user = attrib_map.get("user");
    let password = attrib_map.get("password");

    println_verbose(&format!(
        "feed '{}' -> '{}' | disabled: {}",
        name, url, disabled
    ));

    // NOTE: don't need to decrypt when feed is disabled anyway
    let cred = match !disabled && user.is_some() && password.is_some() {
        true => Some(Credential {
            user: user.unwrap().clone(),
            pass: decrypt_choco_config_string(password.unwrap()),
        }),
        false => None,
    };

    Some(Feed {
        name: name.clone(),
        url: url.clone(),
        credential: cred,
        proxy: None,
        disabled: disabled,
        feed_type: FeedType::Unknown,
        service_index: None,
        certificate: match attrib_map.get("certificate") {
            Some(c) => Some(c.clone()),
            None => None,
        },
        bypass_proxy: match attrib_map.get("bypass_proxy") {
            Some(c) => c == "true",
            None => false,
        },
        self_service: match attrib_map.get("self_service") {
            Some(c) => c == "true",
            None => false,
        },
        admin_only: match attrib_map.get("admin_only") {
            Some(c) => c == "true",
            None => false,
        },
        priority: match attrib_map.get("priority") {
            Some(c) => c.parse::<i64>().unwrap_or(0),
            None => 0,
        },
    })
}

fn get_config_settings_from_attribs(
    config_settings: &mut HashMap<String, String>,
    attrs: &mut quick_xml::events::attributes::Attributes,
) {
    let attrib_map = xml_attribs_to_map(attrs);
    let val = attrib_map.get("value");
    if val.is_some() {
        config_settings.insert(attrib_map.get("key").unwrap().clone(), val.unwrap().clone());
    }
}

fn get_chocolatey_dir() -> Result<String, std::env::VarError> {
    let key = "ChocolateyInstall";
    match std::env::var(key) {
        Ok(val) => Ok(String::from(val)),
        Err(e) => {
            eprintln!("failed to get choco dir 'ChocolateyInstall': {}", e);
            ::std::process::exit(1)
        }
    }
}

fn get_choco_sources() -> Result<Vec<Feed>, std::io::Error> {
    let mut sources = Vec::new();
    let choco_dir = get_chocolatey_dir().expect("failed to get choco dir");
    let mut cfg_dir = PathBuf::from(choco_dir);
    cfg_dir.push("config/chocolatey.config");

    println_verbose(&format!("parse '{}'", cfg_dir.to_str().unwrap()));
    let mut config_settings: HashMap<String, String> = HashMap::new();

    for entry in glob::glob(&cfg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                let mut reader = Reader::from_file(path).expect("failed to init xml reader");
                reader.trim_text(true);
                let mut buf = Vec::new();
                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                            b"source" => {
                                sources.push(get_feed_from_source_attribs(&mut e.attributes()));
                            }
                            b"add" => {
                                get_config_settings_from_attribs(
                                    &mut config_settings,
                                    &mut e.attributes(),
                                );
                            }
                            _ => {}
                        },
                        Ok(Event::Start(ref e)) => match e.name().as_ref() {
                            b"source" => {
                                sources.push(get_feed_from_source_attribs(&mut e.attributes()));
                            }
                            b"add" => {
                                get_config_settings_from_attribs(
                                    &mut config_settings,
                                    &mut e.attributes(),
                                );
                            }
                            _ => {}
                        },
                        Ok(Event::Eof) => break,
                        _ => (),
                    }
                    buf.clear();
                }
            }
            Err(e) => println!("{:?}", e),
        }
    }
    // println!("{:#?}", config_settings);
    let proxy_config = match config_settings.get("proxy") {
        Some(proxy_url) => match proxy_url.is_empty() {
            true => None,
            false => Some(ProxySettings {
                url: proxy_url.clone(),
                credential: match config_settings.get("proxyUser") {
                    Some(proxy_user) => Some(Credential {
                        user: proxy_user.clone(),
                        pass: match config_settings.get("proxyPassword") {
                            Some(proxy_pass) => decrypt_choco_config_string(proxy_pass),
                            None => String::new(),
                        },
                    }),
                    None => None,
                },
            }),
        },
        None => None,
    };

    let mut sources_with_proxy = Vec::new();
    for s in sources {
        match s {
            Some(mut feed) => {
                feed.proxy = proxy_config.clone();
                sources_with_proxy.push(feed);
            }
            None => {}
        }
    }
    Ok(sources_with_proxy)
}

fn decrypt_choco_config_string(encrypted: &str) -> String {
    // TODO replace by using native dpadpi.CryptUnprotectData ??
    println_verbose(&format!("decypher '{}'", encrypted));
    let pwsh = format!(
        "Add-Type -AssemblyName System.Security;([System.Text.UTF8Encoding]::UTF8.GetString([System.Security.Cryptography.ProtectedData]::Unprotect(([System.Convert]::FromBase64String('{}')),([System.Text.UTF8Encoding]::UTF8.GetBytes('Chocolatey')),[System.Security.Cryptography.DataProtectionScope]::LocalMachine)))",
        encrypted
    );
    let chdec = std::process::Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg(pwsh)
        .output()
        .expect("failed to run decypher text");
    let decrypted = String::from_utf8_lossy(&chdec.stdout);
    let res = decrypted.trim(); // remove newlines
    res.to_string()
}

#[cfg(test)]
mod tests {

    use super::*;
    //NOTE: ChocolateyInstall, RocolateyTestRoot env-vars needs to be set in via Cargo [env]

    #[test]
    fn get_chocolatey_dir_test() {
        // env:ChocolateyInstall will be set by cargo [env]
        assert!(get_chocolatey_dir().is_ok());
    }

    #[tokio::test]
    async fn get_choco_sources_test() {
        let sources = get_choco_sources();
        assert!(sources.is_ok());
        let sources = sources.unwrap();
        sources.iter().cloned().for_each(|s| {
            assert!(!s.name.is_empty());
            assert!(!s.url.is_empty());
        });

        let choco_source: &Feed = sources.iter().find(|s| s.name == "chocolatey").unwrap();

        assert_eq!(choco_source.name, "chocolatey");
        assert_eq!(choco_source.url, "https://chocolatey.org/api/v2");
        assert_eq!(choco_source.priority, 101);
        assert_eq!(choco_source.admin_only, false);
        assert_eq!(choco_source.bypass_proxy, false);
        assert_eq!(choco_source.certificate, None);
        assert!(choco_source.credential.is_none());
        // the FeedType is only evaluated when needed/actually used
        assert_eq!(choco_source.feed_type, FeedType::Unknown);
        // TODO: verify choco_source.evaluate_feed_type works correctly
    }
}
