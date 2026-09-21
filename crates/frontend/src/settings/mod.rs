use std::{path::Path, rc::Rc, sync::Arc};

use bridge::{handle::BackendHandle, message::MessageToBackend};
use gpui::{*, prelude::*};
use gpui_component::{ActiveTheme, Root, StyledExt, WindowExt, h_flex, input::{Input, InputEvent, InputState}, notification::{Notification, NotificationType}, separator::Separator, spinner::Spinner, switch::Switch, v_flex};
use schema::{
    backend_config::{BackendConfig, ProxyConfig},
    instance::{InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceMemoryConfiguration},
};

use crate::{component::{generic_title_bar::TitleBar, resize_panel::{ResizePanel, ResizePanelState}}, entity::DataEntities, icon::PandoraIcon, interface_config::InterfaceConfig};

mod general;
mod appearance;
mod network;
mod java;

struct SettingsRoot {
    settings: Settings,
    search_state: Entity<InputState>,
    group_list_state: ListState,
    use_custom_titlebar: bool,
    sidebar_state: ResizePanelState,
    backend_handle: BackendHandle,
    get_configuration_task: Option<Task<()>>,
    actual_backend_config: Option<BackendConfig>,
    temp_backend_config: Option<BackendConfig>,
    on_receive_backend_config: OnReceiveBackendConfig,
    _select_file_task: Task<()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OnReceiveBackendConfig {
    DoNothing,
    ClearTemp,
    RequestAgainThenClearTemp,
}

impl SettingsRoot {
    fn backend_config(&self) -> Option<&BackendConfig> {
        self.temp_backend_config.as_ref()
            .or(self.actual_backend_config.as_ref())
    }

    pub fn set_proxy_settings(&mut self, proxy_settings: ProxyConfig, cx: &mut Context<Self>) {
        if self.actual_backend_config.is_some() {
            self.on_receive_backend_config = if self.temp_backend_config.is_some() {
                OnReceiveBackendConfig::RequestAgainThenClearTemp
            } else {
                OnReceiveBackendConfig::ClearTemp
            };
            let pending = self.temp_backend_config.get_or_insert_with(|| self.actual_backend_config.clone().unwrap());
            pending.proxy = proxy_settings.clone();
        }

        self.backend_handle.send(MessageToBackend::SetProxyConfiguration {
            config: proxy_settings,
        });

        self.update_backend_configuration(cx);
    }

    pub fn set_launch_defaults(
        &mut self,
        memory: Option<InstanceMemoryConfiguration>,
        jvm_flags: Option<InstanceJvmFlagsConfiguration>,
        jvm_binary: Option<InstanceJvmBinaryConfiguration>,
        cx: &mut Context<Self>,
    ) {
        if self.actual_backend_config.is_some() {
            self.on_receive_backend_config = if self.temp_backend_config.is_some() {
                OnReceiveBackendConfig::RequestAgainThenClearTemp
            } else {
                OnReceiveBackendConfig::ClearTemp
            };
            let pending = self.temp_backend_config.get_or_insert_with(|| self.actual_backend_config.clone().unwrap());
            pending.memory = memory.clone();
            pending.jvm_flags = jvm_flags.clone();
            pending.jvm_binary = jvm_binary.clone();
        }

        self.backend_handle.send(MessageToBackend::SetLaunchDefaults {
            memory,
            jvm_flags,
            jvm_binary,
        });

        self.update_backend_configuration(cx);
    }

    pub fn select_file(
        &mut self,
        message: &'static str,
        handle: impl FnOnce(&mut Self, Option<Arc<Path>>, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(message.into()),
        });

