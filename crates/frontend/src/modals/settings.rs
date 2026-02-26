use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, message::{BackendConfigWithPassword, MessageToBackend}};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput},
    select::{SearchableVec, Select, SelectEvent, SelectState},
    sheet::Sheet,
    spinner::Spinner,
    tab::{Tab, TabBar},
    v_flex, ActiveTheme, Disableable, IconName, Sizable, ThemeRegistry,
};
use schema::backend_config::{BackendConfig, ProxyConfig, ProxyProtocol};

use crate::{entity::DataEntities, interface_config::InterfaceConfig, ts};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SettingsTab {
    #[default]
    Interface,
    Network,
}

struct Settings {
    selected_tab: SettingsTab,
    theme_folder: Arc<Path>,
    theme_select: Entity<SelectState<SearchableVec<SharedString>>>,
    backend_handle: BackendHandle,
    pending_request: bool,
    backend_config: Option<BackendConfig>,
    get_configuration_task: Option<Task<()>>,
    // Proxy settings state
    proxy_enabled: bool,
    proxy_protocol_select: Entity<SelectState<Vec<&'static str>>>,
    proxy_host_input: Entity<InputState>,
    proxy_port_input: Entity<InputState>,
    proxy_auth_enabled: bool,
    proxy_username_input: Entity<InputState>,
    proxy_password_input: Entity<InputState>,
    proxy_password_changed: bool,
}

pub fn build_settings_sheet(data: &DataEntities, window: &mut Window, cx: &mut App) -> impl Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static {
    let theme_folder = data.theme_folder.clone();
    let settings = cx.new(|cx| {
        let theme_select_delegate = SearchableVec::new(ThemeRegistry::global(cx).sorted_themes()
            .iter().map(|cfg| cfg.name.clone()).collect::<Vec<_>>());

        let theme_select = cx.new(|cx| {
            let mut state = SelectState::new(theme_select_delegate, Default::default(), window, cx).searchable(true);
            state.set_selected_value(&cx.theme().theme_name().clone(), window, cx);
            state
        });

        cx.subscribe_in(&theme_select, window, |_, entity, _: &SelectEvent<_>, _, cx| {
            let Some(theme_name) = entity.read(cx).selected_value().cloned() else {
                return;
            };

            InterfaceConfig::get_mut(cx).active_theme = theme_name.clone();

            let Some(theme) = gpui_component::ThemeRegistry::global(cx).themes().get(&SharedString::new(theme_name.trim_ascii())).cloned() else {
                return;
            };

            gpui_component::Theme::global_mut(cx).apply_config(&theme);
        }).detach();

        let proxy_protocol_select = cx.new(|cx| {
            let protocols = vec!["HTTP", "HTTPS", "SOCKS5"];
            let mut state = SelectState::new(protocols, None, window, cx);
            state.set_selected_value(&"HTTP", window, cx);
            state
        });

        let proxy_host_input = cx.new(|cx| InputState::new(window, cx).placeholder("proxy.example.com"));
        let proxy_port_input = cx.new(|cx| InputState::new(window, cx).default_value("8080".to_string()));
        let proxy_username_input = cx.new(|cx| InputState::new(window, cx).placeholder("username"));
        let proxy_password_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("password");
            state.set_masked(true, window, cx);
            state
        });

        let mut settings = Settings {
            selected_tab: SettingsTab::Interface,
            theme_folder,
            theme_select,
            backend_handle: data.backend_handle.clone(),
            pending_request: false,
            backend_config: None,
            get_configuration_task: None,
            proxy_enabled: false,
            proxy_protocol_select,
            proxy_host_input,
            proxy_port_input,
            proxy_auth_enabled: false,
            proxy_username_input,
            proxy_password_input,
            proxy_password_changed: false,
        };

        cx.subscribe(&settings.proxy_protocol_select, Settings::on_proxy_protocol_changed).detach();
        cx.subscribe(&settings.proxy_host_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_port_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_username_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_password_input, Settings::on_proxy_password_changed).detach();

        settings.update_backend_configuration(window, cx);

        settings
    });

    move |sheet, window, cx| {
        sheet
            .title(ts!("settings.title"))
            .size(px(420.))
            .p_0()
            .child(v_flex()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(settings.clone())
            )
    }
}

impl Settings {
    pub fn update_backend_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.get_configuration_task.is_some() {
            self.pending_request = true;
            return;
        }

