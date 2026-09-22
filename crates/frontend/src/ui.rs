use std::{collections::VecDeque, sync::Arc};

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, InteractiveElementExt, WindowExt, h_flex, notification::{Notification, NotificationType}, scroll::ScrollableElement, tooltip::Tooltip, v_flex
};
use rustc_hash::FxHashMap;
use schema::pandora_update::UpdatePrompt;
use serde::{Deserialize, Serialize};

use crate::{
    component::{generic_title_bar::TitleBarState, main_title_bar::MainTitleBar, menu::{MenuGroup, MenuGroupItem}, page_path::PagePath, resize_panel::{ResizePanel, ResizePanelState}, shrinking_text::ShrinkingText}, entity::{
        DataEntities, account::AccountExt, instance::{InstanceAddedEvent, InstanceEntries, InstanceEntry, InstanceModifiedEvent, InstanceMovedToTopEvent, InstanceRemovedEvent}
    }, icon::PandoraIcon, interface_config::InterfaceConfig, pages::{curseforge_page::CurseforgeSearchPage, import::ImportPage, instance::instance_page::InstancePage, instances_page::InstancesPage, modrinth_page::ModrinthSearchPage, modrinth_project_page::ModrinthProjectPage, page::Page, quickplay::QuickplayPage, skins_page::SkinsPage, syncing_page::SyncingPage}, png_render_cache,
};

pub struct LauncherUI {
    data: DataEntities,
    page: LauncherPage,
    pub update: Option<UpdatePrompt>,
    sidebar_state: ResizePanelState,
    recent_instances: heapless::Vec<InstanceEntry, 3>,
    page_history_backwards: VecDeque<(PageType, Arc<[PageType]>)>,
    page_history_forwards: Vec<(PageType, Arc<[PageType]>)>,
    previous_pages: FxHashMap<PageType, LauncherPage>,
    pending_page: Option<(PageType, Arc<[PageType]>)>,
    _instance_added_subscription: Subscription,
    _instance_modified_subscription: Subscription,
    _instance_removed_subscription: Subscription,
    _instance_moved_to_top_subscription: Subscription,
}

pub const ICONIZED_SIDEBAR_WIDTH: f32 = 64.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 150.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 225.0;

#[derive(Default, Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PageType {
    #[default]
    Quickplay,
    Instances,
    Skins,
    Modrinth {
        installing_for: Option<SharedString>,
    },
    Curseforge {
        installing_for: Option<SharedString>,
    },
    Import,
    Syncing,
    ModrinthProject {
        project_id: SharedString,
        project_title: SharedString,
        install_for: Option<SharedString>,
    },
    InstancePage {
        name: SharedString,
    },
}

impl PageType {
    pub fn title(&self, data: &DataEntities, cx: &App) -> SharedString {
        match self {
            PageType::Quickplay => t::quickplay::title().into(),
            PageType::Instances => t::instance::title().into(),
            PageType::Skins => t::skins::title().into(),
            PageType::Modrinth { installing_for } => {
                if installing_for.is_some() {
                    t::instance::content::install::from_modrinth().into()
                } else {
                    t::modrinth::name().into()
                }
            },
            PageType::Curseforge { installing_for } => {
                if installing_for.is_some() {
                    t::instance::content::install::from_curseforge().into()
                } else {
                    t::curseforge::name().into()
                }
            },
            PageType::Import => t::import::label().into(),
            PageType::Syncing => t::instance::sync::label().into(),
            PageType::ModrinthProject { project_title, .. } => project_title.clone(),
            PageType::InstancePage { name } => {
                InstanceEntries::find_title_by_name(&data.instances, name, cx)
                    .unwrap_or_else(|| name.clone())
            },
        }
    }
}

#[derive(Clone)]
pub enum LauncherPage {
    Quickplay(Entity<QuickplayPage>),
    Instances(Entity<InstancesPage>),
    Skins(Entity<SkinsPage>),
    Modrinth(Entity<ModrinthSearchPage>),
    Curseforge(Entity<CurseforgeSearchPage>),
    Import(Entity<ImportPage>),
    Syncing(Entity<SyncingPage>),
    ModrinthProject(Entity<ModrinthProjectPage>),
    InstancePage(Entity<InstancePage>),
}

