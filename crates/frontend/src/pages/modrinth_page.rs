use std::{ops::Range, sync::{atomic::AtomicBool, Arc}, time::Duration};

use bridge::{instance::{AtomicContentUpdateStatus, ContentUpdateStatus, InstanceID, InstanceContentID, InstanceContentSummary}, message::MessageToBackend, meta::MetadataRequest, modal_action::ModalAction};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Selectable, StyledExt, WindowExt, breadcrumb::Breadcrumb, button::{Button, ButtonGroup, ButtonVariant, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState}, notification::NotificationType, scroll::{ScrollableElement, Scrollbar}, skeleton::Skeleton, tooltip::Tooltip, v_flex
};
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{content::ContentSource, loader::Loader, modrinth::{
    ModrinthHit, ModrinthProjectType, ModrinthSearchRequest, ModrinthSearchResult, ModrinthSideRequirement
}};

use crate::{
    component::{error_alert::ErrorAlert, page_path::PagePath}, entity::{
        DataEntities, instance::InstanceEntries, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult}
    }, interface_config::InterfaceConfig, ts, ts_short, ui
};

pub struct ModrinthSearchPage {
    data: DataEntities,
    hits: Vec<ModrinthHit>,
    page_path: PagePath,
    install_for: Option<InstanceID>,
    loading: Option<Subscription>,
    pending_clear: bool,
    total_hits: usize,
    search_state: Entity<InputState>,
    _search_input_subscription: Subscription,
    _delayed_clear_task: Task<()>,
    filter_project_type: ModrinthProjectType,
    filter_loaders: FxHashSet<Loader>,
    filter_categories: FxHashSet<&'static str>,
    show_categories: Arc<AtomicBool>,
    can_install_latest: bool,
    installed_mods_by_project: FxHashMap<Arc<str>, Vec<InstalledMod>>,
    last_search: Arc<str>,
    scroll_handle: UniformListScrollHandle,
    search_error: Option<SharedString>,
    image_cache: Entity<RetainAllImageCache>,
}

struct InstalledMod {
    mod_id: InstanceContentID,
    status: Arc<AtomicContentUpdateStatus>,
}

impl ModrinthSearchPage {
    pub fn new(install_for: Option<InstanceID>, project_type: Option<ModrinthProjectType>, page_path: PagePath, data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search mods...").clean_on_escape());

        let mut can_install_latest = false;
        let mut installed_mods_by_project: FxHashMap<Arc<str>, Vec<InstalledMod>> = FxHashMap::default();

        if let Some(install_for) = install_for {
            if let Some(entry) = data.instances.read(cx).entries.get(&install_for) {
                let instance = entry.read(cx);
                can_install_latest = instance.configuration.loader != Loader::Vanilla;

                let mods = instance.mods.read(cx);
                for summary in mods.iter() {
                    let ContentSource::ModrinthProject { project } = &summary.content_source else {
                        continue;
                    };

                    let installed = installed_mods_by_project.entry(project.clone()).or_default();
                    installed.push(InstalledMod {
                        mod_id: summary.id,
                        status: summary.content_summary.update_status.clone(),
                    })
                }
            }
        }

        let _search_input_subscription = cx.subscribe_in(&search_state, window, Self::on_search_input_event);

        let mut filter_project_type = if let Some(project_type) = project_type {
            InterfaceConfig::get_mut(cx).modrinth_page_project_type = project_type;
            project_type
        } else {
            InterfaceConfig::get(cx).modrinth_page_project_type
        };
        if filter_project_type == ModrinthProjectType::Other {
            filter_project_type = ModrinthProjectType::Mod;
        }

        let mut page = Self {
            data: data.clone(),
            hits: Vec::new(),
            page_path,
            install_for,
            loading: None,
            pending_clear: false,
            total_hits: 1,
            search_state,
            _search_input_subscription,
            _delayed_clear_task: Task::ready(()),
            filter_project_type,
            filter_loaders: FxHashSet::default(),
            filter_categories: FxHashSet::default(),
            show_categories: Arc::new(AtomicBool::new(false)),
            can_install_latest,
            installed_mods_by_project,
            last_search: Arc::from(""),
            scroll_handle: UniformListScrollHandle::new(),
            search_error: None,
            image_cache: RetainAllImageCache::new(cx),
        };
        page.load_more(cx);
        page
    }