        let (send, recv) = tokio::sync::oneshot::channel();
        self.get_configuration_task = Some(cx.spawn_in(window, async move |page, cx| {
            let result: BackendConfigWithPassword = recv.await.unwrap_or_default();
            let _ = page.update_in(cx, move |settings, window, cx| {
                settings.proxy_enabled = result.config.proxy.enabled;
                settings.proxy_auth_enabled = result.config.proxy.auth_enabled;

                settings.proxy_host_input.update(cx, |input, cx| {
                    input.set_value(&result.config.proxy.host, window, cx);
                });
                settings.proxy_port_input.update(cx, |input, cx| {
                    input.set_value(result.config.proxy.port.to_string(), window, cx);
                });
                settings.proxy_username_input.update(cx, |input, cx| {
                    input.set_value(&result.config.proxy.username, window, cx);
                });
                settings.proxy_protocol_select.update(cx, |select, cx| {
                    select.set_selected_value(&result.config.proxy.protocol.name(), window, cx);
                });
                if let Some(ref password) = result.proxy_password {
                    settings.proxy_password_input.update(cx, |input, cx| {
                        input.set_value(password, window, cx);
                    });
                }

                settings.backend_config = Some(result.config);
                settings.get_configuration_task = None;
                cx.notify();

                if settings.pending_request {
                    settings.pending_request = false;
                    settings.update_backend_configuration(window, cx);
                }
            });
        }));