impl LauncherPage {
    fn render(self, ui: &LauncherUI, window: &mut Window, cx: &mut App) -> impl IntoElement {
        fn process(entity: Entity<impl Page>, window: &mut Window, cx: &mut App) -> (bool, AnyElement, AnyElement) {
            entity.update(cx, |page, cx| {
                (page.scrollable(cx), page.controls(window, cx).into_any_element(), page.render(window, cx).into_any_element())
            })
        }

        let (scrollable, controls, page) = match self {
            LauncherPage::Quickplay(entity) => process(entity, window, cx),
            LauncherPage::Instances(entity) => process(entity, window, cx),
            LauncherPage::Skins(entity) => process(entity, window, cx),
            LauncherPage::Modrinth(entity) => process(entity, window, cx),
            LauncherPage::Curseforge(entity) => process(entity, window, cx),
            LauncherPage::Import(entity) => process(entity, window, cx),
            LauncherPage::Syncing(entity) => process(entity, window, cx),
            LauncherPage::ModrinthProject(entity) => process(entity, window, cx),
            LauncherPage::InstancePage(entity) => process(entity, window, cx),
        };

        let config = InterfaceConfig::get(cx);
        let page_path = PagePath::new(ui.data.clone(), config.main_page.clone(), config.page_path.clone());
        let title_bar = MainTitleBar {
            page_path,
            controls,
            update: ui.update.clone(),
            send: ui.data.backend_handle.clone(),
        };

        if scrollable {
            v_flex()
                .size_full()
                .child(title_bar)
                .child(div().flex_1().overflow_hidden().child(
                    v_flex().size_full().overflow_y_scrollbar().child(page),
                ))
        } else {
            v_flex()
                .size_full()
                .child(title_bar)
                .child(page)
        }
    }
}

impl LauncherUI {
    pub fn new(data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recent_instances = data
            .instances
            .read(cx)
            .entries
            .values()
            .filter_map(|entry| {
                let entry = entry.read(cx).clone();
                if entry.name == schema::quickplay::INSTANCE_NAME {
                    None
                } else {
                    Some(entry)
                }
            })
            .take(3)
            .collect();

        let _instance_added_subscription =
            cx.subscribe::<_, InstanceAddedEvent>(&data.instances, |this, _, event, cx| {
                if event.instance.name == schema::quickplay::INSTANCE_NAME {
                    return;
                }
                if this.recent_instances.is_full() {
                    this.recent_instances.pop();
                }
                let _ = this.recent_instances.insert(0, event.instance.clone());
                cx.notify();
            });
        let _instance_modified_subscription =
            cx.subscribe_in::<_, InstanceModifiedEvent>(&data.instances, window, |this, _, event, window, cx| {
                if let Some(existing) = this.recent_instances.iter_mut().find(|instance| instance.id == event.instance.id) {
                    *existing = event.instance.clone();
                    cx.notify();
                }
                if let LauncherPage::InstancePage(page) = &this.page
                    && page.read(cx).instance.read(cx).id == event.instance.id
                {
                    let page_path = InterfaceConfig::get_mut(cx).page_path.clone();
                    this.switch_page(PageType::InstancePage { name: event.instance.name.clone() }, &*page_path, window, cx);
                }
                cx.notify();
            });
        let _instance_removed_subscription =
            cx.subscribe_in::<_, InstanceRemovedEvent>(&data.instances, window, |this, _, event, window, cx| {
                this.recent_instances.retain(|entry| entry.id != event.id);

                if let LauncherPage::InstancePage(page) = &this.page
                    && page.read(cx).instance.read(cx).id == event.id
                {
                    this.switch_page(PageType::Instances, &[], window, cx);
                }
                cx.notify();
            });
        let _instance_moved_to_top_subscription =
            cx.subscribe::<_, InstanceMovedToTopEvent>(&data.instances, |this, _, event, cx| {
                if event.instance.name == schema::quickplay::INSTANCE_NAME {
                    return;
                }
                this.recent_instances.retain(|entry| entry.id != event.instance.id);
                if this.recent_instances.is_full() {
                    this.recent_instances.pop();
                }
                let _ = this.recent_instances.insert(0, event.instance.clone());
                cx.notify();
            });

        let config = InterfaceConfig::get(cx);

        let mut default_sidebar_width = config.sidebar_width;
        if default_sidebar_width <= 0.0 {
            default_sidebar_width = (MIN_SIDEBAR_WIDTH + MAX_SIDEBAR_WIDTH)/2.0;
        } else if default_sidebar_width < MIN_SIDEBAR_WIDTH {
            let min_distance = (default_sidebar_width - MIN_SIDEBAR_WIDTH).abs();
            let icon_distance = (default_sidebar_width - ICONIZED_SIDEBAR_WIDTH).abs();
            if icon_distance < min_distance {
                default_sidebar_width = ICONIZED_SIDEBAR_WIDTH;
            } else {
                default_sidebar_width = MIN_SIDEBAR_WIDTH;
            }
        }

        let sidebar_state = ResizePanelState::new(px(default_sidebar_width), px(150.0), px(225.0))
            .snap_point(px(ICONIZED_SIDEBAR_WIDTH))
            .on_resize(|width, _, cx| {
                InterfaceConfig::get_mut(cx).sidebar_width = width.as_f32();
            });

        let mut main_page = config.main_page.clone();
        let original_page_path = config.page_path.clone();

        // If quickplay is hidden, fall back to the instances page
        if !config.show_quickplay_page && main_page == PageType::Quickplay {
            main_page = PageType::Instances;
            InterfaceConfig::get_mut(cx).main_page = PageType::Instances;
        }

        // If main_page failed to deserialize, also reset the path
        if main_page == PageType::Instances {
            let config = InterfaceConfig::get_mut(cx);
            config.page_path = [].into();
        }

        let mut pending_page = None;

        let page = match Self::create_page(&data, main_page.clone(), window, cx) {
            Ok(page) => page,
            Err(page_type) => {
                pending_page = Some((main_page, original_page_path));

                let config = InterfaceConfig::get_mut(cx);
                config.main_page = page_type.clone();
                config.page_path = [].into();
                Self::create_page(&data, page_type, window, cx).unwrap()
            },
        };

        Self {
            data: data.clone(),
            page,
            update: None,
            sidebar_state,
            recent_instances,
            page_history_backwards: VecDeque::with_capacity(32),
            page_history_forwards: Vec::new(),
            previous_pages: FxHashMap::default(),
            pending_page,
            _instance_added_subscription,
            _instance_modified_subscription,
            _instance_removed_subscription,
            _instance_moved_to_top_subscription,
        }
    }

