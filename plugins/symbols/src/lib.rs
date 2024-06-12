use std::{collections::HashMap, fs};

use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use fuzzy_matcher::FuzzyMatcher;
use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/unicode.rs"));

#[derive(Clone, Debug)]
struct Symbol {
    chr: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    prefix: String,
    symbols: HashMap<String, String>,
    max_entries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prefix: "".to_string(),
            symbols: HashMap::new(),
            max_entries: 3,
        }
    }
}

struct State {
    config: Config,
    symbols: Vec<Symbol>,
}

#[init]
fn init(config_dir: RString) -> State {
    // Try to load the config file, if it does not exist only use the static unicode characters
    let config = if let Ok(content) = fs::read_to_string(format!("{}/symbols.ron", config_dir)) {
        ron::from_str(&content).unwrap_or_default()
    } else {
        Config::default()
    };

    let symbols = UNICODE_CHARS
        .iter()
        .map(|(name, chr)| (name.to_string(), chr.to_string()))
        .chain(config.symbols.clone().into_iter())
        .map(|(name, chr)| Symbol { chr, name })
        .collect();

    State { config, symbols }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Symbols".into(),
        icon: "accessories-character-map".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, state: &State) -> RVec<Match> {
    let input = if let Some(input) = input.strip_prefix(&state.config.prefix) {
        input.trim()
    } else {
        return RVec::new();
    };
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case();
    let mut symbols = state
        .symbols
        .iter()
        .filter_map(|symbol| {
            matcher
                .fuzzy_match(&symbol.name, input)
                .map(|score| (symbol, score))
        })
        .collect::<Vec<_>>();

    // Sort the symbol list according to the score
    symbols.sort_by(|a, b| b.1.cmp(&a.1));

    symbols.truncate(state.config.max_entries);

    symbols
        .into_iter()
        .map(|(symbol, _)| Match {
            title: symbol.chr.clone().into(),
            description: ROption::RSome(symbol.name.clone().into()),
            use_pango: false,
            icon: ROption::RNone,
            image: ROption::RNone,
            id: ROption::RNone,
        })
        .collect()
}

#[handler]
fn handler(selection: Match) -> HandleResult {
    HandleResult::Copy(selection.title.into_bytes())
}
