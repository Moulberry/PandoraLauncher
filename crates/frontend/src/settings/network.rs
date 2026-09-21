use std::rc::Rc;

use bridge::message::MessageToBackend;
use gpui::*;
use gpui_component::{ActiveTheme, FocusableExt, input::{Input, InputEvent, InputState, NumberInput}, select::{Select, SelectEvent, SelectState}, switch::Switch};
use schema::backend_config::ProxyProtocol;

use crate::{component::named_dropdown::{DropdownName, NamedDropdown, NamedDropdownItem}, settings::{SettingGroup, SettingItem, SettingItemWidget, SettingPage}};

pub(super) fn create_page() -> SettingPage {
    SettingPage {
        title: t::settings::network,
        groups: vec![
            SettingGroup {
                title: Some(t::settings::network::launcher_proxy),
                items: vec![
                    SettingItem {
                        title: t::settings::network::launcher_proxy::enable_proxy,
                        description: t::settings::network::launcher_proxy::enable_proxy_desc,
                        widget: SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
                            Switch::new("enable-proxy").checked(backend.proxy.enabled).on_click(cx.listener(|root, val, _, cx| {
                                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                                    return;
                                };
                                if proxy_settings.enabled != *val {
                                    proxy_settings.enabled = *val;
                                    root.set_proxy_settings(proxy_settings, cx);
                                }
                            })).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::protocol,
                        description: t::settings::network::launcher_proxy::protocol_desc,
                        widget: create_proxy_protocol_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::host,
                        description: t::settings::network::launcher_proxy::host_desc,
                        widget: create_proxy_host_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::port,
                        description: t::settings::network::launcher_proxy::port_desc,
                        widget: create_proxy_port_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::use_auth,
                        description: t::settings::network::launcher_proxy::use_auth_desc,
                        widget: SettingItemWidget::Backend(Rc::new(|backend, _, cx| {
                            Switch::new("enable-proxy-auth").checked(backend.proxy.auth_enabled).on_click(cx.listener(|root, val, _, cx| {
                                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                                    return;
                                };
                                if proxy_settings.auth_enabled != *val {
                                    proxy_settings.auth_enabled = *val;
                                    root.set_proxy_settings(proxy_settings, cx);
                                }
                            })).into_any_element()
                        })),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::username,
                        description: t::settings::network::launcher_proxy::username_desc,
                        widget: create_proxy_username_widget(),
                        ..Default::default()
                    },
                    SettingItem {
                        title: t::settings::network::launcher_proxy::password,
                        description: t::settings::network::launcher_proxy::password_desc,
                        widget: create_proxy_password_widget(),
                        ..Default::default()
                    },
                ].into(),
                searched_items: None
            },
        ].into(),
        searched_groups: None
    }
}


fn create_proxy_protocol_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let mut created = false;
        let state = window.use_keyed_state("proxy-protocol", cx, |window, cx| {
            created = true;
            let items = vec![
                NamedDropdownItem {
                    name: DropdownName::new("HTTP"),
                    item: ProxyProtocol::Http
                },
                NamedDropdownItem {
                    name: DropdownName::new("HTTPS"),
                    item: ProxyProtocol::Https
                },
                NamedDropdownItem {
                    name: DropdownName::new("SOCKS5"),
                    item: ProxyProtocol::Socks5
                },
            ];
            let mut state = SelectState::new(NamedDropdown::new(items), None, window, cx);
            state.set_selected_value(&backend.proxy.protocol, window, cx);
            state
        });
        if created {
            cx.subscribe(&state, |root, _, event: &SelectEvent<_>, cx| {
                let SelectEvent::Confirm(Some(val)) = event else {
                    return
                };
                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                    return;
                };
                if proxy_settings.protocol != *val {
                    proxy_settings.protocol = *val;
                    root.set_proxy_settings(proxy_settings, cx);
                }
            }).detach();
        } else if state.read(cx).selected_value() != Some(&backend.proxy.protocol) {
            state.update(cx, |state, cx| {
                state.set_selected_value(&backend.proxy.protocol, window, cx)
            });
        }
        Select::new(&state).menu_width(px(200.0)).into_any_element()
    }))
}