    fn create_page(data: &DataEntities, page: PageType, window: &mut Window, cx: &mut Context<Self>) -> Result<LauncherPage, PageType> {
        match page {
            PageType::Quickplay => {
                Ok(LauncherPage::Quickplay(cx.new(|cx| QuickplayPage::new(data, window, cx))))
            },
            PageType::Instances => {
                Ok(LauncherPage::Instances(cx.new(|cx| InstancesPage::new(data, window, cx))))
            },
            PageType::Skins => {
                Ok(LauncherPage::Skins(cx.new(|cx| SkinsPage::new(data, window, cx))))
            },
            PageType::Modrinth { installing_for } => {
                let installing_for = installing_for.as_ref().map(|name| InstanceEntries::find_id_by_name(&data.instances, name, cx));

                if let Some(None) = installing_for {
                    return Err(PageType::Modrinth { installing_for: None })
                }

                let page = cx.new(|cx| {
                    ModrinthSearchPage::new(installing_for.flatten(), data, window, cx)
                });
                Ok(LauncherPage::Modrinth(page))
            },
            PageType::Curseforge { installing_for } => {
                let installing_for = installing_for.as_ref().map(|name| InstanceEntries::find_id_by_name(&data.instances, name, cx));

                if let Some(None) = installing_for {
                    return Err(PageType::Curseforge { installing_for: None })
                }

                let page = cx.new(|cx| {
                    CurseforgeSearchPage::new(installing_for.flatten(), data, window, cx)
                });
                Ok(LauncherPage::Curseforge(page))
            },
            PageType::Import => {
                Ok(LauncherPage::Import(cx.new(|cx| ImportPage::new(data, window, cx))))
            },
            PageType::Syncing => {
                Ok(LauncherPage::Syncing(cx.new(|cx| SyncingPage::new(data, window, cx))))
            },
            PageType::ModrinthProject { project_id, install_for, project_title } => {
                let install_for_id = install_for.as_ref().map(|name| InstanceEntries::find_id_by_name(&data.instances, name, cx));

                if let Some(None) = install_for_id {
                    return Err(PageType::ModrinthProject { project_id, install_for: None, project_title })
                }

                let project_id = project_id.clone();
                let page = cx.new(|cx| {
                    ModrinthProjectPage::new(project_id, install_for_id.flatten(), data, window, cx,)
                });
                Ok(LauncherPage::ModrinthProject(page))
            },
            PageType::InstancePage { ref name } => {
                let Some(id) = InstanceEntries::find_id_by_name(&data.instances, name, cx) else {
                    return Err(PageType::Instances);
                };

                Ok(LauncherPage::InstancePage(cx.new(|cx| {
                    InstancePage::new(id, data, window, cx)
                })))
            },
        }
    }

