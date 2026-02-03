#![deny(unused_must_use)]

use std::{
    path::{Path, PathBuf}, sync::{Arc, atomic::AtomicBool}
};

use bridge::
    handle::{BackendHandle, FrontendReceiver}
;
use gpui::*;
use gpui_component::{
    notification::{Notification, NotificationType}, Root, StyledExt, WindowExt
};
use indexmap::IndexMap;
use parking_lot::RwLock;

use crate::{
    entity::{
        DataEntities, PanicMessages, account::AccountEntries, instance::InstanceEntries, metadata::FrontendMetadata
    }, interface_config::InterfaceConfig, processor::Processor, root::{LauncherRoot, LauncherRootGlobal}, theme_utils::update_theme
};

pub mod component;
pub mod entity;
pub mod game_output;
pub mod modals;
pub mod pages;
pub mod interface_config;
pub mod png_render_cache;
pub mod processor;
pub mod root;
pub mod ui;
pub mod theme_utils;

rust_i18n::i18n!("locales");

macro_rules! ts {
    ($($all:tt)*) => {
        SharedString::new_static(ustr::ustr(&*rust_i18n::t!($($all)*)).as_str())
    };
}
pub(crate) use ts;

macro_rules! ts_short {
    ($id:expr) => {{
        let short_key = format!("{}-short", $id);
        let translated = rust_i18n::t!(&short_key);
        if translated.ends_with("-short") {
            ts!($id)
        } else {
            SharedString::new_static(ustr::ustr(&*translated).as_str())
        }
    }};
}
pub(crate) use ts_short;

#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/**/*.svg"]
#[include = "images/**/*.png"]
#[include = "fonts/**/*.ttf"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow::anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter().filter_map(|p| p.starts_with(path).then(|| p.into())).collect())
    }
}

#[cfg(windows)]
pub const MAIN_FONT: &'static str = "Inter 24pt 24pt";
#[cfg(not(windows))]
pub const MAIN_FONT: &'static str = "Inter 24pt";

actions!([Quit, CloseWindow]);

pub fn start(
    launcher_dir: PathBuf,
    panic_message: Arc<RwLock<Option<String>>>,
    deadlock_message: Arc<RwLock<Option<String>>>,
    backend_handle: BackendHandle,
    mut recv: FrontendReceiver,
) {
    let user_agent = if let Some(version) = option_env!("PANDORA_RELEASE_VERSION") {
        format!("PandoraLauncher/{version} (https://github.com/Moulberry/PandoraLauncher)")
    } else {
        "PandoraLauncher/dev (https://github.com/Moulberry/PandoraLauncher)".to_string()
    };

    let http_client = Arc::new(reqwest_client::ReqwestClient::user_agent(&user_agent).unwrap());

    Application::new().with_http_client(http_client).with_assets(Assets).run(move |cx: &mut App| {
        let _ = cx.text_system().add_fonts(vec![
            Assets.load("fonts/inter/Inter-Regular.ttf").unwrap().unwrap(),
            Assets.load("fonts/roboto-mono/RobotoMono-Regular.ttf").unwrap().unwrap(),
        ]);

        gpui_component::init(cx);
        InterfaceConfig::init(cx, launcher_dir.join("interface.json").into());

        gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);

        let theme_folder = launcher_dir.join("themes");

        _ = gpui_component::ThemeRegistry::watch_dir(theme_folder.clone(), cx, move |cx| {
            update_theme(cx);
        });



        update_theme(cx);

        let theme = gpui_component::Theme::global_mut(cx);
        theme.font_family = SharedString::new_static(MAIN_FONT);
        theme.scrollbar_show = gpui_component::scroll::ScrollbarShow::Always;

        cx.on_app_quit(|cx| {
            InterfaceConfig::force_save(cx);
            async {}
        }).detach();

        let main_window_hidden = Arc::new(AtomicBool::new(false));

        cx.on_window_closed({
            let main_window_hidden = main_window_hidden.clone();
            move |cx| {
                if cx.windows().is_empty() && !main_window_hidden.load(std::sync::atomic::Ordering::SeqCst) {
                    cx.quit();
                }
            }
        }).detach();

        cx.bind_keys([
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-w", CloseWindow, None),
        ]);

        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        let instances = cx.new(|_| InstanceEntries {
            entries: IndexMap::new(),
        });
        let metadata = cx.new(|_| FrontendMetadata::new(backend_handle.clone()));
        let accounts = cx.new(|_| AccountEntries::default());
        let data = DataEntities {
            instances,
            metadata,
            backend_handle,
            accounts,
            theme_folder: theme_folder.into(),
            panic_messages: Arc::new(PanicMessages {
                panic_message,
                deadlock_message,
            })
        };

        let mut processor = Processor::new(data.clone(), main_window_hidden);

        while let Some(message) = recv.try_recv() {
            processor.process(message, cx);
        }

        let main_window = open_main_window(&data, cx);
        processor.set_main_window_handle(main_window, cx);

        cx.spawn(async move |cx| {
            while let Some(message) = recv.recv().await {
                _ = cx.update(|cx| {
                    processor.process(message, cx);
                });
            }
        }).detach();
    });
}

