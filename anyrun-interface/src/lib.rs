use abi_stable::{
    declare_root_module_statics,
    library::RootModule,
    package_version_strings,
    sabi_types::VersionStrings,
    std_types::{ROption, RString, RVec},
    StableAbi,
};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginRef)))]
#[sabi(missing_field(panic))]
pub struct Plugin {
    pub init: extern "C" fn(RString),
    pub info: extern "C" fn() -> PluginInfo,
    pub get_matches: extern "C" fn(RString) -> u64,
    pub poll_matches: extern "C" fn(u64) -> PollResult,
    pub handle_selection: extern "C" fn(Match) -> HandleResult,
}

/// Info of the plugin. Used for the main UI
#[repr(C)]
#[derive(StableAbi, Debug)]
pub struct PluginInfo {
    pub name: RString,
    /// The icon name from the icon theme in use
    pub icon: RString,
}

/// Represents a match from a plugin
///
/// The `title` and `description` support pango markup when `use_pango` is set to true.
/// Refer to [Pango Markup](https://docs.gtk.org/Pango/pango_markup.html) for how to use pango markup.
#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct Match {
    pub title: RString,
    pub description: ROption<RString>,
    /// Whether the title and description should be interpreted as pango markup.
    pub use_pango: bool,
    /// The icon name from the icon theme in use
    pub icon: ROption<RString>,
    /// The path to a custom image to use instead of an icon
    pub image: ROption<RString>,
    /// For runners to differentiate between the matches. Not required.
    pub id: ROption<u64>,
}

/// For determining how anyrun should proceed after the plugin has handled a match selection
#[repr(C)]
#[derive(StableAbi)]
pub enum HandleResult {
    /// Shut down the program
    Close,
    /// Refresh the items. Useful if the runner wants to alter results in place.
    /// The inner value can set an exclusive mode for the plugin.
    Refresh(bool),
    /// Copy the content, due to how copying works it must be done like this.
    Copy(RVec<u8>),
    /// Output the content to stdout, printing to stdout has issues in plugins.
    Stdout(RVec<u8>),
}

#[repr(C)]
#[derive(StableAbi)]
pub enum PollResult {
    Ready(RVec<Match>),
    Pending,
    Cancelled,
}

impl RootModule for PluginRef {
    declare_root_module_statics! {PluginRef}

    const BASE_NAME: &'static str = "anyrun_plugin";
    const NAME: &'static str = "anyrun_plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}