    pub fn switch_page(&mut self, page: PageType, page_path: &[PageType], window: &mut Window, cx: &mut Context<Self>) {
        let page_path: Arc<[PageType]> = page_path.into();

        let config = InterfaceConfig::get(cx);
        if config.main_page == page {
            return;
        }

        self.page_history_forwards.clear();
        if self.page_history_backwards.len() >= 32 {
            self.page_history_backwards.pop_back();
        }
        self.page_history_backwards.push_front((config.main_page.clone(), config.page_path.clone()));

        self.switch_page_without_history(page, page_path, window, cx);
    }

    fn switch_page_without_history(&mut self, page: PageType, page_path: Arc<[PageType]>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_page = None;

        let config = InterfaceConfig::get_mut(cx);
        let previous_page_type = std::mem::replace(&mut config.main_page, page.clone());
        config.main_page = page.clone();
        config.page_path = page_path.clone();

        if let Some(previous_page) = self.previous_pages.remove(&page) {
            self.page = previous_page;
            self.previous_pages.retain(|k, _| page_path.contains(k));
            cx.notify();
            return;
        }

        match Self::create_page(&self.data, page, window, cx) {
            Ok(page) => {
                let previous_page = std::mem::replace(&mut self.page, page);
                if page_path.contains(&previous_page_type) {
                    self.previous_pages.insert(previous_page_type, previous_page);
                }
                self.previous_pages.retain(|k, _| page_path.contains(k));
            },
            Err(fallback) => {
                let config = InterfaceConfig::get_mut(cx);
                config.main_page = fallback.clone();
                config.page_path = [].into();
                self.previous_pages.clear();
                self.page = Self::create_page(&self.data, fallback, window, cx).unwrap();
            },
        }

        cx.notify();
    }

    pub fn nav_backwards(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((page, page_path)) = self.page_history_backwards.pop_front() else {
            return;
        };

        let config = InterfaceConfig::get(cx);
        self.page_history_forwards.push((config.main_page.clone(), config.page_path.clone()));

        self.switch_page_without_history(page, page_path, window, cx);
    }

    pub fn nav_forwards(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((page, page_path)) = self.page_history_forwards.pop() else {
            return;
        };

        let config = InterfaceConfig::get(cx);
        self.page_history_backwards.push_front((config.main_page.clone(), config.page_path.clone()));

        self.switch_page_without_history(page, page_path, window, cx);
    }
}

impl Render for LauncherUI {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(pending_page) = self.pending_page.clone() {
            if let Ok(page) = Self::create_page(&self.data, pending_page.0.clone(), window, cx) {
                self.pending_page = None;
                self.previous_pages.clear();
                self.page_history_forwards.clear();
                self.page_history_backwards.clear();

                let config = InterfaceConfig::get_mut(cx);
                config.main_page = pending_page.0.clone();
                config.page_path = pending_page.1.clone();

                self.page = page;
            }
        }

        let (page_type, show_skins, iconized_sidebar, show_sidebar_icons, show_quickplay_page, has_background) = {
            let config = InterfaceConfig::get(cx);
            (config.main_page.clone(), !config.hide_skins, config.sidebar_width == ICONIZED_SIDEBAR_WIDTH, config.show_sidebar_icons, config.show_quickplay_page, config.background_image.is_some())
        };

