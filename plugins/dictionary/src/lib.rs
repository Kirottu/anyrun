use std::fs;

use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    prefix: String,
    max_entries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prefix: ":def".to_string(),
            max_entries: 3,
        }
    }
}

#[derive(Deserialize)]
struct ApiResponse {
    meanings: Vec<Meaning>,
}

#[derive(Deserialize)]
struct Meaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    definitions: Vec<Definition>,
}

#[derive(Deserialize)]
struct Definition {
    definition: String,
}

#[init]
pub fn init(config_dir: RString) -> Config {
    match fs::read_to_string(format!("{}/dictionary.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

#[handler]
pub fn handler(_match: Match) -> HandleResult {
    HandleResult::Copy(_match.title.into_bytes())
}

#[get_matches]
pub fn get_matches(input: RString, config: &Config) -> RVec<Match> {
    let input = if let Some(input) = input.strip_prefix(&config.prefix) {
        input.trim()
    } else {
        return RVec::new();
    };

    let responses: Vec<ApiResponse> = match reqwest::blocking::get(format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        input
    )) {
        Ok(response) => match response.json() {
            Ok(response) => response,
            Err(why) => {
                eprintln!("Error deserializing response: {}", why);
                return RVec::new();
            }
        },
        Err(why) => {
            eprintln!("Error fetching dictionary result: {}", why);
            return RVec::new();
        }
    };

    responses
        .into_iter()
        .flat_map(|response| {
            response
                .meanings
                .into_iter()
                .flat_map(|meaning| {
                    meaning
                        .definitions
                        .into_iter()
                        .map(|definition| Match {
                            title: definition.definition.into(),
                            description: ROption::RSome(meaning.part_of_speech.clone().into()),
                            use_pango: false,
                            icon: ROption::RSome("accessories-dictionary".into()),
                            image: ROption::RNone,
                            id: ROption::RNone,
                        })
                        .collect::<RVec<_>>()
                })
                .collect::<RVec<_>>()
        })
        .take(config.max_entries)
        .collect()
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Dictionary".into(),
        icon: "accessories-dictionary".into(),
    }
}
