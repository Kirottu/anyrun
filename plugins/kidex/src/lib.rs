use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::{anyrun_interface::HandleResult, *};
use fuzzy_matcher::FuzzyMatcher;
use kidex_common::IndexEntry;
use serde::Deserialize;
use std::{fs, os::unix::prelude::OsStrExt, process::Command};

#[derive(Deserialize)]
struct Config {
    max_entries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { max_entries: 3 }
    }
}

pub struct State {
    config: Config,
    index: Vec<(usize, IndexEntry)>,
    selection: Option<IndexEntry>,
}

enum IndexAction {
    Open,
    CopyPath,
    Back,
}

impl From<u64> for IndexAction {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Open,
            1 => Self::CopyPath,
            2 => Self::Back,
            _ => unreachable!(),
        }
    }
}

#[handler]
pub fn handler(selection: Match, state: &mut State) -> HandleResult {
    match &state.selection {
        Some(index_entry) => match selection.id.unwrap().into() {
            IndexAction::Open => {
                if let Err(why) = Command::new("xdg-open").arg(&index_entry.path).spawn() {
                    println!("Error running xdg-open: {}", why);
                }
                HandleResult::Close
            }
            IndexAction::CopyPath => {
                HandleResult::Copy(index_entry.path.clone().into_os_string().as_bytes().into())
            }
            IndexAction::Back => {
                state.selection = None;
                HandleResult::Refresh(false)
            }
        },
        None => {
            let (_, index_entry) = state
                .index
                .iter()
                .find(|(id, _)| selection.id == ROption::RSome(*id as u64))
                .unwrap();

            state.selection = Some(index_entry.clone());
            HandleResult::Refresh(true)
        }
    }
}

#[init]
pub fn init(config_dir: RString) -> State {
    let config = match fs::read_to_string(format!("{}/kidex.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    };
    let index = match kidex_common::util::get_index(None) {
        Ok(index) => index.into_iter().enumerate().collect(),
        Err(why) => {
            println!("Failed to get kidex index: {}", why);
            Vec::new()
        }
    };
    State {
        config,
        index,
        selection: None,
    }
}

#[get_matches]
pub fn get_matches(input: RString, state: &State) -> RVec<Match> {
    match &state.selection {
        Some(index_entry) => {
            let path = index_entry.path.to_string_lossy();
            vec![
                Match {
                    title: "Open File".into(),
                    description: ROption::RSome(path.clone().into()),
                    use_pango: false,
                    id: ROption::RSome(IndexAction::Open as u64),
                    icon: ROption::RSome("document-open".into()),
                    image: ROption::RNone,
                },
                Match {
                    title: "Copy Path".into(),
                    description: ROption::RSome(path.into()),
                    use_pango: false,
                    id: ROption::RSome(IndexAction::CopyPath as u64),
                    icon: ROption::RSome("edit-copy".into()),
                    image: ROption::RNone,
                },
                Match {
                    title: "Back".into(),
                    description: ROption::RNone,
                    use_pango: false,
                    id: ROption::RSome(IndexAction::Back as u64),
                    icon: ROption::RSome("edit-undo".into()),
                    image: ROption::RNone,
                },
            ]
            .into()
        }
        None => {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();
            let mut index = state
                .index
                .clone()
                .into_iter()
                .filter_map(|(id, index_entry)| {
                    matcher
                        .fuzzy_match(&index_entry.path.as_os_str().to_string_lossy(), &input)
                        .map(|val| (index_entry, id, val))
                })
                .collect::<Vec<_>>();

            index.sort_by(|a, b| b.2.cmp(&a.2));

            index.truncate(state.config.max_entries);
            index
                .into_iter()
                .map(|(entry_index, id, _)| Match {
                    title: entry_index
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into())
                        .unwrap_or("N/A".into()),
                    description: entry_index
                        .path
                        .parent()
                        .map(|path| path.display().to_string().into())
                        .into(),
                    use_pango: false,
                    icon: ROption::RSome(if entry_index.directory {
                        "folder".into()
                    } else {
                        "text-x-generic".into()
                    }),
                    image: ROption::RNone,
                    id: ROption::RSome(id as u64),
                })
                .collect()
        }
    }
}

#[info]
pub fn info() -> PluginInfo {
    PluginInfo {
        name: "Kidex".into(),
        icon: "folder".into(),
    }
}