    fn on_search_input_event(
        &mut self,
        state: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let InputEvent::Change = event else {
            return;
        };

        let search = state.read(cx).text().to_string();
        let search = search.trim();

        if &*self.last_search == search {
            return;
        }

        let search: Arc<str> = Arc::from(search);
        self.last_search = search.clone();
        self.reload(cx);
    }

    fn set_project_type(&mut self, project_type: ModrinthProjectType, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_project_type == project_type {
            return;
        }
        InterfaceConfig::get_mut(cx).modrinth_page_project_type = project_type;
        self.filter_project_type = project_type;
        self.filter_categories.clear();
        self.search_state.update(cx, |state, cx| {
            let placeholder = match project_type {
                ModrinthProjectType::Mod => "Search mods...",
                ModrinthProjectType::Modpack => "Search modpacks...",
                ModrinthProjectType::Resourcepack => "Search resourcepacks...",
                ModrinthProjectType::Shader => "Search shaders...",
                ModrinthProjectType::Other => "Search...",
            };
            state.set_placeholder(placeholder, window, cx)
        });
        self.reload(cx);
    }

    fn set_filter_loaders(&mut self, loaders: FxHashSet<Loader>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_loaders == loaders {
            return;
        }
        self.filter_loaders = loaders;
        self.reload(cx);
    }

    fn set_filter_categories(&mut self, categories: FxHashSet<&'static str>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_categories == categories {
            return;
        }
        self.filter_categories = categories;
        self.reload(cx);
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.pending_clear = true;
        self.loading = None;

        self._delayed_clear_task = cx.spawn(async |page, cx| {
            gpui::Timer::after(Duration::from_millis(300)).await;
            let _ = page.update(cx, |page, cx| {
                if page.pending_clear {
                    page.pending_clear = false;
                    page.hits.clear();
                    page.total_hits = 1;
                    cx.notify();
                }
            });
        });

        self.load_more(cx);
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading.is_some() {
            return;
        }
        self.search_error = None;

        let query = if self.last_search.is_empty() {
            None
        } else {
            Some(self.last_search.clone())
        };

        let project_type = match self.filter_project_type {
            ModrinthProjectType::Mod | ModrinthProjectType::Other => "mod",
            ModrinthProjectType::Modpack => "modpack",
            ModrinthProjectType::Resourcepack => "resourcepack",
            ModrinthProjectType::Shader => "shader",
        };

        let offset = if self.pending_clear { 0 } else { self.hits.len() };

        let mut facets = format!("[[\"project_type={}\"]", project_type);

        let is_mod = self.filter_project_type == ModrinthProjectType::Mod || self.filter_project_type == ModrinthProjectType::Modpack;
        let is_mod_or_shader = is_mod || self.filter_project_type == ModrinthProjectType::Shader;
        let filter_by_instance = self.install_for.is_some() && is_mod_or_shader;

        let instance_version_and_loader = self.install_for.and_then(|id| {
            self.data.instances.read(cx).entries.get(&id).map(|entry| {
                let inst = entry.read(cx);
                (inst.configuration.minecraft_version.clone(), inst.configuration.loader)
            })
        });

        if is_mod {
            let loaders_to_use: Vec<Loader> = if filter_by_instance {
                if let Some((_, loader)) = &instance_version_and_loader
                    && *loader != Loader::Vanilla
                {
                    vec![*loader]
                } else {
                    self.filter_loaders.iter().copied().collect()
                }
            } else if !self.filter_loaders.is_empty() {
                self.filter_loaders.iter().copied().collect()
            } else {
                Vec::new()
            };
            if !loaders_to_use.is_empty() {
                facets.push_str(",[");
                for (i, loader) in loaders_to_use.iter().enumerate() {
                    if i > 0 {
                        facets.push(',');
                    }
                    facets.push_str("\"categories:");
                    facets.push_str(loader.as_modrinth_loader().id());
                    facets.push('"');
                }
                facets.push_str("]");
            }
        }

