use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::{get_matches, handler, HandleResult, info, init, Match, PluginInfo};

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Input".into(),
        icon: "input-keyboard".into(),
    }
}

#[init]
fn init(_config_dir: RString) {}

#[get_matches]
fn get_matches(input: RString) -> RVec<Match> {
    vec![Match {
        title: input,
        description: ROption::RNone,
        use_pango: false,
        icon: ROption::RNone,
        id: ROption::RNone,
    }]
    .into()
}

#[handler]
fn handler(selection: Match) -> HandleResult {
    HandleResult::Stdout(selection.title.into_bytes())
}