        let this_entity = cx.entity();
        self._select_file_task = window.spawn(cx, async move |cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            _ = cx.update_window_entity(&this_entity, move |this, window, cx| {
                match result {
                    Ok(Some(paths)) => {
                        (handle)(this, paths.first().map(|v| v.as_path().into()), cx);
                        cx.notify();
                    },
                    Ok(None) => {},
                    Err(error) => {
                        let error = format!("{}", error);
                        let notification = Notification::new()
                            .autohide(false)
                            .with_type(NotificationType::Error)
                            .title(error);
                        window.push_notification(notification, cx);
                    },
                }
            });
        });
    }

    pub fn update_backend_configuration(&mut self, cx: &mut Context<Self>) {
        if self.get_configuration_task.is_some() {
            return;
        }

        let (send, recv) = tokio::sync::oneshot::channel();
        self.get_configuration_task = Some(cx.spawn(async move |page, cx| {
            let config: BackendConfig = recv.await.unwrap_or_default();
            _ = page.update(cx, move |settings, cx| {
                settings.actual_backend_config = Some(config);
                settings.get_configuration_task = None;

                match settings.on_receive_backend_config {
                    OnReceiveBackendConfig::DoNothing => {},
                    OnReceiveBackendConfig::ClearTemp => {
                        settings.temp_backend_config = None;
                        settings.on_receive_backend_config = OnReceiveBackendConfig::DoNothing;
                    },
                    OnReceiveBackendConfig::RequestAgainThenClearTemp => {
                        settings.update_backend_configuration(cx);
                        settings.on_receive_backend_config = OnReceiveBackendConfig::ClearTemp;
                    },
                }

                cx.notify();
            });
        }));

        self.backend_handle.send(MessageToBackend::GetBackendConfiguration {
            channel: send,
        });
    }
}

impl Render for SettingsRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let sidebar = v_flex()
            .size_full()
            .p_3()
            .when(cfg!(target_os = "macos"), |this| this.pt_8())
            .gap_3()
            .child(Input::new(&self.search_state).prefix(PandoraIcon::Search).w_full())
            .children(self.render_sidebar_items(cx))
            .child(div().flex_1())
            .child(h_flex().text_sm().gap_2().child(version_icon.clone()).child(version_string.clone()));

        let left_content = if let Some(selected_page) = self.settings.selected_page {
            vec![(self.settings.pages[selected_page].title)().into_any_element()]
        } else {
            vec![]
        };

        let content = v_flex()
            .size_full()
            .child(TitleBar {
                left_content,
                right_content: vec![],
                content_only: !self.use_custom_titlebar,
            })
            .child(self.render_page(cx));

        ResizePanel::new(&self.sidebar_state, sidebar, content)
    }
}