        if filter_by_instance {
            if let Some((minecraft_version, _)) = &instance_version_and_loader {
                facets.push_str(",[\"versions:");
                facets.push_str(minecraft_version.as_str());
                facets.push_str("\"]");
            }
        }

        if !self.filter_categories.is_empty() {
            facets.push_str(",[");

            let mut first = true;
            for category in &self.filter_categories {
                if first {
                    first = false;
                } else {
                    facets.push(',');
                }
                facets.push_str("\"categories:");
                facets.push_str(*category);
                facets.push('"');
            }
            facets.push(']');
        }

        facets.push(']');

        let request = ModrinthSearchRequest {
            query,
            facets: Some(facets.into()),
            index: schema::modrinth::ModrinthSearchIndex::Relevance,
            offset,
            limit: 20,
        };

        let data = FrontendMetadata::request(&self.data.metadata, MetadataRequest::ModrinthSearch(request), cx);

        let result: FrontendMetadataResult<ModrinthSearchResult> = data.read(cx).result();
        match result {
            FrontendMetadataResult::Loading => {
                let subscription = cx.observe(&data, |page, data, cx| {
                    let result: FrontendMetadataResult<ModrinthSearchResult> = data.read(cx).result();
                    match result {
                        FrontendMetadataResult::Loading => {},
                        FrontendMetadataResult::Loaded(result) => {
                            page.apply_search_data(result);
                            page.loading = None;
                            cx.notify();
                        },
                        FrontendMetadataResult::Error(shared_string) => {
                            page.search_error = Some(shared_string);
                            page.loading = None;
                            cx.notify();
                        },
                    }
                });
                self.loading = Some(subscription);
            },
            FrontendMetadataResult::Loaded(result) => {
                self.apply_search_data(result);
            },
            FrontendMetadataResult::Error(shared_string) => {
                self.search_error = Some(shared_string);
            },
        }
    }

    fn apply_search_data(&mut self, search_result: &ModrinthSearchResult) {
        if self.pending_clear {
            self.pending_clear = false;
            self.hits.clear();
            self.total_hits = 1;
            self._delayed_clear_task = Task::ready(());
        }

        self.hits.extend(search_result.hits.iter().map(|hit| {
            let mut hit = hit.clone();
            if let Some(description) = hit.description {
                hit.description = Some(description.replace("\n", " ").into());
            }
            hit
        }));
        self.total_hits = search_result.total_hits;
    }

    fn render_items(&mut self, visible_range: Range<usize>, _window: &mut Window, cx: &mut Context<Self>) -> Vec<Div> {
        let theme = cx.theme();
        let mut should_load_more = false;
        let items = visible_range
            .map(|index| {
                let Some(hit) = self.hits.get(index) else {
                    if let Some(search_error) = self.search_error.clone() {
                        return div()
                            .pl_3()
                            .pt_3()
                            .child(ErrorAlert::new("search_error", "Error requesting from Modrinth".into(), search_error));
                    } else {
                        should_load_more = true;
                        return div()
                            .pl_3()
                            .pt_3()
                            .child(Skeleton::new().w_full().h(px(28.0 * 4.0)).rounded_lg());
                    }
                };

                let image = if let Some(icon_url) = &hit.icon_url
                    && !icon_url.is_empty()
                {
                    gpui::img(SharedUri::from(icon_url))
                        .with_fallback(|| Skeleton::new().rounded_lg().size_16().into_any_element())
                } else {
                    gpui::img(ImageSource::Resource(Resource::Embedded(
                        "images/default_mod.png".into(),
                    )))
                };

                let name = hit
                    .title
                    .as_ref()
                    .map(Arc::clone)
                    .map(SharedString::new)
                    .unwrap_or(SharedString::new_static("Unnamed"));
                let author = format!("by {}", hit.author.clone());
                let description = hit
                    .description
                    .as_ref()
                    .map(Arc::clone)
                    .map(SharedString::new)
                    .unwrap_or(SharedString::new_static("No Description"));

                const GRAY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 };
                let author_line = div().text_color(GRAY).text_sm().pb_px().child(author);

                let client_side = hit.client_side.unwrap_or(ModrinthSideRequirement::Unknown);
                let server_side = hit.server_side.unwrap_or(ModrinthSideRequirement::Unknown);

                let (env_icon, env_name) = match (client_side, server_side) {
                    (ModrinthSideRequirement::Required, ModrinthSideRequirement::Required) => {
                        (Icon::empty().path("icons/globe.svg"), ts!("client_and_server"))
                    },
                    (ModrinthSideRequirement::Required, ModrinthSideRequirement::Unsupported) => {
                        (Icon::empty().path("icons/computer.svg"), ts!("client_only"))
                    },
                    (ModrinthSideRequirement::Required, ModrinthSideRequirement::Optional) => {
                        (Icon::empty().path("icons/computer.svg"), ts!("client_only_server_optional"))
                    },
                    (ModrinthSideRequirement::Unsupported, ModrinthSideRequirement::Required) => {
                        (Icon::empty().path("icons/router.svg"), ts!("server_only"))
                    },
                    (ModrinthSideRequirement::Optional, ModrinthSideRequirement::Required) => {
                        (Icon::empty().path("icons/router.svg"), ts!("server_only_client_optional"))
                    },
                    (ModrinthSideRequirement::Optional, ModrinthSideRequirement::Optional) => {
                        (Icon::empty().path("icons/globe.svg"), ts!("client_or_server"))
                    },
                    _ => (Icon::empty().path("icons/cpu.svg"), ts!("unknown_environment")),
                };

                let environment = h_flex().gap_1().font_bold().child(env_icon).child(env_name);

                let categories = hit.display_categories.iter().flat_map(|categories| {
                    categories.iter().map(|category| {
                        let icon = icon_for(category).unwrap_or("icons/diamond.svg");
                        let icon = Icon::empty().path(icon);
                        let translated_category = ts!(category.as_str());
                        h_flex().gap_0p5().child(icon).child(translated_category)
                    })
                });

                let download_icon = Icon::empty().path("icons/download.svg");
                let downloads = h_flex()
                    .gap_0p5()
                    .child(download_icon.clone())
                    .child(format_downloads(hit.downloads));

                let primary_action = self.get_primary_action(&hit.project_id, cx);

                let buttons = ButtonGroup::new(("buttons", index))
                    .layout(Axis::Vertical)
                    .child(
                        Button::new(("install", index))
                            .label(primary_action.text())
                            .icon(primary_action.icon())
                            .with_variant(primary_action.button_variant())
                            .on_click({
                                let data = self.data.clone();
                                let name = name.clone();
                                let project_id = hit.project_id.clone();
                                let install_for = self.install_for.clone();
                                let project_type = hit.project_type;

                                move |_, window, cx| {
                                    if project_type != ModrinthProjectType::Other {
                                        match primary_action {
                                            PrimaryAction::Install | PrimaryAction::Reinstall => {
                                                crate::modals::modrinth_install::open(
                                                    name.as_str(),
                                                    project_id.clone(),
                                                    project_type,
                                                    install_for,
                                                    &data,
                                                    window,
                                                    cx
                                                );
                                            },
                                            PrimaryAction::InstallLatest => {
                                                crate::modals::modrinth_install_auto::open(
                                                    name.as_str(),
                                                    project_id.clone(),
                                                    project_type,
                                                    install_for.unwrap(),
                                                    &data,
                                                    window,
                                                    cx
                                                );
                                            },
                                            PrimaryAction::CheckForUpdates => {
                                                let modal_action = ModalAction::default();
                                                data.backend_handle.send(MessageToBackend::UpdateCheck {
                                                    instance: install_for.unwrap(),
                                                    modal_action: modal_action.clone()
                                                });
                                                crate::modals::generic::show_notification(window, cx,
                                                    "Error checking for updates".into(), modal_action);
                                            },
                                            PrimaryAction::ErrorCheckingForUpdates => {},
                                            PrimaryAction::UpToDate => {},
                                            PrimaryAction::Update(ref ids) => {
                                                for id in ids {
                                                    let modal_action = ModalAction::default();
                                                    data.backend_handle.send(MessageToBackend::UpdateContent {
                                                        instance: install_for.unwrap(),
                                                        content_id: *id,
                                                        modal_action: modal_action.clone()
                                                    });
                                                    crate::modals::generic::show_notification(window, cx,
                                                        "Error updating mod".into(), modal_action);
                                                }

                                            },
                                        }
                                    } else {
                                        window.push_notification(
                                            (
                                                NotificationType::Error,
                                                "Don't know how to handle this type of content",
                                            ),
                                            cx,
                                        );
                                    }
                                }
                            }),
                    )
                    .child(
                        Button::new(("open", index))
                            .label("Open Page")
                            .icon(IconName::Globe)
                            .info()
                            .on_click({
                                let project_type = hit.project_type.as_str();
                                let project_id = hit.project_id.clone();
                                move |_, _, cx| {
                                    cx.open_url(&format!(
                                        "https://modrinth.com/{}/{}",
                                        project_type, project_id
                                    ));
                                }
                            }),
                    );

                let item = h_flex()
                    .rounded_lg()
                    .px_4()
                    .py_2()
                    .gap_4()
                    .h_32()
                    .bg(theme.background)
                    .border_color(theme.border)
                    .border_1()
                    .size_full()
                    .child(image.rounded_lg().size_16().min_w_16().min_h_16())
                    .child(
                        v_flex()
                            .h(px(104.0))
                            .flex_grow()
                            .gap_1()
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_end()
                                    .line_clamp(1)
                                    .text_lg()
                                    .child(name)
                                    .child(author_line),
                            )
                            .child(
                                div()
                                    .flex_auto()
                                    .line_height(px(20.0))
                                    .line_clamp(2)
                                    .child(description),
                            )
                            .child(
                                h_flex()
                                    .gap_2p5()
                                    .children(std::iter::once(environment).chain(categories)),
                            ),
                    )
                    .child(v_flex().gap_2().child(downloads).child(buttons));

                div().pl_3().pt_3().child(item)
            })
            .collect();

        if should_load_more {
            self.load_more(cx);
        }

        items
    }

    fn get_primary_action(&self, project_id: &str, cx: &App) -> PrimaryAction {
        let install_latest = self.can_install_latest && !InterfaceConfig::get(cx).modrinth_install_normally;

        let installed = self.installed_mods_by_project.get(project_id);

        if let Some(installed) = installed && !installed.is_empty() {
            if !install_latest {
                return PrimaryAction::Reinstall;
            }

            let mut action = PrimaryAction::CheckForUpdates;
            for installed_mod in installed {
                match installed_mod.status.load(std::sync::atomic::Ordering::Relaxed) {
                    ContentUpdateStatus::Unknown => {},
                    ContentUpdateStatus::AlreadyUpToDate => {
                        if !matches!(action, PrimaryAction::Update(..)) {
                            action = PrimaryAction::UpToDate;
                        }
                    },
                    ContentUpdateStatus::Modrinth => {
                        if let PrimaryAction::Update(vec) = &mut action {
                            vec.push(installed_mod.mod_id);
                        } else {
                            action = PrimaryAction::Update(vec![installed_mod.mod_id]);
                        }
                    },
                    _ => {
                        if action == PrimaryAction::CheckForUpdates {
                            action = PrimaryAction::ErrorCheckingForUpdates;
                        }
                    }
                };
            }
            return action;
        }

        if install_latest {
            PrimaryAction::InstallLatest
        } else {
            PrimaryAction::Install
        }
    }
}

