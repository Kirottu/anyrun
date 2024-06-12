use std::{env, fs, process::Command};

use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    prefix: String,
    shell: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            prefix: ":sh".to_string(),
            shell: None,
        }
    }
}

#[init]
fn init(config_dir: RString) -> Config {
    match fs::read_to_string(format!("{}/shell.ron", config_dir)) {
        Ok(content) => ron::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Shell".into(),
        icon: "utilities-terminal".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, config: &Config) -> RVec<Match> {
    if input.starts_with(&config.prefix) {
        let (_, command) = input.split_once(&config.prefix).unwrap();
        if !command.is_empty() {
            vec![Match {
                title: command.trim().into(),
                description: ROption::RSome(
                    config
                        .shell
                        .clone()
                        .unwrap_or_else(|| {
                            env::var("SHELL").unwrap_or_else(|_| {
                                "The shell could not be determined!".to_string()
                            })
                        })
                        .into(),
                ),
                use_pango: false,
                icon: ROption::RNone,
                image: ROption::RNone,
                id: ROption::RNone,
            }]
            .into()
        } else {
            RVec::new()
        }
    } else {
        RVec::new()
    }
}

#[handler]
fn handler(selection: Match) -> HandleResult {
    if let Err(why) = Command::new(selection.description.unwrap().as_str())
        .arg("-c")
        .arg(selection.title.as_str())
        .spawn()
    {
        println!("Failed to run command: {}", why);
    }

    HandleResult::Close
}