impl SettingsRoot {
    pub fn render_sidebar_items(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (pages, mut page_elements) = if let Some(searched) = &self.settings.searched_pages {
            (itertools::Either::Left(searched.iter().copied()), Vec::with_capacity(searched.len()))
        } else {
            (itertools::Either::Right(0..self.settings.pages.len()), Vec::with_capacity(self.settings.pages.len()))
        };

        for page_ix in pages {
            let page = &self.settings.pages[page_ix];

            let mut page_element = h_flex()
                .id(("page_item", page_ix))
                .px_2()
                .py_px()
                .w_full()
                .justify_between()
                .rounded(cx.theme().radius)
                .text_sm()
                .overflow_x_hidden()
                .whitespace_nowrap()
                .child((page.title)())
                .on_click(cx.listener(move |root, _, _, cx| {
                    root.settings.selected_page = Some(page_ix);
                    root.settings.selected_group = None;
                    root.settings.deferred_scroll_to_group = false;
                    cx.notify();
                }));

            if self.settings.selected_page == Some(page_ix) {
                page_element = page_element.font_medium()
                    .bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground);
            } else {
                page_element = page_element.hover(|this| {
                    this.bg(cx.theme().sidebar_accent.opacity(0.8))
                        .text_color(cx.theme().sidebar_accent_foreground)
                })
            }

            if page.groups.len() > 1 {
                let (groups, mut group_elements) = if let Some(searched) = &page.searched_groups {
                    (itertools::Either::Left(searched.iter().copied()), Vec::with_capacity(searched.len()))
                } else {
                    (itertools::Either::Right(0..page.groups.len()), Vec::with_capacity(page.groups.len()))
                };

                for group_ix in groups {
                    let group = &page.groups[group_ix];

                    let Some(title) = group.title else {
                        continue;
                    };

                    let mut group_element = div()
                        .id(("group_item", page_ix * 100 + group_ix))
                        .px_2()
                        .py_px()
                        .w_full()
                        .rounded(cx.theme().radius)
                        .text_sm()
                        .overflow_x_hidden()
                        .whitespace_nowrap()
                        .hover(|this| {
                            this.bg(cx.theme().sidebar_accent.opacity(0.8))
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                        .child((title)())
                        .on_mouse_down_out(cx.listener(|root, _, _, cx| {
                            root.settings.selected_group = None;
                            root.settings.deferred_scroll_to_group = false;
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |root, _, _, cx| {
                            root.settings.selected_page = Some(page_ix);
                            root.settings.selected_group = Some(group_ix);
                            root.settings.deferred_scroll_to_group = true;
                            cx.notify();
                        }));

                    if self.settings.selected_page == Some(page_ix) && self.settings.selected_group == Some(group_ix) {
                        group_element = group_element.underline();
                    }

                    group_elements.push(group_element.into_any());
                }

                let combined = v_flex()
                    .w_full()
                    .child(page_element)
                    .child(h_flex()
                        .w_full()
                        .child(Separator::vertical().mx_2())
                        .child(v_flex().w_full().children(group_elements))
                    );

                page_elements.push(combined.into_any());
            } else {
                page_elements.push(page_element.into_any());
            }
        }

        page_elements
    }

    pub fn render_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(page_ix) = self.settings.selected_page else {
            return div().size_full().p_3().text_lg().child(t::common::no_results()).into_any();
        };

        let page = self.settings.pages[page_ix].clone();

        let group_count = page.searched_groups.as_ref().map(|g| g.len())
            .unwrap_or(page.groups.len());
        if self.group_list_state.item_count() != group_count {
            self.group_list_state.reset(group_count);
        }
        if self.settings.deferred_scroll_to_group {
            self.settings.deferred_scroll_to_group = false;
            if let Some(group) = self.settings.selected_group && group < group_count {
                self.group_list_state.scroll_to_reveal_item(group);
            }
        }
        let selected_group = self.settings.selected_group;

        let entity = cx.entity();
        list(self.group_list_state.clone(), move |mut group_ix, window, cx| {
            cx.update_entity(&entity, |root, cx| {
                if let Some(searched) = &page.searched_groups {
                    group_ix = searched[group_ix];
                }
                let group = &page.groups[group_ix];

                let (items, mut item_elements) = if let Some(searched) = &group.searched_items {
                    (itertools::Either::Left(searched.iter().copied()), Vec::with_capacity(searched.len()))
                } else {
                    (itertools::Either::Right(0..group.items.len()), Vec::with_capacity(group.items.len()))
                };

                let mut has_backend_missing = false;

                for item_ix in items {
                    let item = &group.items[item_ix];

                    let widget = match &item.widget {
                        SettingItemWidget::None => {
                            continue;
                        },
                        SettingItemWidget::Switch(get, set) => {
                            let value = (get)(InterfaceConfig::get(cx));
                            let set = *set;
                            Switch::new(("switch-element", page_ix * 10000 + group_ix * 100 + item_ix))
                                .checked(value)
                                .on_click(move |value, _, cx| {
                                    (set)(InterfaceConfig::get_mut(cx), *value);
                                    cx.refresh_windows();
                                })
                                .into_any_element()
                        },
                        SettingItemWidget::Integer { get, set, min, max } => {
                            let mut initialized = false;
                            let state = window.use_keyed_state(
                                ("integer-element", page_ix * 10000 + group_ix * 100 + item_ix),
                                cx,
                                |window, cx| {
                                    initialized = true;
                                    let mut state = InputState::new(window, cx);
                                    if let Some(min) = min {
                                        state = state.min(*min as f64);
                                    }
                                    if let Some(max) = max {
                                        state = state.max(*max as f64);
                                    }
                                    let value = (get)(cx);
                                    state.set_value(format!("{}", value), window, cx);
                                    state
                                }
                            );
                            if initialized {
                                let get = *get;
                                let set = *set;
                                let min = *min;
                                let max = *max;
                                cx.subscribe_in(&state, window, move |_, entity, event: &InputEvent, window, cx| {
                                    if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                                        return;
                                    }

                                    let value = entity.read(cx).value();

                                    if let Ok(val) = value.parse::<f64>() {
                                        let mut val = val.round() as i32;
                                        if let Some(min) = min {
                                            val = val.max(min);
                                        }
                                        if let Some(max) = max {
                                            val = val.min(max);
                                        }
                                        (set)(val, cx);
                                    }

                                    let new_value = format!("{}", (get)(cx));
                                    if new_value != value {
                                        entity.update(cx, |input, cx| {
                                            input.set_value(new_value, window, cx);
                                        });
                                    }
                                }).detach();
                            }
                            gpui_component::input::NumberInput::new(&state).w(px(200.0)).into_any_element()
                        },
                        SettingItemWidget::Backend(func) => {
                            let Some(backend_config) = root.backend_config() else {
                                has_backend_missing = true;
                                continue;
                            };

                            (func)(backend_config, window, cx)
                        },
                        SettingItemWidget::Any(func) => {
                            (func)(window, cx)
                        },
                    };

                    let item_element = h_flex()
                        .justify_between()
                        .items_center()
                        .w_full()
                        .child(v_flex()
                            .text_sm()
                            .flex_1()
                            .max_w_3_5()
                            .child((item.title)())
                            .child(div().text_color(cx.theme().muted_foreground).child((item.description)())))
                        .child(h_flex().max_w_2_5().min_w_0().overflow_x_hidden().justify_end().child(widget));

                    item_elements.push(item_element);
                }

                let items = v_flex()
                    .w_full()
                    .p_3()
                    .gap_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius_lg)
                    .line_height(rems(1.0))
                    .when(has_backend_missing, |this| {
                        this.child(h_flex().gap_1().child(t::settings::loading()).child(Spinner::new()))
                    })
                    .children(item_elements);

                if let Some(title) = group.title {
                    v_flex()
                        .size_full()
                        .px_3()
                        .py_1p5()
                        .child(div()
                            .text_color(cx.theme().muted_foreground)
                            .when(selected_group == Some(group_ix), Styled::underline)
                            .child((title)()))
                        .child(items)
                        .into_any_element()
                } else {
                    v_flex()
                        .size_full()
                        .p_3()
                        .pb_1p5()
                        .child(items)
                        .into_any_element()
                }
            })
        }).size_full().into_any_element()
    }

    pub fn on_search(&mut self, entity: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        if let InputEvent::Change = event {
            self.settings.search(&*entity.read(cx).value());
        }
    }
}

