use std::rc::Rc;

use gpui::*;
use gpui_component::{ActiveTheme, ThemeRegistry, button::{Button, ButtonVariants}, select::{Select, SelectEvent}, slider::{Slider, SliderEvent, SliderState}, h_flex};

use crate::{component::named_dropdown::{DropdownName, NamedDropdownItem, SearchableNamedDropdown}, entity::DataEntities, icon::PandoraIcon, interface_config::InterfaceConfig, settings::{SettingGroup, SettingItem, SettingItemWidget, SettingPage}};

pub(super) fn create_page(data: &DataEntities, window: &mut Window, cx: &mut App) -> SettingPage {
    SettingPage {
        title: t::settings::appearance,
        groups: vec![
            SettingGroup {
                title: Some(t::settings::appearance::theme),
                items: vec![
                    SettingItem {
                        title: t::settings::appearance::theme::base_theme,
                        description: t::settings::appearance::theme::base_theme_desc,
                        widget: create_theme_dropdown(window, cx),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::theme::open_theme_folder,
                        description: t::settings::appearance::theme::open_theme_folder_desc,
                        widget: SettingItemWidget::Any(Rc::new({
                            let theme_folder = data.theme_folder.clone();
                            move |_, _| {
                                Button::new("open-theme-folder").info().icon(PandoraIcon::FolderOpen).label(t::settings::appearance::theme::open_theme_folder()).on_click({
                                    let theme_folder = theme_folder.clone();
                                    move |_, window, cx| {
                                        crate::open_folder(&theme_folder, window, cx);
                                    }
                                }).into_any_element()
                            }
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::background_image,
                        description: t::settings::appearance::background_image_desc,
                        widget: SettingItemWidget::Any(Rc::new(|_window, cx| {
                            let has_background = InterfaceConfig::get(cx).background_image.is_some();
                            let choose = Button::new("choose-background")
                                .icon(PandoraIcon::Image)
                                .label(if has_background { t::settings::appearance::change_image() } else { t::settings::appearance::choose_image() })
                                .on_click(cx.listener(|root, _, window, cx| {
                                    root.select_file(t::settings::appearance::choose_background(), |_, path, cx| {
                                        InterfaceConfig::get_mut(cx).background_image = path.map(|path| path.to_string_lossy().into_owned());
                                        cx.refresh_windows();
                                    }, window, cx);
                                }));
                            let controls = h_flex().gap_2();
                            let controls = if has_background {
                                let clear = Button::new("clear-background").label("×").danger().on_click(cx.listener(|_, _, _, cx| {
                                    InterfaceConfig::get_mut(cx).background_image = None;
                                    cx.refresh_windows();
                                }));
                                controls.child(choose).child(clear)
                            } else {
                                controls.child(choose)
                            };
                            controls.into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::background_opacity,
                        description: t::settings::appearance::background_opacity_desc,
                        widget: SettingItemWidget::Any(Rc::new(|window, cx| {
                            let opacity = InterfaceConfig::get(cx).background_opacity as f32;
                            let enabled = InterfaceConfig::get(cx).background_image.is_some();
                            let state = window.use_keyed_state("background-opacity", cx, move |_, _| {
                                SliderState::new().min(0.0).max(100.0).step(1.0).default_value(opacity)
                            });
                            cx.subscribe_in(&state, window, |_, _, event: &SliderEvent, _, cx| {
                                if let SliderEvent::Change(gpui_component::slider::SliderValue::Single(value)) = event {
                                    InterfaceConfig::get_mut(cx).background_opacity = *value as i32;
                                    cx.refresh_windows();
                                }
                            }).detach();
                            Slider::new(&state).w_32().px_2().disabled(!enabled).opacity(if enabled { 1.0 } else { 0.33 }).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::theme::font,
                        description: t::settings::appearance::theme::font_desc,
                        widget: create_font_dropdown(window, cx),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::theme::font_size,
                        description: t::settings::appearance::theme::font_size_desc,
                        widget: SettingItemWidget::Integer {
                            get: |cx| cx.theme().font_size.as_f32().round() as i32,
                            set: |val, cx| {
                                InterfaceConfig::get_mut(cx).set_font_size(val);
                                InterfaceConfig::apply_theme(cx, true);
                            },
                            min: Some(8),
                            max: Some(32),
                        },
                        ..Default::default()
                    },
                ].into(),
                searched_items: None
            },
            SettingGroup {
                title: Some(t::settings::appearance::window),
                items: vec![
                    SettingItem {
                        title: t::settings::appearance::window::use_os_titlebar,
                        description: t::settings::appearance::window::use_os_titlebar_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.use_os_titlebar, |cfg, val| cfg.use_os_titlebar = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::window::hide_main_window,
                        description: t::settings::appearance::window::hide_main_window_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.hide_main_window_on_launch, |cfg, val| cfg.hide_main_window_on_launch = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::window::quit_when_main_window_closed,
                        description: t::settings::appearance::window::quit_when_main_window_closed_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.quit_on_main_closed, |cfg, val| cfg.quit_on_main_closed = val),
                        ..Default::default()
                    },
                ].into(),
                searched_items: None
            },
            SettingGroup {
                title: Some(t::settings::appearance::sidebar),
                items: vec![
                    SettingItem {
                        title: t::settings::appearance::sidebar::show_sidebar_icons,
                        description: t::settings::appearance::sidebar::show_sidebar_icons_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.show_sidebar_icons, |cfg, val| cfg.show_sidebar_icons = val),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::appearance::sidebar::show_quickplay_page,
                        description: t::settings::appearance::sidebar::show_quickplay_page_desc,
                        widget: SettingItemWidget::Switch(|cfg| cfg.show_quickplay_page, |cfg, val| cfg.show_quickplay_page = val),
                        ..Default::default()
                    },
                ].into(),
                searched_items: None
            },
        ].into(),
        searched_groups: None
    }
}

fn create_theme_dropdown(window: &mut Window, cx: &mut App) -> SettingItemWidget {
    let themes = ThemeRegistry::global(cx).sorted_themes()
        .iter()
        .map(|theme| NamedDropdownItem {
            name: DropdownName::Literal(theme.name.clone()),
            item: theme.name.clone()
        })
        .collect();

    let initial = cx.theme().theme_name().clone();
    let dropdown = SearchableNamedDropdown::create_and_select(themes, initial, window, cx);

    cx.subscribe(&dropdown, |_, event: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(Some(theme_name)) = event else {
            return
        };
        InterfaceConfig::get_mut(cx).set_active_theme(theme_name.clone());
        InterfaceConfig::apply_theme(cx, true);
    }).detach();

    SettingItemWidget::Any(Rc::new(move |_, _| {
        Select::new(&dropdown).menu_width(px(200.0)).into_any_element()
    }))
}

fn create_font_dropdown(window: &mut Window, cx: &mut App) -> SettingItemWidget {
    let fonts = cx.text_system().all_font_names()
        .iter().filter(|name| !name.starts_with('.')).map(|font_name| {
            let font_name = SharedString::from(font_name);
            NamedDropdownItem { name: font_name.clone().into(), item: font_name }
        }).collect();

    let initial = cx.theme().font_family.clone();
    let dropdown = SearchableNamedDropdown::create_and_select(fonts, initial, window, cx);

    cx.subscribe(&dropdown, |_, event: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(Some(font_family)) = event else {
            return
        };

        InterfaceConfig::get_mut(cx).set_font_family(font_family.clone());
        InterfaceConfig::apply_theme(cx, true);
    }).detach();

    SettingItemWidget::Any(Rc::new(move |_, _| {
        Select::new(&dropdown).menu_width(px(200.0)).into_any_element()
    }))
}