        let page_groups: &[(&'static str, &[(bool, &'static str, PandoraIcon, PageType)])] = &[
            ("Minecraft", &[
                (show_quickplay_page, t::quickplay::title(), PandoraIcon::Rocket, PageType::Quickplay),
                (true, t::instance::title(), PandoraIcon::Box, PageType::Instances),
                (show_skins, t::skins::title(), PandoraIcon::FaceSlightlySmiling, PageType::Skins),
            ]),
            (t::instance::content::title(), &[
                (true, t::modrinth::name(), PandoraIcon::CircleCheck, PageType::Modrinth { installing_for: None }),
                (true, t::curseforge::name(), PandoraIcon::Anvil, PageType::Curseforge { installing_for: None }),
            ]),
            (t::instance::sync::files(), &[
                (true, t::import::label(), PandoraIcon::Download, PageType::Import),
                (true, t::instance::sync::label(), PandoraIcon::RefreshCcw, PageType::Syncing),
            ]),
        ];

        let sidebar_pages = if !iconized_sidebar {
            let mut groups: heapless::Vec<MenuGroup, 4> = heapless::Vec::new();

            for (group_name, group_entries) in page_groups {
                let mut group = MenuGroup::new(*group_name);
                for (show, title, icon, page) in *group_entries {
                    if !*show {
                        continue;
                    }
                    let page = page.clone();
                    group = group.child(MenuGroupItem::new(*title)
                        .when(show_sidebar_icons, |this| this.icon(icon.clone()))
                        .active(page_type == page)
                        .on_click(cx.listener(move |launcher, _, window, cx| {
                            launcher.switch_page(page.clone(), &[], window, cx);
                        })));
                }
                let _ = groups.push(group);
            }

            if !self.recent_instances.is_empty() {
                let mut recent_instances_group = MenuGroup::new(t::instance::recent());

                for instance in &self.recent_instances {
                    let name = instance.name.clone();
                    let active = page_type == PageType::InstancePage { name: name.clone() };
                    let item = MenuGroupItem::new(name.clone())
                        .when(show_sidebar_icons, |this| {
                            let size = (window.rem_size().as_f32() * 0.875).round() as u32;
                            let size_px = px(size as f32);
                            let icon = if let Some(icon) = instance.icon.clone() {
                                let transform = png_render_cache::ImageTransformation::Resize { width: size, height: size };
                                png_render_cache::render_with_transform(icon, transform, cx)
                                    .rounded(cx.theme().radius).size(size_px).min_w(size_px).min_h(size_px).into_any_element()
                            } else {
                                let icon_path = instance.configuration.instance_fallback_icon
                                    .map(|s| s.as_str())
                                    .unwrap_or("icons/box.svg");
                                Icon::default().path(icon_path).size(size_px).min_w(size_px).min_h(size_px).into_any_element()
                            };
                            this.icon(icon)
                        })
                        .active(active)
                        .on_click(cx.listener(move |launcher, _, window, cx| {
                            launcher.switch_page(PageType::InstancePage { name: name.clone() }, &[PageType::Instances], window, cx);
                        }));
                    recent_instances_group = recent_instances_group.child(item);
                }

                let _ = groups.push(recent_instances_group);
            }

            v_flex()
                .flex_1()
                .min_h_0()
                .px_3()
                .gap_y_3()
                .children(groups)
                .overflow_y_scrollbar()
                .into_any_element()
        } else {
            let mut groups = Vec::new();

            for (_group_name, group_entries) in page_groups {
                let mut group = v_flex()
                    .items_center().min_w_full().max_w_full().w_full()
                    .min_h_0()
                    .gap_y_0p5();
                for (show, title, icon, page) in *group_entries {
                    if !*show {
                        continue;
                    }
                    let title = *title;
                    let page = page.clone();
                    group = group.child(div()
                        .id(title)
                        .p_2()
                        .rounded(cx.theme().radius)
                        .when_else(page_type == page, |this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        }, |this| {
                            this.hover(|this| {
                                this.bg(cx.theme().sidebar_accent)
                                    .text_color(cx.theme().sidebar_accent_foreground)
                            })
                        })
                        .child(icon.clone())
                        .tooltip(move |window, cx| {
                            Tooltip::new(title).build(window, cx)
                        })
                        .on_click(cx.listener(move |launcher, _, window, cx| {
                            launcher.switch_page(page.clone(), &[], window, cx);
                        })));
                }
                groups.push(group.into_any_element());
            }

            if !self.recent_instances.is_empty() {
                let mut group = v_flex()
                    .items_center().min_w_full().max_w_full().w_full()
                    .min_h_0()
                    .gap_y_0p5();

                for instance in &self.recent_instances {
                    let name = instance.name.clone();
                    let active = page_type == PageType::InstancePage { name: name.clone() };

                    let size = window.rem_size().as_f32().round() as u32;
                    let size_px = px(size as f32);
                    let icon = if let Some(icon) = instance.icon.clone() {
                        let transform = png_render_cache::ImageTransformation::Resize { width: size, height: size };
                        png_render_cache::render_with_transform(icon, transform, cx)
                            .rounded(cx.theme().radius).size(size_px).min_w(size_px).min_h(size_px).into_any_element()
                    } else {
                        let icon_path = instance.configuration.instance_fallback_icon
                            .map(|s| s.as_str())
                            .unwrap_or("icons/box.svg");
                        Icon::default().path(icon_path).size(size_px).min_w(size_px).min_h(size_px).into_any_element()
                    };

                    group = group.child(div()
                        .id(name.clone())
                        .p_2()
                        .rounded(cx.theme().radius)
                        .when_else(active, |this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        }, |this| {
                            this.hover(|this| {
                                this.bg(cx.theme().sidebar_accent)
                                    .text_color(cx.theme().sidebar_accent_foreground)
                            })
                        })
                        .child(icon)
                        .tooltip({
                            let name = name.clone();
                            move |window, cx| {
                                Tooltip::new(name.clone()).build(window, cx)
                            }
                        })
                        .on_click(cx.listener(move |launcher, _, window, cx| {
                            launcher.switch_page(PageType::InstancePage { name: name.clone() }, &[PageType::Instances], window, cx);
                        })));
                }
                groups.push(group.into_any_element());
            }

            v_flex()
                .items_center().min_w_full().max_w_full().w_full()
                .min_h_0()
                .pt_1()
                .px_2()
                .gap_y_3()
                .children(groups)
                .overflow_y_scrollbar()
                .into_any_element()
        };


        let accounts = self.data.accounts.read(cx);
        let (account_head, account_name) = if let Some(account) = &accounts.selected_account {
            let account_name = account.username(InterfaceConfig::get(cx).hide_usernames);
            let hide_skins = InterfaceConfig::get(cx).hide_skins;

            let head = if hide_skins {
                gpui::img(ImageSource::Resource(Resource::Embedded("images/hidden_head.png".into())))
            } else if let Some(head) = &account.head {
                let resize = png_render_cache::ImageTransformation::Resize { width: 32, height: 32 };
                png_render_cache::render_with_transform(head.clone(), resize, cx)
            } else {
                gpui::img(ImageSource::Resource(Resource::Embedded("images/default_head.png".into())))
            };
            (head, account_name)
        } else {
            (
                gpui::img(ImageSource::Resource(Resource::Embedded("images/default_head.png".into()))),
                t::account::none().into(),
            )
        };

        let account_button = h_flex()
            .id("account-button")
            .flex_1()
            .p_2()
            .max_w_full()
            .gap_2()
            .justify_center()
            .text_size(rems(0.9375))
            .line_height(rems(1.0))
            .rounded(cx.theme().radius)
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .child(account_head.size_8().min_w_8().min_h_8())
            .when(!iconized_sidebar, |this| this.child(ShrinkingText::new(account_name)))
            .on_click({
                let data = self.data.clone();
                move |_, window, cx| {
                    if data.accounts.read(cx).accounts.is_empty() {
                        crate::root::start_new_account_login(&data.backend_handle, window, cx);
                        return;
                    }

                    let build = crate::modals::accounts::build_accounts_sheet(&data, window, cx);
                    window.open_sheet_at(gpui_component::Placement::Left, cx, build);
                }
            });

        let settings_button = div()
            .id("settings-button")
            .p_2()
            .rounded(cx.theme().radius)
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .child(PandoraIcon::Settings)
            .on_click({
                let data = self.data.clone();
                move |_, window, cx| {
                    crate::settings::open_settings_window(window, &data, cx);
                }
            });
        let bug_report_button = div()
            .id("bug-report-button")
            .p_2()
            .rounded(cx.theme().radius)
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .child(PandoraIcon::Bug)
            .tooltip(move |window, cx| {
                Tooltip::new(t::system::report_bug()).build(window, cx)
            })
            .on_click({
                move |_, window, cx| {
                    open_bug_report_url(window, cx);
                }
            });
        let discord_invite = option_env!("DISCORD_INVITE").unwrap_or("https://pandora.moulberry.com/discord");
        let discord_button = div()
            .id("discord-button")
            .p_2()
            .rounded(cx.theme().radius)
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .child(PandoraIcon::Discord)
            .tooltip(move |window, cx| {
                Tooltip::new(t::system::join_discord()).build(window, cx)
            })
            .on_click({
                move |_, _, cx| {
                    cx.open_url(discord_invite);
                }
            });

        let header_drag_state = window.use_keyed_state("sidebar-header-drag-state", cx, |_, _| TitleBarState::default());
        let header = h_flex()
            .id("sidebar-header")
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down_out(window.listener_for(&header_drag_state, |state, _, _, _| {
                state.should_move = false;
            }))
            .when(cfg!(target_os = "linux"), |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .when(cfg!(target_os = "macos"), |this| {
                this.on_double_click(|_, window, _| window.titlebar_double_click())
            })
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&header_drag_state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&header_drag_state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&header_drag_state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    window.start_window_move();
                }
            }))
            .when_else(cfg!(target_os = "macos"), |this| this.pt(px(41.0)), |this| this.pt(px(14.0)))
            .px_5()
            .pb_2()
            .gap_2()
            .w_full()
            .justify_center()
            .text_size(rems(0.9375))
            .child(Icon::new(PandoraIcon::Pandora).size_8().min_w_8().min_h_8())
            .when(!iconized_sidebar, |this| this.child(t::common::app_name()));
        let footer_buttons = h_flex()
            .when(iconized_sidebar, |this| this.flex_col())
            .child(settings_button)
            .child(bug_report_button)
            .when(!discord_invite.is_empty(), |this| this.child(discord_button));
        let footer = v_flex().pb_2().px_2().items_center().min_w_full().max_w_full().w_full().child(footer_buttons).child(account_button);
        let sidebar = v_flex()
            .size_full()
            .min_size_full()
            .max_size_full()
            .bg(if has_background { cx.theme().sidebar.opacity(0.66) } else { cx.theme().sidebar })
            .text_color(cx.theme().sidebar_foreground)
            .child(header)
            .child(sidebar_pages)
            .child(footer);

        ResizePanel::new(&self.sidebar_state, sidebar, self.page.clone().render(&self, window, cx))
    }
}