#[derive(Default, Clone)]
enum SettingItemWidget {
    #[default]
    None,
    Switch(fn(&InterfaceConfig) -> bool, fn(&mut InterfaceConfig, bool)),
    Integer {
        get: fn(&App) -> i32,
        set: fn(i32, &mut App),
        min: Option<i32>,
        max: Option<i32>,
    },
    Backend(Rc<dyn Fn(&BackendConfig, &mut Window, &mut Context<SettingsRoot>) -> AnyElement>),
    Any(Rc<dyn Fn(&mut Window, &mut Context<SettingsRoot>) -> AnyElement>),
}

struct SettingItem {
    title: fn() -> &'static str,
    description: fn() -> &'static str,
    widget: SettingItemWidget,
    casefolded_title: Option<String>,
    casefolded_description: Option<String>,
}

impl Clone for SettingItem {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            description: self.description.clone(),
            widget: self.widget.clone(),
            casefolded_title: None,
            casefolded_description: None
        }
    }
}

impl Default for SettingItem {
    fn default() -> Self {
        Self {
            title: || "",
            description: || "",
            widget: SettingItemWidget::default(),
            casefolded_title: None,
            casefolded_description: None
        }
    }
}

impl SettingItem {
    pub fn matches_search(&self, query: &str) -> bool {
        if let Some(casefolded_title) = &self.casefolded_title && casefolded_title.contains(query) {
            return true;
        }
        if let Some(casefolded_description) = &self.casefolded_description && casefolded_description.contains(query) {
            return true;
        }
        false
    }
}

