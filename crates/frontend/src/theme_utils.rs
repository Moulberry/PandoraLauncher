use crate::interface_config::{self, InterfaceConfig};
use gpui::*;

pub fn update_theme(cx: &mut App) {
    let config = InterfaceConfig::get(cx);
    let theme_mode = config.theme_mode;
    let light_theme: SharedString = "Default Light".into();
    let dark_theme: SharedString = "Default Dark".into();

    let theme_name = match theme_mode {
        interface_config::ThemeMode::System => {
            match cx.window_appearance() {
                WindowAppearance::Light => light_theme,
                _ => dark_theme,
            }
        },
        interface_config::ThemeMode::Light => light_theme,
        interface_config::ThemeMode::Dark => dark_theme,
    };

    if theme_name.is_empty() {
        return;
    }

    if let Some(theme) = gpui_component::ThemeRegistry::global(cx).themes().get(&SharedString::new(theme_name.trim_ascii())).cloned() {
        gpui_component::Theme::global_mut(cx).apply_config(&theme);
        cx.refresh_windows();
    }
}
