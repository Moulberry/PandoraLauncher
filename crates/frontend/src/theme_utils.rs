use crate::interface_config::{self, InterfaceConfig};
use gpui::*;

pub const SYSTEM_DEFAULT_THEME: &str = "System Default";

pub fn update_theme(cx: &mut App) {
    let config = InterfaceConfig::get(cx);
    let theme_name = if config.active_theme == SYSTEM_DEFAULT_THEME {
        match cx.window_appearance() {
            WindowAppearance::Light => "Default Light".into(),
            _ => "Default Dark".into(),
        }
    } else {
        config.active_theme.clone()
    };

    if theme_name.is_empty() {
        return;
    }

    if let Some(theme) = gpui_component::ThemeRegistry::global(cx).themes().get(&SharedString::new(theme_name.trim_ascii())).cloned() {
        gpui_component::Theme::global_mut(cx).apply_config(&theme);
        cx.refresh_windows();
    }
}