#[derive(PartialEq, Eq)]
enum PrimaryAction {
    Install,
    Reinstall,
    InstallLatest,
    CheckForUpdates,
    ErrorCheckingForUpdates,
    UpToDate,
    Update(Vec<InstanceContentID>),
}

impl PrimaryAction {
    pub fn text(&self) -> &'static str {
        match self {
            PrimaryAction::Install => "Install",
            PrimaryAction::Reinstall => "Reinstall",
            PrimaryAction::InstallLatest => "Install Latest",
            PrimaryAction::CheckForUpdates => "Update Check",
            PrimaryAction::ErrorCheckingForUpdates => "Error",
            PrimaryAction::UpToDate => "Up-to-date",
            PrimaryAction::Update(..) => "Update",
        }
    }

    pub fn icon(&self) -> Icon {
        match self {
            PrimaryAction::Install => Icon::empty().path("icons/download.svg"),
            PrimaryAction::Reinstall => Icon::empty().path("icons/download.svg"),
            PrimaryAction::InstallLatest => Icon::empty().path("icons/download.svg"),
            PrimaryAction::CheckForUpdates => Icon::default().path("icons/refresh-ccw.svg"),
            PrimaryAction::ErrorCheckingForUpdates => Icon::default().path("icons/triangle-alert.svg"),
            PrimaryAction::UpToDate => Icon::default().path("icons/check.svg"),
            PrimaryAction::Update(..) => Icon::empty().path("icons/download.svg"),
        }
    }

    pub fn button_variant(&self) -> ButtonVariant {
        match self {
            PrimaryAction::Install => ButtonVariant::Success,
            PrimaryAction::Reinstall => ButtonVariant::Success,
            PrimaryAction::InstallLatest => ButtonVariant::Success,
            PrimaryAction::CheckForUpdates => ButtonVariant::Warning,
            PrimaryAction::ErrorCheckingForUpdates => ButtonVariant::Danger,
            PrimaryAction::UpToDate => ButtonVariant::Secondary,
            PrimaryAction::Update(..) => ButtonVariant::Success,
        }
    }
}