fn open_bug_report_url(window: &mut Window, cx: &mut App) {
    let mut body = String::from(r#"## Description of bug
(Write here)

## Steps to reproduce
(Write here)

## This issue is unique
- [ ] I've searched the other issues and didn't see an issue describing the same bug

## Environment
"#);

    use std::fmt::Write;
    _ = writeln!(&mut body, "Version: {:?}", option_env!("PANDORA_RELEASE_VERSION"));
    _ = writeln!(&mut body, "Distributor: {:?}", option_env!("PANDORA_DISTRIBUTION"));
    _ = writeln!(&mut body, "OS: {} ({})", std::env::consts::OS, std::env::consts::ARCH);

    if cfg!(target_os = "linux") {
        if let Ok(os_release) = std::fs::read_to_string("/etc/os-release") {
            for line in os_release.lines() {
                let line = line.trim_ascii();
                if let Some(name) = line.strip_prefix("NAME=") {
                    _ = writeln!(&mut body, "OS Name: {}", name);
                } else if let Some(version) = line.strip_prefix("VERSION=") {
                    _ = writeln!(&mut body, "OS Version: {}", version);
                }
            }
        }

        _ = writeln!(&mut body, "Desktop: {:?}", std::env::var_os("XDG_CURRENT_DESKTOP"));

        if let Some(snap_name) = std::env::var_os("SNAP_NAME") {
            _ = writeln!(&mut body, "Snap: {:?}", snap_name);
        }
        if let Some(snap_name) = std::env::var_os("FLATPAK_ID") {
            _ = writeln!(&mut body, "Flatpak ID: {:?}", snap_name);
        }
        if std::env::var_os("APPIMAGE").is_some() {
            body.push_str("AppImage: true\n");
        }
    }

    let Some(github) = option_env!("GITHUB_REPOSITORY_URL") else {
        let mut notification: Notification = (NotificationType::Error, SharedString::from("Unable to report bug, GITHUB_REPOSITORY_URL was not set at compile time")).into();
        notification = notification.autohide(false);
        window.push_notification(notification, cx);
        return;
    };

    cx.open_url(&format!("{}/issues/new?body={}", github, urlencoding::encode(&body)));
}
