use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::{anyrun_interface::HandleResult, *};
use fuzzy_matcher::FuzzyMatcher;
use scrubber::DesktopEntry;
use serde::Deserialize;
use std::{env, fs, process::Command};

#[derive(Deserialize)]
pub struct Config {
    desktop_actions: bool,
    max_entries: usize,
    terminal: Option<String>,
    history_size: usize,     
}

impl Default for Config {
    fn default() -> Self {
        Self {
            desktop_actions: false,
            max_entries: 5,
            terminal: None,
            history_size: 50,
        }
    }
}

pub struct State {
    config: Config,    
    entries: Vec<(DesktopEntry, u64)>,
    history: history::History, 
}

mod scrubber;
mod history;

const SENSIBLE_TERMINALS: &[&str] = &["alacritty", "foot", "kitty", "wezterm", "wterm"];

#[handler]
pub fn handler(selection: Match, state: &mut State) -> HandleResult {
    let entry = state
        .entries
        .iter()
        .find_map(|(entry, id)| {
            if *id == selection.id.unwrap() {
                Some(entry)
            } else {
                None
            }
        })
        .unwrap();

    if entry.term {
        match &state.config.terminal {
            Some(term) => {
                if let Err(why) = Command::new(term).arg("-e").arg(&entry.exec).spawn() {
                    eprintln!("Error running desktop entry: {}", why);
                }
            }
            None => {
                for term in SENSIBLE_TERMINALS {
                    if Command::new(term)
                        .arg("-e")
                        .arg(&entry.exec)
                        .spawn()
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }
    } else if let Err(why) = Command::new("sh")
        .arg("-c")
        .arg(&entry.exec)
        .current_dir(entry.path.as_ref().unwrap_or(&env::current_dir().unwrap()))
        .spawn()
    {
        eprintln!("Error running desktop entry: {}", why);
    }

    state.history.add_entry(entry.clone());
    state.history.truncate(state.config.history_size);
    state.history.write();


    HandleResult::Close
}

#[init]
pub fn init(config_dir: RString) -> State {
    let config: Config = match fs::read_to_string(format!("{}/applications.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_else(|why| {
            eprintln!("Error parsing applications plugin config: {}", why);
            Config::default()
        }),
        Err(why) => {
            eprintln!("Error reading applications plugin config: {}", why);
            Config::default()
        }
    };

    let entries = scrubber::scrubber(&config).unwrap_or_else(|why| {
        eprintln!("Failed to load desktop entries: {}", why);
        Vec::new()
    });

    let history = history::History::load();
    println!("Loaded {} history entries", history.count());

    State { config, entries, history }
}

#[get_matches]
pub fn get_matches(input: RString, state: &State) -> RVec<Match> {

    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();
    let mut entries = state
        .entries
        .iter()
        .filter_map(|(entry, id)| {
            let app_score = match &entry.desc {
                None => matcher.fuzzy_match(&entry.name, &input).unwrap_or(0),
                Some(val) => matcher
                    .fuzzy_match(&format!("{} {}", &val, &entry.name).to_string(), &input)
                    .unwrap_or(0),
            };

            let keyword_score = entry
                .keywords
                .iter()
                .map(|keyword| matcher.fuzzy_match(keyword, &input).unwrap_or(0))
                .sum::<i64>();   

            let history_score = state.history.get_entry_info(entry).map(|(index, count)| {                
                let recency_bias = i64::max(5-index as i64, 0);
                (count as i64 + recency_bias) * 20
            }).unwrap_or(0);

            if app_score + keyword_score == 0 {
                return None;
            }

            let mut score = (app_score * 25 + keyword_score + history_score) - entry.offset;

            // prioritize actions
            if entry.desc.is_some() {
                score = score * 2;
            }

            if score > 0 {
                Some((entry, *id, score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| b.2.cmp(&a.2));

    entries.truncate(state.config.max_entries);    
    entries
    .into_iter()
    .map(|(entry, id, _)| Match {
        title: entry.name.clone().into(),
        description: entry.desc.clone().map(|desc| desc.into()).into(),
        use_pango: false,
        icon: ROption::RSome(entry.icon.clone().into()),
        id: ROption::RSome(id),
    })
    .collect()
}

#[info]
pub fn info() -> PluginInfo {
    PluginInfo {
        name: "Applications".into(),
        icon: "application-x-executable".into(),
    }
}