pub fn open_main_window(data: &DataEntities, cx: &mut App) -> AnyWindowHandle {
    let window_bounds = match InterfaceConfig::get(cx).main_window_bounds {
        interface_config::WindowBounds::Inherit => None,
        interface_config::WindowBounds::Windowed { x, y, w, h } => {
            Some(WindowBounds::Windowed(Bounds::new(Point::new(px(x), px(y)), Size::new(px(w), px(h)))))
        },
        interface_config::WindowBounds::Maximized { x, y, w, h } => {
            Some(WindowBounds::Maximized(Bounds::new(Point::new(px(x), px(y)), Size::new(px(w), px(h)))))
        },
        interface_config::WindowBounds::Fullscreen { x, y, w, h } => {
            Some(WindowBounds::Fullscreen(Bounds::new(Point::new(px(x), px(y)), Size::new(px(w), px(h)))))
        },
    };

    let handle = cx.open_window(
        WindowOptions {
            app_id: Some("PandoraLauncher".into()),
            window_min_size: Some(size(px(360.0), px(240.0))),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::new_static("Pandora")),
                ..Default::default()
            }),
            window_bounds,
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title("Pandora");

            let launcher_root = cx.new(|cx| {
                cx.observe_window_bounds(window, move |_, window, cx| {
                    let origin = window.bounds().origin;
                    let size = window.viewport_size();
                    let new_bounds = (
                        origin.x.to_f64() as f32, origin.y.to_f64() as f32,
                        size.width.to_f64() as f32, size.height.to_f64() as f32
                    );

                    let old_window_bounds = InterfaceConfig::get(cx).main_window_bounds.clone();
                    let old_bounds = match old_window_bounds {
                        interface_config::WindowBounds::Inherit => new_bounds,
                        interface_config::WindowBounds::Windowed { x, y, w, h } => (x, y, w, h),
                        interface_config::WindowBounds::Maximized { x, y, w, h } => (x, y, w, h),
                        interface_config::WindowBounds::Fullscreen { x, y, w, h } => (x, y, w, h),
                    };

                    let new_window_bounds = if window.is_fullscreen() {
                        interface_config::WindowBounds::Fullscreen {
                            x: old_bounds.0,
                            y: old_bounds.1,
                            w: old_bounds.2,
                            h: old_bounds.3
                        }
                    } else if window.is_maximized() {
                        interface_config::WindowBounds::Maximized {
                            x: old_bounds.0,
                            y: old_bounds.1,
                            w: old_bounds.2,
                            h: old_bounds.3
                        }
                    } else {
                        interface_config::WindowBounds::Windowed {
                            x: new_bounds.0,
                            y: new_bounds.1,
                            w: new_bounds.2,
                            h: new_bounds.3
                        }
                    };

                    if new_window_bounds != old_window_bounds {
                        InterfaceConfig::get_mut(cx).main_window_bounds = new_window_bounds;
                    }
                }).detach();

                LauncherRoot::new(&data, window, cx)
            });

            cx.set_global(LauncherRootGlobal {
                root: launcher_root.clone(),
            });
            cx.new(|cx| Root::new(launcher_root, window, cx))
        },
    ).unwrap();

    cx.activate(true);

    handle.into()
}

pub(crate) fn is_valid_instance_name(name: &str) -> bool {
    is_single_component_path(name) &&
    sanitize_filename::is_sanitized_with_options(name, sanitize_filename::OptionsForCheck { windows: true, ..Default::default() })
}

pub(crate) fn is_single_component_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek()
        && !matches!(first, std::path::Component::Normal(_))
    {
        return false;
    }

    components.count() == 1
}

#[inline]
pub(crate) fn labelled(label: &'static str, element: impl IntoElement) -> Div {
    gpui_component::v_flex().gap_0p5().child(div().text_sm().font_medium().child(label)).child(element)
}

pub(crate) fn open_folder(path: &Path, window: &mut Window, cx: &mut App) {
    if path.is_dir() {
        if let Err(err) = open::that_detached(path) {
            let notification: Notification = (NotificationType::Error, SharedString::from(format!("Unable to open folder: {err}"))).into();
            window.push_notification(notification.autohide(false), cx);
        }
    } else {
        let notification: Notification = (NotificationType::Error, SharedString::from("Unable to open folder: not a directory")).into();
        window.push_notification(notification.autohide(false), cx);
    }
}