fn create_proxy_host_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let mut created = false;
        let state = window.use_keyed_state("proxy-host", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(&backend.proxy.host, window, cx);
            state
        });
        let mut dirty = false;
        if created {
            cx.subscribe_in(&state, window, |root, state, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. }) {
                    return;
                }
                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                    return;
                };
                let value = state.read(cx).value();
                if proxy_settings.host != value {
                    proxy_settings.host = value.into();
                    root.set_proxy_settings(proxy_settings, cx);
                }
                window.blur();
            }).detach();
        } else {
            let state_read = state.read(cx);
            if &*state_read.value() != &*backend.proxy.host {
                if !state_read.focus_handle(cx).is_focused(window) {
                    state.update(cx, |state, cx| {
                        state.set_value(&backend.proxy.host, window, cx)
                    });
                } else {
                    dirty = true;
                }
            }
        }
        let mut input = Input::new(&state).w(px(200.0));
        if dirty {
            input = input.focus_ring(false).border_1().border_color(cx.theme().warning);
        }
        input.into_any_element()
    }))
}

fn create_proxy_port_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let mut created = false;
        let state = window.use_keyed_state("proxy-port", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(format!("{}", backend.proxy.port), window, cx);
            state
        });
        let mut dirty = false;
        if created {
            cx.subscribe_in(&state, window, |root, state, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. }) {
                    return;
                }
                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                    return;
                };
                let value = state.read(cx).value();
                let Ok(value) = value.parse::<u16>() else {
                    return;
                };
                if proxy_settings.port != value {
                    proxy_settings.port = value;
                    root.set_proxy_settings(proxy_settings, cx);
                }
                window.blur();
            }).detach();
        } else {
            let state_read = state.read(cx);
            if state_read.value().parse::<u16>() != Ok(backend.proxy.port) {
                if !state_read.focus_handle(cx).is_focused(window) {
                    state.update(cx, |state, cx| {
                        state.set_value(format!("{}", backend.proxy.port), window, cx)
                    });
                } else {
                    dirty = true;
                }
            }
        }
        let mut input = NumberInput::new(&state).w(px(200.0));
        if dirty {
            input = input.focus_ring(false).border_1().border_color(cx.theme().warning);
        }
        input.into_any_element()
    }))
}

fn create_proxy_username_widget() -> SettingItemWidget {
    SettingItemWidget::Backend(Rc::new(|backend, window, cx| {
        let mut created = false;
        let state = window.use_keyed_state("proxy-username", cx, |window, cx| {
            created = true;
            let mut state = InputState::new(window, cx);
            state.set_value(&backend.proxy.username, window, cx);
            state
        });
        let mut dirty = false;
        if created {
            cx.subscribe_in(&state, window, |root, state, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. }) {
                    return;
                }
                let Some(mut proxy_settings) = root.backend_config().map(|b| b.proxy.clone()) else {
                    return;
                };
                let value = state.read(cx).value();
                if proxy_settings.username != value {
                    proxy_settings.username = value.into();
                    root.set_proxy_settings(proxy_settings, cx);
                }
                window.blur();
            }).detach();
        } else {
            let state_read = state.read(cx);
            if &*state_read.value() != &*backend.proxy.username {
                if !state_read.focus_handle(cx).is_focused(window) {
                    state.update(cx, |state, cx| {
                        state.set_value(&backend.proxy.username, window, cx)
                    });
                } else {
                    dirty = true;
                }
            }
        }
        let mut input = Input::new(&state).w(px(200.0));
        if dirty {
            input = input.focus_ring(false).border_1().border_color(cx.theme().warning);
        }
        input.into_any_element()
    }))
}

fn create_proxy_password_widget() -> SettingItemWidget {
    SettingItemWidget::Any(Rc::new(|window, cx| {
        let mut created = false;
        let state = window.use_keyed_state("proxy-password", cx, |window, cx| {
            created = true;
            InputState::new(window, cx).masked(true).placeholder(t::settings::network::launcher_proxy::password_hidden())
        });
        let mut dirty = false;
        let mut empty = false;
        if created {
            cx.subscribe_in(&state, window, |root, state, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. }) {
                    return;
                }

                let value = state.update(cx, |state, cx| {
                    let value = state.value();
                    state.set_value("", window, cx);
                    value
                });

                root.backend_handle.send(MessageToBackend::SetProxyPassword { password: value.into() });
                window.blur();
            }).detach();
        } else {
            let state_read = state.read(cx);
            empty = state_read.value().is_empty();
            dirty = state_read.focus_handle(cx).is_focused(window) || !empty;
        }
        let mut input = Input::new(&state).w(px(200.0));
        if dirty {
            input = input.focus_ring(false).border_1().border_color(cx.theme().warning);
        }
        if !empty {
            input = input.mask_toggle();
        }
        input.into_any_element()
    }))
}