#[derive(Clone)]
struct SettingGroup {
    title: Option<fn() -> &'static str>,
    items: Rc<[SettingItem]>,
    searched_items: Option<Rc<[usize]>>,
}

impl SettingGroup {
    pub fn reset_search(&mut self) {
        self.searched_items = None;
    }

    pub fn refine_search(&mut self, query: &str) -> bool {
        debug_assert!(!query.is_empty());

        let Some(previous) = &self.searched_items else {
            return self.search(query);
        };

        let mut searched = previous.to_vec();
        searched.retain(|index| self.items[*index].matches_search(query));
        let any_matches = !searched.is_empty();
        self.searched_items = Some(searched.into());
        any_matches
    }

    pub fn search(&mut self, query: &str) -> bool {
        if query.is_empty() {
            self.searched_items = None;
            return true;
        }

        let mut searched = Vec::new();
        for (index, group) in self.items.iter().enumerate() {
            if group.matches_search(query) {
                searched.push(index);
            }
        }
        let any_matches = !searched.is_empty();
        if searched.len() == self.items.len() {
            self.searched_items = None;
        } else {
            self.searched_items = Some(searched.into());
        }
        any_matches
    }
}

#[derive(Clone)]
struct SettingPage {
    title: fn() -> &'static str,
    groups: Rc<[SettingGroup]>,
    searched_groups: Option<Rc<[usize]>>,
}

impl SettingPage {
    pub fn reset_search(&mut self) {
        let groups = Rc::make_mut(&mut self.groups);
        for group in groups {
            group.reset_search();
        }
        self.searched_groups = None;
    }

    pub fn refine_search(&mut self, query: &str) -> bool {
        debug_assert!(!query.is_empty());

        let Some(previous) = &self.searched_groups else {
            return self.search(query);
        };

        let groups = Rc::make_mut(&mut self.groups);

        let mut searched = previous.to_vec();
        searched.retain(|index| groups[*index].refine_search(query));
        let any_matches = !searched.is_empty();
        self.searched_groups = Some(searched.into());
        any_matches
    }

    pub fn search(&mut self, query: &str) -> bool {
        debug_assert!(!query.is_empty());

        let groups = Rc::make_mut(&mut self.groups);

        let mut searched = Vec::new();
        for (index, group) in groups.iter_mut().enumerate() {
            if group.search(query) {
                searched.push(index);
            }
        }
        let any_matches = !searched.is_empty();
        if searched.len() == self.groups.len() {
            self.searched_groups = None;
        } else {
            self.searched_groups = Some(searched.into());
        }
        any_matches
    }
}

struct Settings {
    pages: Box<[SettingPage]>,
    selected_page: Option<usize>,
    selected_group: Option<usize>,
    deferred_scroll_to_group: bool,
    searched_pages: Option<Vec<usize>>,
    last_language_id: Option<u8>,
    last_query: Option<String>,
}