        self.backend_handle.send(MessageToBackend::GetBackendConfiguration {
            channel: send,
        });
    }

    fn on_proxy_protocol_changed(
        &mut self,
        _state: Entity<SelectState<Vec<&'static str>>>,
        event: &SelectEvent<Vec<&'static str>>,
        _cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(_) = event {
            self.save_proxy_config(_cx);
        }
    }

    fn on_proxy_input_changed(
        &mut self,
        _state: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Blur = event {
            self.save_proxy_config(cx);
        }
    }

    fn on_proxy_password_changed(
        &mut self,
        _state: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                self.proxy_password_changed = true;
            }
            InputEvent::Blur => {
                if self.proxy_password_changed {
                    self.save_proxy_config(cx);
                }
            }
            _ => {}
        }
    }

    fn get_proxy_config(&self, cx: &App) -> ProxyConfig {
        let protocol_name = self.proxy_protocol_select.read(cx).selected_value()
            .map(|s| *s)
            .unwrap_or("HTTP");

        ProxyConfig {
            enabled: self.proxy_enabled,
            protocol: ProxyProtocol::from_name(protocol_name),
            host: self.proxy_host_input.read(cx).value().to_string(),
            port: self.proxy_port_input.read(cx).value().parse().unwrap_or(8080),
            auth_enabled: self.proxy_auth_enabled,
            username: self.proxy_username_input.read(cx).value().to_string(),
        }
    }

    fn save_proxy_config(&mut self, cx: &mut Context<Self>) {
        let config = self.get_proxy_config(cx);
        let password = if self.proxy_password_changed {
            Some(self.proxy_password_input.read(cx).value().to_string())
        } else {
            None
        };

        self.backend_handle.send(MessageToBackend::SetProxyConfiguration {
            config,
            password,
        });

        self.proxy_password_changed = false;
    }

    fn render_interface_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let interface_config = InterfaceConfig::get(cx);

        let mut div = v_flex()
            .px_4()
            .py_3()
            .gap_3()
            .child(crate::labelled(
                ts!("settings.theme.title"),
                Select::new(&self.theme_select)
            ))
            .child(Button::new("open-theme-folder").info().icon(IconName::FolderOpen).label(ts!("settings.theme.open_folder")).on_click({
                let theme_folder = self.theme_folder.clone();
                move |_, window, cx| {
                    crate::open_folder(&theme_folder, window, cx);
                }
            }))
            .child(Button::new("open-theme-repo").info().icon(IconName::Globe).label(ts!("settings.theme.open_repo")).on_click({
                move |_, _, cx| {
                    cx.open_url("https://github.com/longbridge/gpui-component/tree/main/themes");
                }
            }))
            .child(crate::labelled(ts!("settings.delete.title"),
                v_flex().gap_2()
                    .child(Checkbox::new("confirm-delete-mods")
                        .label(ts!("settings.delete.skip_mod_delete_confirmation"))
                        .checked(interface_config.quick_delete_mods)
                        .on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_mods = *value;
                        }))
                    .child(Checkbox::new("confirm-delete-instance")
                        .label(ts!("settings.delete.skip_instance_delete_confirmation"))
                        .checked(interface_config.quick_delete_instance).on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_instance = *value;
                        }))
                    )
            );

        if let Some(backend_config) = &self.backend_config {
            div = div
                .child(crate::labelled(
                    ts!("settings.launch.title"),
                    v_flex().gap_2()
                        .child(Checkbox::new("hide-on-launch")
                            .label(ts!("settings.launch.hide_main_window"))
                            .checked(interface_config.hide_main_window_on_launch)
                            .on_click(|value, _, cx| {
                                InterfaceConfig::get_mut(cx).hide_main_window_on_launch = *value;
                            }))
                        .child(Checkbox::new("open-game-output")
                            .label(ts!("settings.launch.open_game_output"))
                            .checked(!backend_config.dont_open_game_output_when_launching)
                            .on_click(cx.listener({
                                let backend_handle = self.backend_handle.clone();
                                move |settings, value, window, cx| {
                                    backend_handle.send(MessageToBackend::SetOpenGameOutputAfterLaunching {
                                        value: *value
                                    });
                                    settings.update_backend_configuration(window, cx);
                                }
                            })))
                ))
        } else {
            div = div.child(Spinner::new().large());
        }

        div
    }

    fn render_network_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let proxy_enabled = self.proxy_enabled;
        let proxy_auth_enabled = self.proxy_auth_enabled;

        v_flex()
            .px_4()
            .py_3()
            .gap_3()
            .child(crate::labelled(
                ts!("settings.proxy.title"),
                v_flex().gap_2()
                    .child(Checkbox::new("proxy-enabled")
                        .label(ts!("settings.proxy.enabled"))
                        .checked(proxy_enabled)
                        .on_click(cx.listener(|settings, value, _, cx| {
                            settings.proxy_enabled = *value;
                            settings.save_proxy_config(cx);
                            cx.notify();
                        })))
                    .child(h_flex().gap_2()
                        .child(v_flex().gap_1().w_32()
                            .child(ts!("settings.proxy.protocol"))
                            .child(Select::new(&self.proxy_protocol_select)
                                .disabled(!proxy_enabled)
                                .w_full()))
                        .child(v_flex().gap_1().flex_1()
                            .child(ts!("settings.proxy.host"))
                            .child(Input::new(&self.proxy_host_input)
                                .disabled(!proxy_enabled)))
                        .child(v_flex().gap_1().w_32()
                            .child(ts!("settings.proxy.port"))
                            .child(NumberInput::new(&self.proxy_port_input)
                                .disabled(!proxy_enabled))))
            ))
            .child(crate::labelled(
                ts!("settings.proxy.auth"),
                v_flex().gap_2()
                    .child(Checkbox::new("proxy-auth-enabled")
                        .label(ts!("settings.proxy.use_auth"))
                        .checked(proxy_auth_enabled)
                        .disabled(!proxy_enabled)
                        .on_click(cx.listener(|settings, value, _, cx| {
                            settings.proxy_auth_enabled = *value;
                            settings.save_proxy_config(cx);
                            cx.notify();
                        })))
                    .child(h_flex().gap_2()
                        .child(v_flex().gap_1().flex_1()
                            .child(ts!("settings.proxy.username"))
                            .child(Input::new(&self.proxy_username_input)
                                .disabled(!proxy_enabled || !proxy_auth_enabled)))
                        .child(v_flex().gap_1().flex_1()
                            .child(ts!("settings.proxy.password"))
                            .child(Input::new(&self.proxy_password_input)
                                .disabled(!proxy_enabled || !proxy_auth_enabled))))
            ))
            .child(div()
                .pt_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(ts!("settings.proxy.launcher_only_note")))
    }
}
impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_tab = self.selected_tab;

        let tab_bar = TabBar::new("settings-tabs")
            .prefix(div().w_4())
            .selected_index(match selected_tab {
                SettingsTab::Interface => 0,
                SettingsTab::Network => 1,
            })
            .underline()
            .child(Tab::new().label(ts!("settings.interface")))
            .child(Tab::new().label(ts!("settings.network")))
            .on_click(cx.listener(|settings, index, _window, cx| {
                settings.selected_tab = match index {
                    0 => SettingsTab::Interface,
                    1 => SettingsTab::Network,
                    _ => SettingsTab::Interface,
                };
                cx.notify();
            }));

        let content = match selected_tab {
            SettingsTab::Interface => self.render_interface_tab(window, cx).into_any_element(),
            SettingsTab::Network => self.render_network_tab(window, cx).into_any_element(),
        };

        v_flex()
            .child(tab_bar)
            .child(content)
    }
}
