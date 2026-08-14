use std::{path::Path, sync::Arc};

use bridge::{handle::BackendHandle, message::{BackendConfigWithPassword, MessageToBackend}};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    IndexPath,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput},
    scroll::ScrollableElement,
    select::{SearchableVec, Select, SelectEvent, SelectState},
    sheet::Sheet,
    spinner::Spinner,
    tab::{Tab, TabBar},
    v_flex, ActiveTheme, Disableable, Sizable, ThemeRegistry,
};
use schema::backend_config::{BackendConfig, ProxyConfig, ProxyProtocol};

use crate::{
    component::named_dropdown::{NamedDropdown, NamedDropdownItem},
    entity::DataEntities,
    icon::PandoraIcon,
    interface_config::InterfaceConfig,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SettingsTab {
    #[default]
    Interface,
    Network,
    Java,
}

struct Settings {
    selected_tab: SettingsTab,
    language_select: Entity<SelectState<NamedDropdown<t::Language>>>,
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

    // Java settings state
    java_provider_select: Entity<SelectState<NamedDropdown<schema::java_manager::JavaProvider>>>,
    java_versions: Option<Vec<schema::java_manager::JavaVariant>>,
    fetch_java_versions_task: Option<Task<()>>,
}

pub fn build_settings_sheet(data: &DataEntities, window: &mut Window, cx: &mut App) -> impl Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static {
    let theme_folder = data.theme_folder.clone();
    let settings = cx.new(|cx| {
        let language_select = cx.new(|cx| {
            let lang_options = Settings::build_language_options();
            let lang = &InterfaceConfig::get(cx).language;
            let selected_index = lang_options.iter()
                .position(|item| item.item == *lang)
                .map(IndexPath::new);
            SelectState::new(NamedDropdown::new(lang_options), selected_index, window, cx)
        });

        cx.subscribe_in(&language_select, window, Settings::on_language_changed).detach();

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

        let java_provider_select = cx.new(|cx| {
            let options = vec![
                NamedDropdownItem { name: "Mojang".into(), item: schema::java_manager::JavaProvider::Mojang },
                NamedDropdownItem { name: "Adoptium".into(), item: schema::java_manager::JavaProvider::Adoptium },
                NamedDropdownItem { name: "Zulu".into(), item: schema::java_manager::JavaProvider::Zulu },
            ];
            SelectState::new(NamedDropdown::new(options), Some(IndexPath::new(0)), window, cx)
        });

        let mut settings = Settings {
            selected_tab: SettingsTab::Interface,
            language_select,
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
            java_provider_select,
            java_versions: None,
            fetch_java_versions_task: None,
        };

        cx.subscribe(&settings.proxy_protocol_select, Settings::on_proxy_protocol_changed).detach();
        cx.subscribe(&settings.proxy_host_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_port_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_username_input, Settings::on_proxy_input_changed).detach();
        cx.subscribe(&settings.proxy_password_input, Settings::on_proxy_password_changed).detach();
        cx.subscribe_in(&settings.java_provider_select, window, Settings::on_java_provider_changed).detach();

        settings.update_backend_configuration(window, cx);
        
        settings.fetch_java_versions(window, cx);

        settings
    });

    let version = option_env!("PANDORA_RELEASE_VERSION").unwrap_or("Dev");
    let version_string = if let Some(git_rev) = option_env!("GIT_REVISION") {
        SharedString::new(format!("{} ({})", version, git_rev))
    } else {
        version.into()
    };
    let version_icon = if version == "Dev" {
        PandoraIcon::GitBranch
    } else {
        PandoraIcon::Rocket
    };

    move |sheet, _, cx| {
        sheet
            .title(t::settings::title())
            .size(px(420.))
            .p_0()
            .when(cfg!(target_os = "macos"), |this| this.pt_5())
            .child(v_flex()
                .size_full()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(settings.clone())
            )
            .child(h_flex().p_2().gap_2().child(version_icon.clone()).child(version_string.clone()))
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
        let SelectEvent::Confirm(_) = event;
        self.save_proxy_config(_cx);
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

        if let Some(backend_config) = &mut self.backend_config {
            if !self.proxy_password_changed && backend_config.proxy == config {
                return;
            }
            backend_config.proxy = config.clone();
        }

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

    fn build_language_options() -> Vec<NamedDropdownItem<t::Language>> {
        std::iter::once(NamedDropdownItem {
            name: t::settings::language::system().into(),
            item: t::Language::System,
        }).chain(t::languages().iter().map(|&(code, name)| NamedDropdownItem {
            name: name.into(),
            item: t::Language::Code(code.to_string()),
        }))
        .collect()
    }

    fn on_language_changed(
        &mut self,
        _state: &Entity<SelectState<NamedDropdown<t::Language>>>,
        event: &SelectEvent<NamedDropdown<t::Language>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(_) = event;
        let Some(lang_item) = self.language_select.read(cx).selected_value().cloned() else {
            return;
        };
        let lang = lang_item.item;
        t::set_lang(&lang);

        let lang_options = Self::build_language_options();
        let selected_index = lang_options.iter()
            .position(|option| option.item == lang)
            .map(IndexPath::new);

        InterfaceConfig::get_mut(cx).language = lang;

        self.language_select.update(cx, |select, cx| {
            select.set_items(NamedDropdown::new(lang_options), window, cx);
            select.set_selected_index(selected_index, window, cx);
        });

        cx.notify();
    }

    fn render_interface_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let interface_config = InterfaceConfig::get(cx);

        let mut div = v_flex()
            .px_4()
            .py_3()
            .gap_3()
            .child(crate::labelled(
                t::settings::language::title(),
                Select::new(&self.language_select)
            ))
            .child(crate::labelled(
                t::settings::theme::title(),
                Select::new(&self.theme_select).search_placeholder(t::common::search())
            ))
            .child(Button::new("open-theme-folder").info().icon(PandoraIcon::FolderOpen).label(t::settings::theme::open_folder()).on_click({
                let theme_folder = self.theme_folder.clone();
                move |_, window, cx| {
                    crate::open_folder(&theme_folder, window, cx);
                }
            }))
            .child(Button::new("open-theme-repo").info().icon(PandoraIcon::Globe).label(t::settings::theme::open_repo()).on_click({
                move |_, _, cx| {
                    cx.open_url("https://github.com/longbridge/gpui-component/tree/main/themes");
                }
            }))
            .child(crate::labelled(t::settings::delete::title(),
                v_flex().gap_2()
                    .child(Checkbox::new("confirm-delete-mods")
                        .label(t::settings::delete::skip_mod_delete_confirmation())
                        .checked(interface_config.quick_delete_mods)
                        .on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_mods = *value;
                        }))
                    .child(Checkbox::new("confirm-delete-instance")
                        .label(t::settings::delete::skip_instance_delete_confirmation())
                        .checked(interface_config.quick_delete_instance).on_click(|value, _, cx| {
                            InterfaceConfig::get_mut(cx).quick_delete_instance = *value;
                        }))
                    )
            );

        if let Some(backend_config) = &self.backend_config {
            div = div
                .child(crate::labelled(
                    t::settings::windows::title(),
                    v_flex().gap_2()
                        .child(Checkbox::new("hide-on-launch")
                            .label(t::settings::windows::hide_main_window())
                            .checked(interface_config.hide_main_window_on_launch)
                            .on_click(|value, _, cx| {
                                InterfaceConfig::get_mut(cx).hide_main_window_on_launch = *value;
                            }))
                        .child(Checkbox::new("open-game-output")
                            .label(t::settings::windows::open_game_output())
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
                        .child(Checkbox::new("quit-on-main-close")
                            .label(t::settings::windows::close_all_when_main_closed())
                            .checked(interface_config.quit_on_main_closed)
                            .on_click(|value, _, cx| {
                                InterfaceConfig::get_mut(cx).quit_on_main_closed = *value;
                            }))
                        .child(Checkbox::new("use-os-titlebar")
                            .label(t::settings::windows::use_os_titlebar())
                            .checked(interface_config.use_os_titlebar)
                            .on_click(|value, _, cx| {
                                InterfaceConfig::get_mut(cx).use_os_titlebar = *value;
                            }))
                ))
        } else {
            div = div.child(Spinner::new().large());
        }

        div = div.child(crate::labelled(t::settings::privacy::title(),
            v_flex().gap_2()
                .child(Checkbox::new("hide-usernames")
                    .label(t::settings::privacy::hide_usernames())
                    .checked(interface_config.hide_usernames)
                    .on_click(|value, _, cx| {
                        InterfaceConfig::get_mut(cx).hide_usernames = *value;
                    }))
                .child(Checkbox::new("hide-skins")
                    .label(t::settings::privacy::hide_skins())
                    .checked(interface_config.hide_skins)
                    .on_click(|value, _, cx| {
                        InterfaceConfig::get_mut(cx).hide_skins = *value;
                    }))
                .child(Checkbox::new("hide-server-addresses")
                    .label(t::settings::privacy::hide_server_addresses())
                    .checked(interface_config.hide_server_addresses)
                    .on_click(|value, _, cx| {
                        InterfaceConfig::get_mut(cx).hide_server_addresses = *value;
                    }))
        ));

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
                t::settings::proxy::title(),
                v_flex().gap_2()
                    .child(Checkbox::new("proxy-enabled")
                        .label(t::settings::proxy::enabled())
                        .checked(proxy_enabled)
                        .on_click(cx.listener(|settings, value, _, cx| {
                            settings.proxy_enabled = *value;
                            settings.save_proxy_config(cx);
                            cx.notify();
                        })))
                    .child(h_flex().gap_2()
                        .child(v_flex().gap_1().w_32()
                            .child(t::settings::proxy::protocol())
                            .child(Select::new(&self.proxy_protocol_select)
                                .disabled(!proxy_enabled)
                                .w_full()))
                        .child(v_flex().gap_1().flex_1()
                            .child(t::settings::proxy::host())
                            .child(Input::new(&self.proxy_host_input)
                                .disabled(!proxy_enabled)))
                        .child(v_flex().gap_1().w_32()
                            .child(t::settings::proxy::port())
                            .child(NumberInput::new(&self.proxy_port_input)
                                .disabled(!proxy_enabled))))
            ))
            .child(crate::labelled(
                t::settings::proxy::auth(),
                v_flex().gap_2()
                    .child(Checkbox::new("proxy-auth-enabled")
                        .label(t::settings::proxy::use_auth())
                        .checked(proxy_auth_enabled)
                        .disabled(!proxy_enabled)
                        .on_click(cx.listener(|settings, value, _, cx| {
                            settings.proxy_auth_enabled = *value;
                            settings.save_proxy_config(cx);
                            cx.notify();
                        })))
                    .child(h_flex().gap_2()
                        .child(v_flex().gap_1().flex_1()
                            .child(t::settings::proxy::username())
                            .child(Input::new(&self.proxy_username_input)
                                .disabled(!proxy_enabled || !proxy_auth_enabled)))
                        .child(v_flex().gap_1().flex_1()
                            .child(t::settings::proxy::password())
                            .child(Input::new(&self.proxy_password_input)
                                .disabled(!proxy_enabled || !proxy_auth_enabled))))
            ))
            .child(div()
                .pt_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t::settings::proxy::launcher_only_note()))
    }

    fn render_java_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex().gap_4();

        list = list.child(
            v_flex().gap_1()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Java Provider"))
                .child(Select::new(&self.java_provider_select))
        );

        if self.fetch_java_versions_task.is_some() {
            list = list.child(h_flex().gap_2().child(Spinner::new()).child("Fetching available versions..."));
        } else if let Some(versions) = &self.java_versions {
            if versions.is_empty() {
                list = list.child(div().text_sm().text_color(cx.theme().muted_foreground).child("No versions found."));
            } else {
                for variant in versions {
                    let variant_clone = variant.clone();
                    list = list.child(
                        h_flex().w_full().justify_between().items_center().p_2().border_1().border_color(cx.theme().border).rounded_md()
                            .child(
                                h_flex().gap_3().items_center()
                                    .child(
                                        match variant.provider {
                                            schema::java_manager::JavaProvider::Mojang => crate::icon::PandoraIcon::Mojang,
                                            schema::java_manager::JavaProvider::Adoptium => crate::icon::PandoraIcon::Adoptium,
                                            schema::java_manager::JavaProvider::Zulu => crate::icon::PandoraIcon::Zulu,
                                        }
                                    )
                                    .child(
                                        v_flex().gap_0p5()
                                            .child(div().font_weight(FontWeight::BOLD).child(format!("Java {}", variant.major_version)))
                                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("OS: {} | Arch: {}", variant.os, variant.architecture)))
                                    )
                            )
                            .child(
                                if variant.is_installed {
                                    h_flex().gap_2().items_center()
                                        .child(
                                            Button::new(format!("uninstall_java_{}", variant.major_version))
                                                .icon(crate::icon::PandoraIcon::Close)
                                                .ghost()
                                                .danger()
                                                .on_click(cx.listener({
                                                    let variant_clone = variant_clone.clone();
                                                    move |settings, _, window, cx| {
                                                        settings.uninstall_java(variant_clone.clone(), window, cx);
                                                    }
                                                }))
                                        )
                                        .child(div().text_sm().text_color(cx.theme().success).child("Installed"))
                                } else {
                                    div().child(Button::new(format!("install_java_{}", variant.major_version)).label("Install").on_click(cx.listener(move |settings, _, window, cx| {
                                        settings.install_java(variant_clone.clone(), window, cx);
                                    })))
                                }
                            )
                    );
                }
            }
        }

        v_flex().p_4().size_full().overflow_y_scrollbar().child(list)
    }

    fn on_java_provider_changed(
        &mut self,
        _state: &Entity<SelectState<NamedDropdown<schema::java_manager::JavaProvider>>>,
        event: &SelectEvent<NamedDropdown<schema::java_manager::JavaProvider>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(_) = event;
        self.fetch_java_versions(window, cx);
    }

    fn fetch_java_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_java_versions_task.is_some() {
            return;
        }

        let Some(provider) = self.java_provider_select.read(cx).selected_value().cloned() else {
            return;
        };

        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::FetchJavaVersions {
            provider: provider.item,
            result: send,
        });

        self.fetch_java_versions_task = Some(cx.spawn_in(window, async move |page, cx| {
            let result = recv.await.unwrap_or_default();
            let _ = page.update_in(cx, |settings, _, cx| {
                settings.java_versions = Some(result);
                settings.fetch_java_versions_task = None;
                cx.notify();
            });
        }));
    }

    fn install_java(&mut self, variant: schema::java_manager::JavaVariant, window: &mut Window, cx: &mut Context<Self>) {
        let modal_action = bridge::modal_action::ModalAction::default();
        self.backend_handle.send(MessageToBackend::InstallJava {
            variant,
            modal_action: modal_action.clone(),
        });
        crate::modals::generic::show_modal(window, cx, "Installing Java".into(), "Error installing Java".into(), modal_action.clone());

        let modal_action_clone = modal_action.clone();
        cx.spawn_in(window, async move |page, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                if modal_action_clone.get_finished_at().is_some() || modal_action_clone.has_requested_cancel() {
                    let _ = page.update_in(cx, |settings, window, cx| {
                        settings.fetch_java_versions(window, cx);
                    });
                    break;
                }
            }
        }).detach();
    }

    fn uninstall_java(&mut self, variant: schema::java_manager::JavaVariant, window: &mut Window, cx: &mut Context<Self>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        self.backend_handle.send(MessageToBackend::UninstallJava { variant, result: send });
        
        cx.spawn_in(window, async move |page, cx| {
            if let Ok(_) = recv.await {
                let _ = page.update_in(cx, |settings, window, cx| {
                    settings.fetch_java_versions(window, cx);
                });
            }
        }).detach();
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
                SettingsTab::Java => 2,
            })
            .underline()
            .child(Tab::new().label(t::settings::interface()))
            .child(Tab::new().label(t::settings::network()))
            .child(Tab::new().label("Java"))
            .on_click(cx.listener(|settings, index, _window, cx| {
                settings.selected_tab = match index {
                    0 => SettingsTab::Interface,
                    1 => SettingsTab::Network,
                    2 => SettingsTab::Java,
                    _ => SettingsTab::Interface,
                };
                cx.notify();
            }));

        let content = match selected_tab {
            SettingsTab::Interface => self.render_interface_tab(window, cx).into_any_element(),
            SettingsTab::Network => self.render_network_tab(window, cx).into_any_element(),
            SettingsTab::Java => self.render_java_tab(window, cx).into_any_element(),
        };

        v_flex()
            .child(tab_bar)
            .child(content)
    }
}
