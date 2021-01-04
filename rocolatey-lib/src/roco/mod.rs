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

fn get_feed_from_source_attribs(attrs: &mut quick_xml::events::attributes::Attributes) -> Feed {
    let attrib_map = attrs
        .map(|a| {
            let a = a.unwrap();
            (
                String::from_utf8(Vec::from(a.key)).unwrap(),
                String::from_utf8(Vec::from(a.value)).unwrap(),
            )
        })
        .collect::<HashMap<String, String>>();

    let name = attrib_map.get("id").unwrap();
    let url = attrib_map.get("value").unwrap();
    let user = attrib_map.get("user");
    let password = attrib_map.get("password");

    let cred = match user.is_some() && password.is_some() {
        true => Some(Credential {
            user: user.unwrap().clone(),
            pass: password.unwrap().clone(),
        }),
        false => None
    };

    // TODO set proxy data

    Feed {
        name: name.clone(),
        url: url.clone(),
        credential: cred,
        proxy: None,
    }
}

fn get_choco_sources() -> Result<Vec<Feed>, std::io::Error> {
    let mut sources = Vec::new();
    let choco_dir = get_chocolatey_dir().unwrap();
    let mut cfg_dir = PathBuf::from(choco_dir);
    cfg_dir.push("config/chocolatey.config");

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
                            _ => {}
                        },
                        Ok(Event::Start(ref e)) => match e.name() {
                            b"source" => {
                                sources.push(get_feed_from_source_attribs(&mut e.attributes()));
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
    Ok(sources)
}
