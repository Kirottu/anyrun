use std::{
    cell::RefCell,
    env, fs,
    io::{self, Write},
    mem,
    path::PathBuf,
    rc::Rc,
    sync::Once,
    time::Duration,
};

use abi_stable::std_types::{ROption, RVec};
use anyrun_interface::{HandleResult, Match, PluginInfo, PluginRef, PollResult};
use clap::{Parser, ValueEnum};
use gtk::{gdk, gdk_pixbuf, gio, glib, prelude::*};
use nix::unistd;
use serde::Deserialize;
use wl_clipboard_rs::copy;

#[anyrun_macros::config_args]
#[derive(Deserialize)]
struct Config {
    #[serde(default = "Config::default_x")]
    x: RelativeNum,

    #[serde(default = "Config::default_y")]
    y: RelativeNum,

    #[serde(default = "Config::default_width")]
    width: RelativeNum,

    #[serde(default = "Config::default_height")]
    height: RelativeNum,

    #[serde(default = "Config::default_plugins")]
    plugins: Vec<PathBuf>,

    #[serde(default)]
    hide_icons: bool,
    #[serde(default)]
    hide_plugin_info: bool,
    #[serde(default)]
    ignore_exclusive_zones: bool,
    #[serde(default)]
    close_on_click: bool,
    #[serde(default)]
    show_results_immediately: bool,
    #[serde(default)]
    max_entries: Option<usize>,
    #[serde(default = "Config::default_layer")]
    layer: Layer,

    #[serde(default = "Config::default_binding_close")]
    binding_close: Vec<String>,
    #[serde(default = "Config::default_binding_up")]
    binding_up: Vec<String>,
    #[serde(default = "Config::default_binding_down")]
    binding_down: Vec<String>,
    #[serde(default = "Config::default_binding_select")]
    binding_select: Vec<String>,
}
impl Config {
    fn default_x() -> RelativeNum {
        RelativeNum::Fraction(0.5)
    }

    fn default_y() -> RelativeNum {
        RelativeNum::Absolute(0)
    }

    fn default_width() -> RelativeNum {
        RelativeNum::Fraction(0.5)
    }

    fn default_height() -> RelativeNum {
        RelativeNum::Absolute(0)
    }

    fn default_plugins() -> Vec<PathBuf> {
        vec![
            "libapplications.so".into(),
            "libsymbols.so".into(),
            "libshell.so".into(),
            "libtranslate.so".into(),
        ]
    }

    fn default_layer() -> Layer {
        Layer::Overlay
    }

    fn default_binding_close() -> Vec<String> {
        vec!["Escape".into()]
    }
    fn default_binding_up() -> Vec<String> {
        vec!["Up".into(), "Shift Tab".into()]
    }
    fn default_binding_down() -> Vec<String> {
        vec!["Down".into(), "Tab".into()]
    }
    fn default_binding_select() -> Vec<String> {
        vec!["Return".into()]
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            x: Self::default_x(),
            y: Self::default_y(),
            width: Self::default_width(),
            height: Self::default_height(),
            plugins: Self::default_plugins(),
            hide_icons: false,
            hide_plugin_info: false,
            ignore_exclusive_zones: false,
            close_on_click: false,
            show_results_immediately: false,
            max_entries: None,
            layer: Self::default_layer(),
            binding_close: Self::default_binding_close(),
            binding_up: Self::default_binding_up(),
            binding_down: Self::default_binding_down(),
            binding_select: Self::default_binding_select(),
        }
    }
}

#[derive(Deserialize, Clone, ValueEnum)]
enum Layer {
    Background,
    Bottom,
    Top,
    Overlay,
}

// Could have a better name
#[derive(Deserialize, Clone)]
enum RelativeNum {
    Absolute(i32),
    Fraction(f32),
}

impl RelativeNum {
    fn to_val(&self, val: u32) -> i32 {
        match self {
            RelativeNum::Absolute(num) => *num,
            RelativeNum::Fraction(frac) => (frac * val as f32) as i32,
        }
    }
}

