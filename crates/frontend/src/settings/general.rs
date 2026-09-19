use std::rc::Rc;

use gpui::*;
use gpui_component::select::{Select, SelectEvent};

use crate::{component::named_dropdown::{DropdownName, NamedDropdown, NamedDropdownItem, SearchableNamedDropdown}, interface_config::{InterfaceConfig, LiveGameOutputDisplay}, settings::{SettingGroup, SettingItem, SettingItemWidget, SettingPage}};

pub(super) fn create_page(window: &mut Window, cx: &mut App) -> SettingPage {
    SettingPage {
        title: t::settings::general,
        groups: vec![
            SettingGroup {
                title: None,
                items: vec![
                    SettingItem {
                        title: t::settings::general::general::language,
                        description: t::settings::general::general::language_desc,
                        widget: create_language_dropdown(window, cx),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::general::general::quick_delete_mods,
                        description: t::settings::general::general::quick_delete_mods_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.quick_delete_mods, |cfg, val| cfg.quick_delete_mods = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::general::general::quick_delete_instance,
                        description: t::settings::general::general::quick_delete_instance_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.quick_delete_instance, |cfg, val| cfg.quick_delete_instance = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::general::general::live_game_output_display,
                        description: t::settings::general::general::live_game_output_display_desc,
                        widget: create_live_game_output_dropdown(window, cx),
                        ..Default::default()
                    },
                ].into(),
                searched_items: None
            },
            SettingGroup {
                title: Some(t::settings::general::privacy),
                items: vec![
                    SettingItem {
                        title: t::settings::general::privacy::hide_usernames,
                        description: t::settings::general::privacy::hide_usernames_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.hide_usernames, |cfg, val| cfg.hide_usernames = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::general::privacy::hide_skins,
                        description: t::settings::general::privacy::hide_skins_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.hide_skins, |cfg, val| cfg.hide_skins = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::general::privacy::hide_server_addresses,
                        description: t::settings::general::privacy::hide_server_addresses_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.hide_server_addresses, |cfg, val| cfg.hide_server_addresses = val),
                        ..Default::default()
                    },
                    // todo: Only hide when OBS is open
                ].into(),
                searched_items: None
            }
        ].into(),
        searched_groups: None
    }
}

fn create_language_dropdown(window: &mut Window, cx: &mut App) -> SettingItemWidget {
    let languages = std::iter::once(NamedDropdownItem {
            name: DropdownName::Translated(t::settings::general::general::language::system),
            item: SharedString::new_static("system")
        })
        .chain(t::languages().into_iter().map(|(id, name)| NamedDropdownItem {
            name: DropdownName::Literal(SharedString::new_static(*name)),
            item: SharedString::new_static(*id)
        }))
        .collect();

    let initial = match &InterfaceConfig::get(cx).language {
        t::Language::System => "system".into(),
        t::Language::Code(code) => code.into(),
    };
    let dropdown = SearchableNamedDropdown::create_and_select(languages, initial, window, cx);

    cx.subscribe(&dropdown, |_, event: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(Some(value)) = event else {
            return
        };
        let language = match &**value {
            "system" => t::Language::System,
            lang => t::Language::Code(lang.into())
        };
        t::set_lang(&language);
        InterfaceConfig::get_mut(cx).language = language;
        cx.refresh_windows();
    }).detach();

    SettingItemWidget::Any(Rc::new(move |_, _| {
        Select::new(&dropdown).menu_width(px(200.0)).search_placeholder(t::common::search()).into_any_element()
    }))
}

fn create_live_game_output_dropdown(window: &mut Window, cx: &mut App) -> SettingItemWidget {
    let options = vec![
        NamedDropdownItem {
            name: DropdownName::translated(t::settings::general::general::live_game_output_display::tab_on_instance_page),
            item: LiveGameOutputDisplay::TabOnInstancePage,
        },
        NamedDropdownItem {
            name: DropdownName::translated(t::settings::general::general::live_game_output_display::separate_window),
            item: LiveGameOutputDisplay::SeparateWindow,
        },
        NamedDropdownItem {
            name: DropdownName::translated(t::settings::general::general::live_game_output_display::hidden),
            item: LiveGameOutputDisplay::Hidden,
        },
    ];

    let initial = InterfaceConfig::get(cx).live_game_output_display;
    let dropdown = NamedDropdown::create_and_select(options, initial, window, cx);

    cx.subscribe(&dropdown, |_, event: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(Some(value)) = event else {
            return
        };
        InterfaceConfig::get_mut(cx).live_game_output_display = *value;
        cx.refresh_windows();
    }).detach();

    SettingItemWidget::Any(Rc::new(move |_, _| {
        Select::new(&dropdown).menu_width(px(200.0)).into_any_element()
    }))
}