impl Settings {
    pub fn search(&mut self, query: &str) {
        self.selected_group = None;
        self.deferred_scroll_to_group = false;

        if query.is_empty() {
            for page in &mut self.pages {
                page.reset_search();
            }
            self.last_query = None;
            self.searched_pages = None;
            if self.selected_page.is_none() {
                self.selected_page = Some(0);
            }
            return;
        }

        let lang_id = t::get_current_lang_id();

        if self.last_language_id != Some(lang_id) {
            self.last_language_id = Some(lang_id);

            for page in &mut self.pages {
                let groups = Rc::make_mut(&mut page.groups);
                for group in groups {
                    let items = Rc::make_mut(&mut group.items);
                    for item in items {
                        item.casefolded_title = Some(casefold::simple_fold((item.title)().to_string()));
                        item.casefolded_description = Some(casefold::simple_fold((item.description)().to_string()));
                    }
                }
            }
        }

        let query = casefold::simple_fold(query.to_string());

        let searched = if let Some(searched) = &mut self.searched_pages && let Some(last_query) = &self.last_query && query.contains(last_query) {
            searched.retain(|index| self.pages[*index].refine_search(&*query));
            searched
        } else {
            let searched = self.searched_pages.get_or_insert_default();
            searched.clear();

            for (index, page) in self.pages.iter_mut().enumerate() {
                if page.search(&*query) {
                    searched.push(index);
                }
            }
            searched
        };

        // Update selected page
        if searched.is_empty() {
            self.selected_page = None;
        } else if let Some(old_selected) = self.selected_page {
            if !searched.contains(&old_selected) {
                self.selected_page = Some(searched.first().copied().unwrap());
            }
        } else {
            self.selected_page = Some(searched.first().copied().unwrap());
        }

        // Set last query
        self.last_query = Some(query);
    }
}

pub fn open_settings_window(main_window: &Window, data: &DataEntities, cx: &mut App) {
    let existing = cx.windows().into_iter().find_map(|w| {
        let root = w.downcast::<Root>()?;
        if !root.read(cx).ok()?.view().clone().downcast::<SettingsRoot>().is_ok() {
            return None;
        }
        Some(root)
    });
    if let Some(existing) = existing {
        _ = existing.update(cx, |_, window, _| window.activate_window());
        return;
    }

    let use_custom_titlebar = crate::root::should_render_custom_titlebar();
    let display_id = main_window.display(cx).map(|d| d.id());
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(t::settings::title().into()),
            appears_transparent: use_custom_titlebar,
            ..Default::default()
        }),
        app_owns_titlebar_drag: use_custom_titlebar,
        window_decorations: Some(if use_custom_titlebar { WindowDecorations::Client } else { WindowDecorations::Server }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(display_id, size(px(960.0), px(540.0)), cx))),
        window_min_size: Some(size(px(480.0), px(270.0))),
        ..Default::default()
    };
    _ = cx.open_window(options, |window, cx| {
        let settings_root = cx.new(|cx| {
            let sidebar_state = ResizePanelState::new(px(175.0), px(150.0), px(225.0));

            let search_state = cx.new(|cx| {
                InputState::new(window, cx).placeholder(t::common::search())
            });

            cx.subscribe(&search_state, SettingsRoot::on_search).detach();

            let mut root = SettingsRoot {
                settings: create_settings(data, window, cx),
                search_state,
                use_custom_titlebar,
                sidebar_state,
                group_list_state: ListState::new(0, ListAlignment::Top, px(100.)),
                backend_handle: data.backend_handle.clone(),
                get_configuration_task: None,
                actual_backend_config: None,
                temp_backend_config: None,
                on_receive_backend_config: OnReceiveBackendConfig::DoNothing,
                _select_file_task: Task::ready(()),
            };

            root.update_backend_configuration(cx);

            root
        });
        cx.new(|cx| Root::new(settings_root, window, cx))
    });
}

fn create_settings(data: &DataEntities, window: &mut Window, cx: &mut App) -> Settings {
    Settings {
        pages: vec![
            general::create_page(window, cx),
            appearance::create_page(data, window, cx),
            java::create_page(),
            network::create_page(),
        ].into_boxed_slice(),
        selected_page: Some(0),
        selected_group: None,
        deferred_scroll_to_group: false,
        searched_pages: None,
        last_language_id: None,
        last_query: None,
    }
}