impl From<&str> for RelativeNum {
    fn from(value: &str) -> Self {
        let (ty, val) = value.split_once(':').expect("Invalid RelativeNum value");

        match ty {
            "absolute" => Self::Absolute(val.parse().unwrap()),
            "fraction" => Self::Fraction(val.parse().unwrap()),
            _ => panic!("Invalid type of value"),
        }
    }
}

/// A "view" of plugin's info and matches
#[derive(Clone)]
struct PluginView {
    plugin: PluginRef,
    row: gtk::ListBoxRow,
    list: gtk::ListBox,
}

#[derive(Parser)]
struct Args {
    /// Override the path to the config directory
    #[arg(short, long)]
    config_dir: Option<String>,
    #[command(flatten)]
    config: ConfigArgs,
}

#[derive(Deserialize, Clone, ValueEnum)]
enum Position {
    Top,
    Center,
}

/// Actions to run after GTK has finished
enum PostRunAction {
    Copy(Vec<u8>),
    None,
}

/// Some data that needs to be shared between various parts
struct RuntimeData {
    /// A plugin may request exclusivity which is set with this
    exclusive: Option<PluginView>,
    plugins: Vec<PluginView>,
    post_run_action: PostRunAction,
    config: Config,
    /// Used for displaying errors later on
    error_label: String,
    config_dir: String,
}

/// The naming scheme for CSS styling
///
/// Refer to [GTK 3.0 CSS Overview](https://docs.gtk.org/gtk3/css-overview.html)
/// and [GTK 3.0 CSS Properties](https://docs.gtk.org/gtk3/css-properties.html) for how to style.
mod style_names {
    /// The text entry box
    pub const ENTRY: &str = "entry";
    /// "Main" widgets (main GtkListBox, main GtkBox)
    pub const MAIN: &str = "main";
    /// The window
    pub const WINDOW: &str = "window";
    /// Widgets related to the whole plugin. Including the info box
    pub const PLUGIN: &str = "plugin";
    /// Widgets for the specific match `MATCH_*` names are for more specific parts.
    pub const MATCH: &str = "match";

    pub const MATCH_TITLE: &str = "match-title";
    pub const MATCH_DESC: &str = "match-desc";
}

/// Default config directory
pub const DEFAULT_CONFIG_DIR: &str = "/etc/anyrun";

fn main() {
    let app = gtk::Application::new(Some("com.kirottu.anyrun"), Default::default());

    // Register here so we know if the instance is the primary or a remote
    app.register(None::<&gio::Cancellable>).unwrap();

    // If another instance is running, quit
    if app.is_remote() {
        return;
    }

    let args = Args::parse();

    // Figure out the config dir
    let user_dir = format!(
        "{}/.config/anyrun",
        env::var("HOME").expect("Could not determine home directory! Is $HOME set?")
    );
    let config_dir = args.config_dir.unwrap_or_else(|| {
        if PathBuf::from(&user_dir).exists() {
            user_dir
        } else {
            DEFAULT_CONFIG_DIR.to_string()
        }
    });

    // Load config, if unable to then read default config. If an error occurs the message will be displayed.
    let (mut config, error_label) = match fs::read_to_string(format!("{}/config.ron", config_dir)) {
        Ok(content) => ron::from_str(&content)
            .map(|config| (config, String::new()))
            .unwrap_or_else(|why| {
                (
                    Config::default(),
                    format!(
                        "Failed to parse Anyrun config file, using default config: {}",
                        why
                    ),
                )
            }),
        Err(why) => (
            Config::default(),
            format!(
                "Failed to read Anyrun config file, using default config: {}",
                why
            ),
        ),
    };

    config.merge_opt(args.config);

    let runtime_data: Rc<RefCell<RuntimeData>> = Rc::new(RefCell::new(RuntimeData {
        exclusive: None,
        plugins: Vec::new(),
        post_run_action: PostRunAction::None,
        config,
        error_label,
        config_dir,
    }));

    let runtime_data_clone = runtime_data.clone();
    app.connect_activate(move |app| activate(app, runtime_data_clone.clone()));

    // Run with no args to make sure only clap is used
    app.run_with_args::<String>(&[]);

    let runtime_data = runtime_data.borrow_mut();

    // Perform a post run action if one is set
    match &runtime_data.post_run_action {
        PostRunAction::Copy(bytes) => match unsafe { unistd::fork() } {
            // The parent process just exits and prints that out
            Ok(unistd::ForkResult::Parent { .. }) => {
                println!("Child spawned to serve copy requests.");
            }
            // Child process starts serving copy requests
            Ok(unistd::ForkResult::Child) => {
                let mut opts = copy::Options::new();
                opts.foreground(true);
                opts.copy(
                    copy::Source::Bytes(bytes.clone().into_boxed_slice()),
                    copy::MimeType::Autodetect,
                )
                .expect("Failed to serve copy bytes");
            }
            Err(why) => {
                eprintln!("Failed to fork for copy sharing: {}", why);
            }
        },
        PostRunAction::None => (),
    }
}