impl Render for ModrinthSearchPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_load_more = self.total_hits > self.hits.len();
        let scroll_handle = self.scroll_handle.clone();

        let item_count = self.hits.len() + if can_load_more || self.search_error.is_some() { 1 } else { 0 };

        let list = h_flex()
            .image_cache(self.image_cache.clone())
            .size_full()
            .overflow_y_hidden()
            .child(
                uniform_list(
                    "uniform-list",
                    item_count,
                    cx.processor(Self::render_items),
                )
                .size_full()
                .track_scroll(&scroll_handle),
            )
            .child(
                div()
                    .w_3()
                    .h_full()
                    .py_3()
                    .child(Scrollbar::vertical(&scroll_handle)),
            );

        let mut top_bar = h_flex()
            .w_full()
            .gap_3()
            .child(Input::new(&self.search_state));


        if self.can_install_latest {
            let tooltip = |window: &mut Window, cx: &mut App| {
                Tooltip::new(SharedString::new_static("Always install the latest version. Untick to be able to choose older versions of content to install")).build(window, cx)
            };

            let install_latest = !InterfaceConfig::get(cx).modrinth_install_normally;
            top_bar = top_bar.child(Checkbox::new("install-latest")
                .label("Install Latest")
                .tooltip(tooltip)
                .checked(install_latest)
                .on_click({
                    move |value, _, cx| {
                        InterfaceConfig::get_mut(cx).modrinth_install_normally = !*value;
                    }
                })
            );
        }

        let theme = cx.theme();
        let content = v_flex()
            .size_full()
            .gap_3()
            .child(top_bar)
            .child(div().size_full().rounded_lg().border_1().border_color(theme.border).child(list));

        let type_button_group = ButtonGroup::new("type")
            .layout(Axis::Vertical)
            .outline()
            .child(Button::new("mods").label("Mods").selected(self.filter_project_type == ModrinthProjectType::Mod))
            .child(
                Button::new("modpacks")
                    .label("Modpacks")
                    .selected(self.filter_project_type == ModrinthProjectType::Modpack),
            )
            .child(
                Button::new("resourcepacks")
                    .label("Resourcepacks")
                    .selected(self.filter_project_type == ModrinthProjectType::Resourcepack),
            )
            .child(Button::new("shaders").label("Shaders").selected(self.filter_project_type == ModrinthProjectType::Shader))
            .on_click(cx.listener(|page, clicked: &Vec<usize>, window, cx| match clicked[0] {
                0 => page.set_project_type(ModrinthProjectType::Mod, window, cx),
                1 => page.set_project_type(ModrinthProjectType::Modpack, window, cx),
                2 => page.set_project_type(ModrinthProjectType::Resourcepack, window, cx),
                3 => page.set_project_type(ModrinthProjectType::Shader, window, cx),
                _ => {},
            }));

        let loader_button_group = if self.filter_project_type == ModrinthProjectType::Mod || self.filter_project_type == ModrinthProjectType::Modpack {
            Some(ButtonGroup::new("loader_group")
                .layout(Axis::Vertical)
                .outline()
                .multiple(true)
                .child(Button::new("fabric").label("Fabric").selected(self.filter_loaders.contains(&Loader::Fabric)))
                .child(Button::new("forge").label("Forge").selected(self.filter_loaders.contains(&Loader::Forge)))
                .child(Button::new("neoforge").label("NeoForge").selected(self.filter_loaders.contains(&Loader::NeoForge)))
                .on_click(cx.listener(|page, clicked: &Vec<usize>, window, cx| {
                    page.set_filter_loaders(clicked.iter().filter_map(|index| match index {
                        0 => Some(Loader::Fabric),
                        1 => Some(Loader::Forge),
                        2 => Some(Loader::NeoForge),
                        _ => None
                    }).collect(), window, cx);
                })))
        } else {
            None
        };

        let categories = match self.filter_project_type {
            ModrinthProjectType::Mod => FILTER_MOD_CATEGORIES,
            ModrinthProjectType::Modpack => FILTER_MODPACK_CATEGORIES,
            ModrinthProjectType::Resourcepack => FILTER_RESOURCEPACK_CATEGORIES,
            ModrinthProjectType::Shader => FILTER_SHADERPACK_CATEGORIES,
            ModrinthProjectType::Other => &[],
        };

        let category = if self.show_categories.load(std::sync::atomic::Ordering::Relaxed) {
            ButtonGroup::new("category_group")
                .layout(Axis::Vertical)
                .outline()
                .multiple(true)
                .children(categories.iter().map(|id| {
                    Button::new(*id)
                        .label(ts_short!(id))
                        .when_some(icon_for(id), |this, icon| {
                            this.icon(Icon::empty().path(icon))
                        })
                        .selected(self.filter_categories.contains(id)
                    )
                }))
                .on_click(cx.listener(|page, clicked: &Vec<usize>, window, cx| {
                    page.set_filter_categories(clicked.iter().filter_map(|index| categories.get(*index).map(|s| *s)).collect(), window, cx);
                })).into_any_element()
        } else {
            let show_categories = self.show_categories.clone();
            Button::new("show-categories").icon(IconName::ArrowDown).label("Categories").outline().on_click(move |_, _, _| {
                show_categories.store(true, std::sync::atomic::Ordering::Relaxed);
            }).into_any_element()
        };

        let parameters = h_flex()
            .h_full()
            .min_h_0()
            .flex_1()
            .overflow_y_scrollbar()
            .child(v_flex().h_full().gap_3()
                .child(type_button_group)
                .when_some(loader_button_group, |this, group| this.child(group))
                .child(category)
            );

        ui::page(cx, self.page_path.create_breadcrumb(&self.data, cx))
            .child(h_flex().flex_1().min_h_0().size_full().p_3().gap_3().child(parameters).child(content))
    }
}

