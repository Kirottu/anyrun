use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use serde::{Deserialize, Serialize};
use std::{fmt, fs, process::Command};
use urlencoding::encode;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Engine {
    Google,
    Ecosia,
    Bing,
    DuckDuckGo,
    Custom { name: String, url: String },
}

impl Engine {
    fn value(&self) -> &str {
        match self {
            Self::Google => "google.com/search?q={}",
            Self::Ecosia => "www.ecosia.org/search?q={}",
            Self::Bing => "www.bing.com/search?q={}",
            Self::DuckDuckGo => "duckduckgo.com/?q={}",
            Self::Custom { url, .. } => url,
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Google => write!(f, "Google"),
            Self::Ecosia => write!(f, "Ecosia"),
            Self::Bing => write!(f, "Bing"),
            Self::DuckDuckGo => write!(f, "DuckDuckGo"),
            Self::Custom { name, .. } => write!(f, "{}", name),
        }
    }
}

#[derive(Deserialize, Debug)]
struct Config {
    prefix: String,
    engines: Vec<Engine>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            prefix: "?".to_string(),
            engines: vec![Engine::Google],
        }
    }
}

#[init]
fn init(config_dir: RString) -> Config {
    match fs::read_to_string(format!("{}/websearch.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Websearch".into(),
        icon: "help-about".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, config: &Config) -> RVec<Match> {
    if !input.starts_with(&config.prefix) {
        RVec::new()
    } else {
        config
            .engines
            .iter()
            .enumerate()
            .map(|(i, engine)| Match {
                title: input.trim_start_matches(&config.prefix).into(),
                description: ROption::RSome(format!("Search with {}", engine).into()),
                use_pango: false,
                icon: ROption::RNone,
                image: ROption::RNone,
                id: ROption::RSome(i as u64),
            })
            .collect()
    }
}

#[handler]
fn handler(selection: Match, config: &Config) -> HandleResult {
    let engine = &config.engines[selection.id.unwrap() as usize];

    if let Err(why) = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "xdg-open https://{}",
            engine
                .value()
                .replace("{}", &encode(&selection.title.to_string()))
        ))
        .spawn()
    {
        println!("Failed to perform websearch: {}", why);
    }

    HandleResult::Close
}