fn activate(app: &gtk::Application, runtime_data: Rc<RefCell<RuntimeData>>) {
    // Create the main window
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .name(style_names::WINDOW)
        .build();

    // Init GTK layer shell
    gtk_layer_shell::init_for_window(&window);

    // Make layer-window fullscreen
    gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Top, true);
    gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Bottom, true);
    gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Left, true);
    gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Right, true);

    gtk_layer_shell::set_namespace(&window, "anyrun");

    if runtime_data.borrow().config.ignore_exclusive_zones {
        gtk_layer_shell::set_exclusive_zone(&window, -1);
    }

    gtk_layer_shell::set_keyboard_mode(&window, gtk_layer_shell::KeyboardMode::Exclusive);

    match runtime_data.borrow().config.layer {
        Layer::Background => {
            gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Background)
        }
        Layer::Bottom => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Bottom),
        Layer::Top => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Top),
        Layer::Overlay => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay),
    };

    // Try to load custom CSS, if it fails load the default CSS
    let provider = gtk::CssProvider::new();
    if let Err(why) =
        provider.load_from_path(&format!("{}/style.css", runtime_data.borrow().config_dir))
    {
        eprintln!("Failed to load custom CSS: {}", why);
        provider
            .load_from_data(include_bytes!("../res/style.css"))
            .unwrap();
    }
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::default().expect("Failed to get GDK screen for CSS provider!"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Create the main list of plugin views
    let main_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .name(style_names::MAIN)
        .build();

    // Prioritise the ANYRUN_PLUGINS env var over other paths
    let mut plugin_paths = match env::var("ANYRUN_PLUGINS") {
        Ok(string) => string.split(':').map(PathBuf::from).collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    plugin_paths.append(&mut vec![
        format!("{}/plugins", runtime_data.borrow().config_dir).into(),
        format!("{}/plugins", DEFAULT_CONFIG_DIR).into(),
    ]);

    // Load plugins from the paths specified in the config file
    let plugins = runtime_data
        .borrow()
        .config
        .plugins
        .iter()
        .map(|plugin_path| {
            // Load the plugin's dynamic library.
            let mut user_path =
                PathBuf::from(&format!("{}/plugins", runtime_data.borrow().config_dir));
            let mut global_path = PathBuf::from("/etc/anyrun/plugins");
            user_path.extend(plugin_path.iter());
            global_path.extend(plugin_path.iter());

            // Load the plugin's dynamic library.

            let plugin = if plugin_path.is_absolute() {
                abi_stable::library::lib_header_from_path(plugin_path)
            } else {
                let path = plugin_paths
                    .clone()
                    .into_iter()
                    .map(|mut path| {
                        path.push(plugin_path);
                        path
                    })
                    .find(|path| path.exists())
                    .expect("Invalid plugin path");

                abi_stable::library::lib_header_from_path(&path)
            }
            .and_then(|plugin| plugin.init_root_module::<PluginRef>())
            .expect("Failed to load plugin");

            // Run the plugin's init code to init static resources etc.
            plugin.init()(runtime_data.borrow().config_dir.clone().into());

            let plugin_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .name(style_names::PLUGIN)
                .build();
            if !runtime_data.borrow().config.hide_plugin_info {
                plugin_box.add(&create_info_box(
                    &plugin.info()(),
                    runtime_data.borrow().config.hide_icons,
                ));
                plugin_box.add(
                    &gtk::Separator::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .name(style_names::PLUGIN)
                        .build(),
                );
            }
            let list = gtk::ListBox::builder()
                .name(style_names::PLUGIN)
                .hexpand(true)
                .build();

            plugin_box.add(&list);

            let row = gtk::ListBoxRow::builder().name(style_names::PLUGIN).build();
            row.add(&plugin_box);

            main_list.add(&row);

            PluginView { plugin, row, list }
        })
        .collect::<Vec<PluginView>>();

    // Assign the plugins here to avoid multiple mutable/immutable borrows
    runtime_data.borrow_mut().plugins = plugins;

    // Connect selection events to avoid completely messing up selection logic
    for plugin_view in runtime_data.borrow().plugins.iter() {
        let plugins_clone = runtime_data.borrow().plugins.clone();
        plugin_view.list.connect_row_selected(move |list, row| {
            if row.is_some() {
                let combined_matches = plugins_clone
                    .iter()
                    .flat_map(|view| {
                        view.list.children().into_iter().map(|child| {
                            (
                                child.dynamic_cast::<gtk::ListBoxRow>().unwrap(),
                                view.list.clone(),
                            )
                        })
                    })
                    .collect::<Vec<(gtk::ListBoxRow, gtk::ListBox)>>();

                // Unselect everything except the new selection
                for (_, _list) in combined_matches {
                    if _list != *list {
                        _list.select_row(None::<&gtk::ListBoxRow>);
                    }
                }
            }
        });
    }

    // Text entry box
    let entry = gtk::Entry::builder()
        .hexpand(true)
        .name(style_names::ENTRY)
        .build();

    // Refresh the matches when text input changes
    let runtime_data_clone = runtime_data.clone();
    entry.connect_changed(move |entry| {
        refresh_matches(entry.text().to_string(), runtime_data_clone.clone())
    });

    // Handle other key presses for selection control and all other things that may be needed
    let binding_close = rearrange_key_vectors(runtime_data.borrow().config.binding_close.clone());
    let binding_up = rearrange_key_vectors(runtime_data.borrow().config.binding_up.clone());
    let binding_down = rearrange_key_vectors(runtime_data.borrow().config.binding_down.clone());
    let binding_select = rearrange_key_vectors(runtime_data.borrow().config.binding_select.clone());

    let entry_clone = entry.clone();
    let runtime_data_clone = runtime_data.clone();

    window.connect_key_press_event(move |window, event| {
        let result_key =
            rearrange_keys(&event.keyval().name().unwrap(), &event.state().to_string());

        // Close window
        if binding_close.contains(&result_key) {
            window.close();
            Inhibit(true)
        }
        // Handle selections
        else if [binding_up.clone(), binding_down.clone()]
            .concat()
            .contains(&result_key)
        {
            // Combine all of the matches into a `Vec` to allow for easier handling of the selection
            let combined_matches = runtime_data_clone
                .borrow()
                .plugins
                .iter()
                .flat_map(|view| {
                    view.list.children().into_iter().map(|child| {
                        (
                            // All children of lists are GtkListBoxRow widgets
                            child.dynamic_cast::<gtk::ListBoxRow>().unwrap(),
                            view.list.clone(),
                        )
                    })
                })
                .collect::<Vec<(gtk::ListBoxRow, gtk::ListBox)>>();

            // Get the selected match
            let (selected_match, selected_list) = match runtime_data_clone
                .borrow()
                .plugins
                .iter()
                .find_map(|view| view.list.selected_row().map(|row| (row, view.list.clone())))
            {
                Some(selected) => selected,
                None => {
                    // If nothing is selected select either the top or bottom match based on the input
                    if !combined_matches.is_empty() {
                        if binding_down.contains(&result_key) {
                            combined_matches[0]
                                .1
                                .select_row(Some(&combined_matches[0].0))
                        } else if binding_up.contains(&result_key) {
                            combined_matches[combined_matches.len() - 1]
                                .1
                                .select_row(Some(&combined_matches[combined_matches.len() - 1].0))
                        }
                    }
                    return Inhibit(true);
                }
            };

            // Clear the previous selection
            selected_list.select_row(None::<&gtk::ListBoxRow>);

            // Get the index of the current selection
            let index = combined_matches
                .iter()
                .position(|(row, _)| *row == selected_match)
                .unwrap();

            // Move the selection based on the input, loops from top to bottom and vice versa
            if binding_down.contains(&result_key) {
                if index < combined_matches.len() - 1 {
                    combined_matches[index + 1]
                        .1
                        .select_row(Some(&combined_matches[index + 1].0));
                } else {
                    combined_matches[0]
                        .1
                        .select_row(Some(&combined_matches[0].0));
                }
            } else if binding_up.contains(&result_key) {
                if index > 0 {
                    combined_matches[index - 1]
                        .1
                        .select_row(Some(&combined_matches[index - 1].0));
                } else {
                    combined_matches[combined_matches.len() - 1]
                        .1
                        .select_row(Some(&combined_matches[combined_matches.len() - 1].0));
                }
            }

            Inhibit(true)
        }
        // Handle when the selected match is "activated"
        else if binding_select.contains(&result_key) {
            let mut _runtime_data_clone = runtime_data_clone.borrow_mut();

            let (selected_match, plugin_view) = match _runtime_data_clone
                .plugins
                .iter()
                .find_map(|view| view.list.selected_row().map(|row| (row, view)))
            {
                Some(selected) => selected,
                None => {
                    return Inhibit(false);
                }
            };

            // Perform actions based on the result of handling the selection
            match plugin_view.plugin.handle_selection()(unsafe {
                (*selected_match.data::<Match>("match").unwrap().as_ptr()).clone()
            }) {
                HandleResult::Close => {
                    window.close();
                    Inhibit(true)
                }
                HandleResult::Refresh(exclusive) => {
                    if exclusive {
                        _runtime_data_clone.exclusive = Some(plugin_view.clone());
                    } else {
                        _runtime_data_clone.exclusive = None;
                    }
                    mem::drop(_runtime_data_clone); // Drop the mutable borrow
                    refresh_matches(entry_clone.text().into(), runtime_data_clone.clone());
                    Inhibit(false)
                }
                HandleResult::Copy(bytes) => {
                    _runtime_data_clone.post_run_action = PostRunAction::Copy(bytes.into());
                    window.close();
                    Inhibit(true)
                }
                HandleResult::Stdout(bytes) => {
                    if let Err(why) = io::stdout().lock().write_all(&bytes) {
                        eprintln!("Error outputting content to stdout: {}", why);
                    }
                    window.close();
                    Inhibit(true)
                }
            }
        } else {
            Inhibit(false)
        }
    });

    // If the option is enabled, close the window when any click is received
    // that is outside the bounds of the main box
    if runtime_data.borrow().config.close_on_click {
        window.connect_button_press_event(move |window, event| {
            if event.window() == window.window() {
                window.close();
                Inhibit(true)
            } else {
                Inhibit(false)
            }
        });
    }

    // Only create the widgets once to avoid issues
    let configure_once = Once::new();

    // Create widgets here for proper positioning
    window.connect_configure_event(move |window, event| {
        let runtime_data = runtime_data.clone();
        let entry = entry.clone();
        let main_list = main_list.clone();

        configure_once.call_once(move || {
            {
                let runtime_data = runtime_data.borrow();

                let width = runtime_data.config.width.to_val(event.size().0);
                let x = runtime_data.config.x.to_val(event.size().0) - width / 2;
                let height = runtime_data.config.height.to_val(event.size().1);
                let y = runtime_data.config.y.to_val(event.size().1) - height / 2;

                // The GtkFixed widget is used for absolute positioning of the main box
                let fixed = gtk::Fixed::builder().build();
                let main_vbox = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .halign(gtk::Align::Center)
                    .vexpand(false)
                    .width_request(width)
                    .height_request(height)
                    .name(style_names::MAIN)
                    .build();
                main_vbox.add(&entry);

                // Display the error message
                if !runtime_data.error_label.is_empty() {
                    main_vbox.add(
                        &gtk::Label::builder()
                            .label(&format!(
                                r#"<span foreground="red">{}</span>"#,
                                runtime_data.error_label
                            ))
                            .use_markup(true)
                            .build(),
                    );
                }

                fixed.put(&main_vbox, x, y);
                window.add(&fixed);
                window.show_all();

                // Add and show the list later, to avoid showing empty plugin categories on launch
                main_vbox.add(&main_list);
                main_list.show();
                entry.grab_focus(); // Grab the focus so typing is immediately accepted by the entry box
            }

            if runtime_data.borrow().config.show_results_immediately {
                // Get initial matches
                refresh_matches(String::new(), runtime_data);
            }
        });

        false
    });

    // Show the window initially, so it gets allocated and configured
    window.show_all();
}

// Rearrange key binding names in ascending order
fn rearrange_keys(key: &str, modifiers: &str) -> String {
    // Fixes to match key names
    let mut modifier_clean = Vec::new();
    let modifiers_copy = modifiers.replace("| ", "");
    for modifier in modifiers_copy.split(" ").collect::<Vec<&str>>() {
        modifier_clean.push(match modifier {
            "Control" => "CONTROL_MASK",
            "Shift" => "SHIFT_MASK",
            "Mod1" => "MOD1_MASK",
            "Mod4" => "MOD4_MASK",
            "Alt" => "MOD1_MASK",
            "Super" => "MOD4_MASK",
            "(empty)" => "",
            _ => modifier,
        });
    }
    let key_clean = match key {
        "Enter" => "Return",
        "ISO_Left_Tab" => "Tab",
        _ => key,
    };
    modifier_clean.sort();
    (modifier_clean.join(" ") + " " + key_clean)
        .trim()
        .to_string()
}

// Rearrange key binding names in vectors in ascending order
fn rearrange_key_vectors(keys: Vec<String>) -> Vec<String> {
    let mut newkeys = Vec::new();
    for binding in keys {
        let (modifiers, keys) = binding.rsplit_once(" ").unwrap_or(("", &binding));
        let newkey = rearrange_keys(keys, modifiers);
        newkeys.push(newkey);
    }
    newkeys
}

fn handle_matches(plugin_view: PluginView, runtime_data: &RuntimeData, matches: RVec<Match>) {
    // Clear out the old matches from the list
    for widget in plugin_view.list.children() {
        plugin_view.list.remove(&widget);
    }

    // If there are no matches, hide the plugin's results
    if matches.is_empty() {
        plugin_view.row.hide();
        return;
    }

    for _match in matches {
        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .name(style_names::MATCH)
            .hexpand(true)
            .build();
        if !runtime_data.config.hide_icons {
            if let ROption::RSome(icon) = &_match.icon {
                let mut builder = gtk::Image::builder()
                    .name(style_names::MATCH)
                    .pixel_size(32);

                let path = PathBuf::from(icon.as_str());

                // If the icon path is absolute, load that file
                if path.is_absolute() {
                    match gdk_pixbuf::Pixbuf::from_file_at_size(icon.as_str(), 32, 32) {
                        Ok(pixbuf) => builder = builder.pixbuf(&pixbuf),
                        Err(why) => {
                            println!("Failed to load icon file: {}", why);
                            builder = builder.icon_name("image-missing"); // Set "broken" icon
                        }
                    }
                } else {
                    builder = builder.icon_name(icon);
                }

                hbox.add(&builder.build());
            }
        }
        let title = gtk::Label::builder()
            .name(style_names::MATCH_TITLE)
            .wrap(true)
            .xalign(0.0)
            .use_markup(_match.use_pango)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .label(&_match.title)
            .build();

        // If a description is present, make a box with it and the title
        match &_match.description {
            ROption::RSome(desc) => {
                let title_desc_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .name(style_names::MATCH)
                    .hexpand(true)
                    .vexpand(true)
                    .build();
                title_desc_box.add(&title);
                title_desc_box.add(
                    &gtk::Label::builder()
                        .name(style_names::MATCH_DESC)
                        .wrap(true)
                        .xalign(0.0)
                        .use_markup(_match.use_pango)
                        .halign(gtk::Align::Start)
                        .valign(gtk::Align::Center)
                        .label(desc)
                        .build(),
                );
                hbox.add(&title_desc_box);
            }
            ROption::RNone => {
                hbox.add(&title);
            }
        }
        let row = gtk::ListBoxRow::builder()
            .name(style_names::MATCH)
            .height_request(32)
            .build();
        row.add(&hbox);
        // GTK data setting is not type checked, so it is unsafe.
        // Only `Match` objects are stored though.
        unsafe {
            row.set_data("match", _match);
        }
        plugin_view.list.add(&row);
    }

    // Refresh the items in the view
    plugin_view.row.show_all();

    let combined_matches = runtime_data
        .plugins
        .iter()
        .flat_map(|view| {
            view.list
                .children()
                .into_iter()
                .map(move |child| (child.dynamic_cast::<gtk::ListBoxRow>().unwrap(), view))
        })
        .collect::<Vec<(gtk::ListBoxRow, &PluginView)>>();

    // If `max_entries` is set, truncate the amount of entries
    if let Some(max_matches) = runtime_data.config.max_entries {
        for (row, view) in combined_matches.iter().skip(max_matches) {
            view.list.remove(row);
        }
    }

    // Hide the plugins that no longer have any entries
    for (_, view) in &combined_matches {
        if view.list.children().is_empty() {
            view.row.hide();
        }
    }

    if let Some((row, view)) = combined_matches.get(0) {
        view.list.select_row(Some(row));
    }
}

/// Create the info box for the plugin
fn create_info_box(info: &PluginInfo, hide_icons: bool) -> gtk::Box {
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .name(style_names::PLUGIN)
        .width_request(200)
        .height_request(32)
        .expand(false)
        .spacing(10)
        .build();
    if !hide_icons {
        info_box.add(
            &gtk::Image::builder()
                .icon_name(&info.icon)
                .name(style_names::PLUGIN)
                .pixel_size(32)
                .halign(gtk::Align::Start)
                .valign(gtk::Align::Start)
                .build(),
        );
    }
    info_box.add(
        &gtk::Label::builder()
            .label(&info.name)
            .name(style_names::PLUGIN)
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build(),
    );
    // This is so that we can align the plugin name with the icon. GTK would not let it be properly aligned otherwise.
    let main_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .name(style_names::PLUGIN)
        .build();
    main_box.add(&info_box);
    main_box.add(
        &gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .name(style_names::PLUGIN)
            .build(),
    );
    main_box
}

/// Refresh the matches from the plugins
fn refresh_matches(input: String, runtime_data: Rc<RefCell<RuntimeData>>) {
    for plugin_view in runtime_data.borrow().plugins.iter() {
        let id = plugin_view.plugin.get_matches()(input.clone().into());
        let plugin_view = plugin_view.clone();
        let runtime_data_clone = runtime_data.clone();
        // If a plugin has requested exclusivity, respect it
        if let Some(exclusive) = &runtime_data.borrow().exclusive {
            if plugin_view.plugin.info() == exclusive.plugin.info() {
                glib::timeout_add_local(Duration::from_micros(1000), move || {
                    async_match(plugin_view.clone(), runtime_data_clone.clone(), id)
                });
            } else {
                handle_matches(plugin_view.clone(), &runtime_data.borrow(), RVec::new());
            }
        } else {
            glib::timeout_add_local(Duration::from_micros(1000), move || {
                async_match(plugin_view.clone(), runtime_data_clone.clone(), id)
            });
        }
    }
}

/// Handle the asynchronously running match task
fn async_match(
    plugin_view: PluginView,
    runtime_data: Rc<RefCell<RuntimeData>>,
    id: u64,
) -> glib::Continue {
    match plugin_view.plugin.poll_matches()(id) {
        PollResult::Ready(matches) => {
            handle_matches(plugin_view, &runtime_data.borrow(), matches);
            glib::Continue(false)
        }
        PollResult::Pending => glib::Continue(true),
        PollResult::Cancelled => glib::Continue(false),
    }
}