fn format_downloads(downloads: usize) -> String {
    if downloads >= 1_000_000_000 {
        format!("{}B Downloads", (downloads / 10_000_000) as f64 / 100.0)
    } else if downloads >= 1_000_000 {
        format!("{}M Downloads", (downloads / 10_000) as f64 / 100.0)
    } else if downloads >= 10_000 {
        format!("{}K Downloads", (downloads / 10) as f64 / 100.0)
    } else {
        format!("{} Downloads", downloads)
    }
}

fn icon_for(str: &str) -> Option<&'static str> {
    match str {
        "forge" => Some("icons/anvil.svg"),
        "fabric" => Some("icons/scroll.svg"),
        "neoforge" => Some("icons/cat.svg"),
        "quilt" => Some("icons/grid-2x2.svg"),
        "adventure" => Some("icons/compass.svg"),
        "cursed" => Some("icons/bug.svg"),
        "decoration" => Some("icons/house.svg"),
        "economy" => Some("icons/dollar-sign.svg"),
        "equipment" | "combat" => Some("icons/swords.svg"),
        "food" => Some("icons/carrot.svg"),
        "game-mechanics" => Some("icons/sliders-vertical.svg"),
        "library" | "items" => Some("icons/book.svg"),
        "magic" => Some("icons/wand.svg"),
        "management" => Some("icons/server.svg"),
        "minigame" => Some("icons/award.svg"),
        "mobs" | "entities" => Some("icons/cat.svg"),
        "optimization" => Some("icons/zap.svg"),
        "social" => Some("icons/message-circle.svg"),
        "storage" => Some("icons/archive.svg"),
        "technology" => Some("icons/hard-drive.svg"),
        "transportation" => Some("icons/truck.svg"),
        "utility" => Some("icons/briefcase.svg"),
        "worldgen" | "locale" => Some("icons/globe.svg"),
        "audio" => Some("icons/headphones.svg"),
        "blocks" | "rift" => Some("icons/box.svg"),
        "core-shaders" => Some("icons/cpu.svg"),
        "fonts" => Some("icons/type.svg"),
        "gui" => Some("icons/panels-top-left.svg"),
        "models" => Some("icons/layers.svg"),
        "cartoon" => Some("icons/brush.svg"),
        "fantasy" => Some("icons/wand-sparkles.svg"),
        "realistic" => Some("icons/camera.svg"),
        "semi-realistic" => Some("icons/film.svg"),
        "vanilla-like" => Some("icons/ice-cream-cone.svg"),
        "atmosphere" => Some("icons/cloud-sun-rain.svg"),
        "colored-lighting" => Some("icons/palette.svg"),
        "foliage" => Some("icons/tree-pine.svg"),
        "path-tracing" => Some("icons/waypoints.svg"),
        "pbr" => Some("icons/lightbulb.svg"),
        "reflections" => Some("icons/flip-horizontal-2.svg"),
        "shadows" => Some("icons/mountain.svg"),
        "challenging" => Some("icons/chart-no-axes-combined.svg"),
        "kitchen-sink" => Some("icons/bath.svg"),
        "lightweight" | "liteloader" => Some("icons/feather.svg"),
        "multiplayer" => Some("icons/users.svg"),
        "quests" => Some("icons/network.svg"),
        _ => None,
    }
}

const FILTER_MOD_CATEGORIES: &[&'static str] = &[
    "adventure",
    "cursed",
    "decoration",
    "economy",
    "equipment",
    "food",
    "library",
    "magic",
    "management",
    "minigame",
    "mobs",
    "optimization",
    "social",
    "storage",
    "technology",
    "transportation",
    "utility",
    "worldgen"
];

const FILTER_MODPACK_CATEGORIES: &[&'static str] = &[
    "adventure",
    "challenging",
    "combat",
    "kitchen-sink",
    "lightweight",
    "magic",
    "multiplayer",
    "optimization",
    "quests",
    "technology",
];

const FILTER_RESOURCEPACK_CATEGORIES: &[&'static str] = &[
    "combat",
    "cursed",
    "decoration",
    "modded",
    "realistic",
    "simplistic",
    "themed",
    "tweaks",
    "utility",
    "vanilla-like",
];

const FILTER_SHADERPACK_CATEGORIES: &[&'static str] = &[
    "cartoon",
    "cursed",
    "fantasy",
    "realistic",
    "semi-realistic",
    "vanilla-like",
];
