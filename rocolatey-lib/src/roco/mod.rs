use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod local;
pub mod remote;

pub enum NuspecTag {
    Null,
    Id,
    Version,
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

fn my_semver_is_newer(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split(".").collect();
    let b_parts: Vec<&str> = b.split(".").collect();

    for (i, v_a) in a_parts.iter().enumerate() {
        if b_parts.len() <= i {
            return true;
        }
        let n_a = v_a.parse::<i32>();
        let n_b = b_parts.get(i).unwrap().parse::<i32>();
        if n_a.is_ok() && n_b.is_err() {
            // a is digit, b is something else
            return true;
        }
        if n_a.is_err() && n_b.is_ok() {
            // a is not a digit, but b is
            return false;
        }
        if n_a.is_ok() && n_b.is_ok() {
            let n_a = n_a.unwrap();
            let n_b = n_b.unwrap();
            if n_a > n_b {
                return true;
            }
            if n_b > n_a {
                return false;
            }
            continue;
        }
        if n_a.is_err() && n_b.is_err() {
            // string compare
            return v_a > b_parts.get(i).unwrap();
        }
    }
    return false;
}

fn semver_is_newer(a: &str, b: &str) -> bool {
    let r = my_semver_is_newer(a, b);
    /*
    let a = semver::Version::parse(a).unwrap();
    let b = semver::Version::parse(b).unwrap();
    // rust semver can only do 3 digits!
    let r = a > b
    */
    r
}

pub fn get_chocolatey_dir() -> Result<String, std::env::VarError> {
    let key = "ChocolateyInstall";
    match std::env::var(key) {
        Ok(val) => Ok(String::from(val)),
        Err(e) => Err(e),
    }
}

fn xml_attribs_to_map(
    attrs: &mut quick_xml::events::attributes::Attributes,
) -> HashMap<String, String> {
    attrs
        .map(|a| {
            let a = a.unwrap();
            (
                String::from_utf8(Vec::from(a.key)).unwrap(),
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

    let cred = match user.is_some() && password.is_some() {
        true => Some(Credential {
            user: user.unwrap().clone(),
            pass: password.unwrap().clone(),
        }),
        false => None,
    };

    Some(Feed {
        name: name.clone(),
        url: url.clone(),
        credential: cred,
        proxy: None,
        disabled: disabled,
        // TODO set actual values!
        certificate: None,
        bypass_proxy: false,
        self_service: false,
        admin_only: false,
        priority: 0,
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

fn get_choco_sources() -> Result<Vec<Feed>, std::io::Error> {
    let mut sources = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut cfg_dir = PathBuf::from(choco_dir);
    cfg_dir.push("config/chocolatey.config");

    let mut config_settings: HashMap<String, String> = HashMap::new();

    for entry in glob::glob(&cfg_dir.to_string_lossy()).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                let mut reader = Reader::from_file(path).expect("failed to init xml reader");
                reader.trim_text(true);
                let mut buf = Vec::new();
                loop {
                    match reader.read_event(&mut buf) {
                        Ok(Event::Empty(ref e)) => match e.name() {
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
                        Ok(Event::Start(ref e)) => match e.name() {
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
    if config_settings.get("proxy").is_some() {}
    let proxy_config = match config_settings.get("proxy") {
        Some(proxy_url) => match proxy_url.is_empty() {
            true => None,
            false => Some(ProxySettings {
                url: proxy_url.clone(),
                credential: match config_settings.get("proxyUser") {
                    Some(proxy_user) => Some(Credential {
                        user: proxy_user.clone(),
                        pass: match config_settings.get("proxyPassword") {
                            Some(proxy_pass) => proxy_pass.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_is_newer_test() {
        assert_eq!(2 + 2, 4);
        assert!(semver_is_newer("2.1.0", "1.0.0"));
        assert!(semver_is_newer("2.0.2", "1.12.0"));
        assert!(semver_is_newer("1.2.3.4", "1.2.3"));
        assert!(semver_is_newer("1.1.0-alpha", "1.0.0"));
        assert!(semver_is_newer("1.1.0-beta2", "1.1.0-beta1"));
        assert!(semver_is_newer("1.1.0-beta2", "1.1.0-beta14")); // this is weird, but matches choco behavior
        assert!(!semver_is_newer("1.12.0", "2.0.2"));
        assert!(!semver_is_newer("1.3.0", "2.1.0"));
        assert!(!semver_is_newer("1.2.3", "1.2.3.1"));
        assert!(!semver_is_newer("1.1.0-alpha", "1.1.0"));
    }
}
