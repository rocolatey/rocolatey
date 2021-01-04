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
    pub exists_on_remote: bool,
}

fn semver_is_newer(a: &str, b: &str) -> bool {
    let a = semver::Version::parse(a);
    let b = semver::Version::parse(b);
    a > b
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

fn get_feed_from_source_attribs(attrs: &mut quick_xml::events::attributes::Attributes) -> Feed {
    let attrib_map = xml_attribs_to_map(attrs);

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

    Feed {
        name: name.clone(),
        url: url.clone(),
        credential: cred,
        proxy: None,
    }
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
    for mut s in sources {
        s.proxy = proxy_config.clone();
        sources_with_proxy.push(s);
    }
    Ok(sources_with_proxy)
}
