use std::{collections::BTreeSet, ops::Range, sync::{Arc, atomic::AtomicBool}, time::Duration};

use bridge::{install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::{ContentFolder, ContentUpdateStatus, InstanceContentID, InstanceID}, message::MessageToBackend, meta::MetadataRequest, modal_action::ModalAction};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Selectable, WindowExt, button::{Button, ButtonGroup, ButtonVariant, ButtonVariants}, checkbox::Checkbox, h_flex, input::{Input, InputEvent, InputState}, notification::NotificationType, scroll::{ScrollableElement, Scrollbar}, skeleton::Skeleton, v_flex
};
use rustc_hash::FxHashMap;
use schema::{content::{ContentInstallReason, ContentSource}, curseforge::{CurseforgeClassId, CurseforgeHit, CurseforgeSearchRequest, CurseforgeSearchResult, CurseforgeSortField}, loader::Loader};
use strum::IntoEnumIterator;
use ustr::Ustr;

use crate::{
    component::error_alert::ErrorAlert, entity::{
        DataEntities, instance::ContentStates, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult}
    }, icon::PandoraIcon, interface_config::InterfaceConfig, pages::page::Page,
};

pub struct CurseforgeSearchPage {
    data: DataEntities,
    hits: Vec<CurseforgeHit>,
    install_for: Option<InstanceID>,
    filter_version: Option<Ustr>,
    loading: Option<Subscription>,
    pending_reload: bool,
    pending_clear: bool,
    total_hits: u64,
    search_state: Entity<InputState>,
    _search_input_subscription: Subscription,
    _delayed_clear_task: Task<()>,
    filter_loaders: EnumSet<Loader>,
    filter_categories: BTreeSet<u32>,
    sort_field: CurseforgeSortField,
    show_categories: Arc<AtomicBool>,
    show_sort: Arc<AtomicBool>,
    can_install_latest: bool,
    specific_installed_content_by_project: enum_map::EnumMap<ContentFolder, FxHashMap<u32, Vec<InstalledContent>>>,
    all_installed_content_by_project: FxHashMap<u32, Vec<InstalledContent>>,
    last_search: Arc<str>,
    scroll_handle: UniformListScrollHandle,
    search_error: Option<SharedString>,
    image_cache: Entity<RetainAllImageCache>,
    content_states: Option<ContentStates>,
}

#[derive(Clone, Copy)]
pub struct InstalledContent {
    pub content_id: InstanceContentID,
    pub status: ContentUpdateStatus
}

pub fn get_primary_action(
    project_id: u32,
    can_install_latest: bool,
    installed_content_by_project: &FxHashMap<u32, Vec<InstalledContent>>,
    cx: &App,
) -> PrimaryAction {
    let install_latest = can_install_latest && InterfaceConfig::get(cx).content_install_latest;
    let installed = installed_content_by_project.get(&project_id);

    if let Some(installed) = installed && !installed.is_empty() {
        if !install_latest {
            return PrimaryAction::Reinstall;
        }

        let mut action = PrimaryAction::CheckForUpdates;
        for installed_content in installed {
            match installed_content.status {
                ContentUpdateStatus::Unknown => {},
                ContentUpdateStatus::AlreadyUpToDate => {
                    if !matches!(action, PrimaryAction::Update(..)) {
                        action = PrimaryAction::UpToDate;
                    }
                },
                ContentUpdateStatus::Curseforge => {
                    if let PrimaryAction::Update(vec) = &mut action {
                        vec.push(installed_content.content_id);
                    } else {
                        action = PrimaryAction::Update(vec![installed_content.content_id]);
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

impl CurseforgeSearchPage {
    pub fn new(install_for: Option<InstanceID>, data: &DataEntities, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut project_type = InterfaceConfig::get(cx).curseforge_page_class_id;
        if project_type == CurseforgeClassId::Other {
            project_type = CurseforgeClassId::Mod;
            InterfaceConfig::get_mut(cx).curseforge_page_class_id = CurseforgeClassId::Mod;
        }

        let search_state = cx.new(|cx| {
            let placeholder = match project_type {
                CurseforgeClassId::Mod => t::instance::content::search::mod_(),
                CurseforgeClassId::Modpack => t::instance::content::search::modpack(),
                CurseforgeClassId::Resourcepack => t::instance::content::search::resourcepack(),
                CurseforgeClassId::Shader => t::instance::content::search::shader(),
                _ => t::instance::content::search::file(),
            };
            InputState::new(window, cx).placeholder(placeholder).clean_on_escape()
        });

        let mut can_install_latest = false;
        let mut specific_installed_content_by_project: enum_map::EnumMap<ContentFolder, FxHashMap<u32, Vec<InstalledContent>>> = Default::default();
        let mut all_installed_content_by_project: FxHashMap<u32, Vec<InstalledContent>> = FxHashMap::default();
        let mut filter_version = None;

        let mut content_states = None;
        if let Some(install_for) = install_for {
            if let Some(entry) = data.instances.read(cx).entries.get(&install_for) {
                let instance = entry.read(cx);
                content_states = Some(instance.content_states.clone());
                let instance_content = instance.content.clone();
                let loader = instance.configuration.loader;
                let minecraft_version = instance.configuration.minecraft_version;
                can_install_latest = loader != Loader::Vanilla;
                filter_version = Some(minecraft_version);

                for content_folder in ContentFolder::iter() {
                    let mut specific_installed_content: FxHashMap<u32, Vec<InstalledContent>> = FxHashMap::default();

                    for summary in instance_content[content_folder].read(cx).iter() {
                        let ContentSource::CurseforgeProject { project_id: project } = summary.content_source else {
                            continue;
                        };

                        let installed_content = InstalledContent {
                            content_id: summary.id,
                            status: summary.update.status_if_matches(loader, minecraft_version.as_str()),
                        };

                        let installed = all_installed_content_by_project.entry(project).or_default();
                        installed.push(installed_content);

                        let installed = specific_installed_content.entry(project).or_default();
                        installed.push(installed_content);
                    }

                    specific_installed_content_by_project[content_folder] = specific_installed_content;

                    let content = instance_content[content_folder].clone();
                    cx.observe(&content, move |page, entity, cx| {
                        let specific = &mut page.specific_installed_content_by_project[content_folder];

                        specific.clear();

                        let content = entity.read(cx);
                        for summary in content.iter() {
                            let ContentSource::CurseforgeProject { project_id: project } = summary.content_source else {
                                continue;
                            };

                            let installed = specific.entry(project).or_default();
                            installed.push(InstalledContent {
                                content_id: summary.id,
                                status: summary.update.status_if_matches(loader, minecraft_version.as_str()),
                            })
                        }

                        page.all_installed_content_by_project.clear();
                        for content_folder in ContentFolder::iter() {
                            for (key, value) in &page.specific_installed_content_by_project[content_folder] {
                                let installed = page.all_installed_content_by_project.entry(*key).or_default();
                                installed.extend_from_slice(value.as_slice());
                            }
                        }
                    }).detach();
                }
            }
        }

        let _search_input_subscription = cx.subscribe_in(&search_state, window, Self::on_search_input_event);

        let mut page = Self {
            data: data.clone(),
            hits: Vec::new(),
            install_for,
            filter_version,
            loading: None,
            pending_reload: false,
            pending_clear: false,
            total_hits: 1,
            sort_field: CurseforgeSortField::default(),
            search_state,
            _search_input_subscription,
            _delayed_clear_task: Task::ready(()),
            filter_loaders: Default::default(),
            filter_categories: Default::default(),
            show_categories: Arc::new(AtomicBool::new(false)),
            show_sort: Arc::new(AtomicBool::new(false)),
            can_install_latest,
            specific_installed_content_by_project,
            all_installed_content_by_project,
            last_search: Arc::from(""),
            scroll_handle: UniformListScrollHandle::new(),
            search_error: None,
            image_cache: RetainAllImageCache::new(cx),
            content_states,
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

    fn set_project_type(&mut self, project_type: CurseforgeClassId, window: &mut Window, cx: &mut Context<Self>) {
        if InterfaceConfig::get(cx).curseforge_page_class_id == project_type {
            return;
        }
        InterfaceConfig::get_mut(cx).curseforge_page_class_id = project_type;
        self.filter_categories.clear();
        self.search_state.update(cx, |state, cx| {
            let placeholder = match project_type {
                CurseforgeClassId::Mod => t::instance::content::search::mod_(),
                CurseforgeClassId::Modpack => t::instance::content::search::modpack(),
                CurseforgeClassId::Resourcepack => t::instance::content::search::resourcepack(),
                CurseforgeClassId::Shader => t::instance::content::search::shader(),
                _ => t::instance::content::search::file(),
            };
            state.set_placeholder(placeholder, window, cx)
        });
        self.reload(cx);
    }

    fn set_filter_loaders(&mut self, loaders: EnumSet<Loader>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_loaders == loaders {
            return;
        }
        self.filter_loaders = loaders;
        self.reload(cx);
    }

    fn set_filter_categories(&mut self, categories: BTreeSet<u32>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_categories == categories {
            return;
        }
        self.filter_categories = categories;
        self.reload(cx);
    }

    fn set_sort_field(&mut self, sort_field: CurseforgeSortField, _window: &mut Window, cx: &mut Context<Self>) {
        if self.sort_field == sort_field {
            return;
        }
        self.sort_field = sort_field;
        self.reload(cx);
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.loading.is_some() {
            self.pending_reload = true;
            return;
        }

        self.pending_clear = true;
        self._delayed_clear_task = cx.spawn(async |page, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
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
        self.pending_reload = false;
        self.search_error = None;

        let query = if self.last_search.is_empty() {
            None
        } else {
            Some(self.last_search.clone())
        };

        let config = InterfaceConfig::get(cx);
        let class_id = config.curseforge_page_class_id;
        let modrinth_filter_version = config.content_filter_version;

        let offset = if self.pending_clear { 0 } else { self.hits.len() };

        let is_mod = class_id == CurseforgeClassId::Mod || class_id == CurseforgeClassId::Modpack;
        let game_version = if is_mod && let Some(filter_version) = self.filter_version && modrinth_filter_version {
            Some(filter_version)
        } else {
            None
        };

        let mod_loader_types = if !self.filter_loaders.is_empty() && is_mod {
            let mut string = "[\"".to_string();
            for (i, loader) in self.filter_loaders.iter().enumerate() {
                if i > 0 {
                    string.push_str("\",\"");
                }
                string.push_str(loader.pretty_name());
            }
            string.push_str("\"]");
            let string: Arc<str> = string.into();
            Some(string)
        } else {
            None
        };

        let category_ids = if !self.filter_categories.is_empty() {
            let mut string = "[\"".to_string();
            for (i, category_id) in self.filter_categories.iter().enumerate() {
                if i > 0 {
                    string.push_str("\",\"");
                }
                use std::fmt::Write;
                _ = write!(&mut string, "{}", *category_id);
            }
            string.push_str("\"]");
            let string: Arc<str> = string.into();
            Some(string)
        } else {
            None
        };

        let request = CurseforgeSearchRequest {
            search_filter: query,
            class_id: class_id as u32,
            category_ids,
            game_version,
            mod_loader_types,
            sort_field: self.sort_field as u32,
            index: offset as u32,
            page_size: 20
        };

        let data = FrontendMetadata::request(&self.data.metadata, MetadataRequest::CurseforgeSearch(request), cx);

        let result: FrontendMetadataResult<CurseforgeSearchResult> = data.read(cx).result();
        match result {
            FrontendMetadataResult::Loading => {
                let subscription = cx.observe(&data, |page, data, cx| {
                    let result: FrontendMetadataResult<CurseforgeSearchResult> = data.read(cx).result();
                    match result {
                        FrontendMetadataResult::Loading => {
                            return;
                        },
                        FrontendMetadataResult::Loaded(result) => {
                            if !page.pending_reload {
                                page.apply_search_data(result);
                            }
                            page.loading = None;
                            cx.notify();
                        },
                        FrontendMetadataResult::Error(shared_string) => {
                            page.search_error = Some(shared_string);
                            page.loading = None;
                            cx.notify();
                        },
                    }
                    if page.pending_reload {
                        page.reload(cx);
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

    fn apply_search_data(&mut self, search_result: &CurseforgeSearchResult) {
        if self.pending_clear {
            self.pending_clear = false;
            self.hits.clear();
            self.total_hits = 1;
            self._delayed_clear_task = Task::ready(());
        }

        self.hits.extend(search_result.data.iter().map(|hit| {
            let mut hit = hit.clone();
            hit.summary = hit.summary.replace("\n", " ").into();
            hit
        }));
        self.total_hits = search_result.pagination.total_count;
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
                            .child(ErrorAlert::new(t::instance::content::requesting_from_modrinth_error().into(), search_error));
                    } else {
                        should_load_more = true;
                        return div()
                            .pl_3()
                            .pt_3()
                            .child(Skeleton::new().w_full().h(px(28.0 * 4.0)).rounded_lg());
                    }
                };


                let image = if let Some(logo) = &hit.logo && !logo.thumbnail_url.is_empty() {
                    gpui::img(SharedUri::from(logo.thumbnail_url.clone()))
                        .with_fallback(|| Skeleton::new().rounded_lg().size_16().into_any_element())
                } else {
                    gpui::img(ImageSource::Resource(Resource::Embedded(
                        "images/default_mod.png".into(),
                    )))
                };

                let author = if hit.authors.len() == 1 {
                    let author = &*hit.authors[0].name;
                    t::instance::content::by(author)
                } else if hit.authors.is_empty() {
                    t::instance::content::by("Unknown")
                } else {
                    let mut authors_string = String::new();
                    for (i, author) in hit.authors.iter().enumerate() {
                        if i > 0 {
                            authors_string.push_str(", ");
                        }
                        authors_string.push_str(&author.name);
                    }
                    t::instance::content::by(&authors_string)
                };

                let name = SharedString::new(hit.name.clone());
                let description = SharedString::new(hit.summary.clone());

                let author_line = div().text_color(cx.theme().muted_foreground).text_sm().pb_px().child(author);

                let muted = cx.theme().muted_foreground;
                let mut is_categories_empty = true;
                let categories = hit.categories.iter().filter_map(|category| {
                    if category.is_class {
                        return None;
                    }
                    is_categories_empty = false;
                    Some(SharedString::new(category.name.clone()).into_any_element())
                });
                let categories = itertools::Itertools::intersperse_with(categories,
                    || div().flex_shrink_0().w_px().h_1_2().bg(muted).into_any_element());

                let downloads = h_flex()
                    .gap_0p5()
                    .child(PandoraIcon::Download)
                    .child(format_downloads(hit.download_count));

                let primary_action = self.get_primary_action(hit.id, cx);

                let install_button = Button::new(("install", index))
                    .label(primary_action.text())
                    .icon(primary_action.icon())
                    .with_variant(primary_action.button_variant())
                    .on_click({
                        let data = self.data.clone();
                        let hit = hit.clone();
                        let install_for = self.install_for.clone();

                        move |_, window, cx| {
                            cx.stop_propagation();

                            if hit.class_id.is_some() && hit.class_id != Some(0) {
                                match primary_action {
                                    PrimaryAction::Install | PrimaryAction::Reinstall => {
                                        crate::modals::curseforge_install::open(
                                            hit.clone(),
                                            install_for,
                                            &data,
                                            window,
                                            cx
                                        );
                                    },
                                    PrimaryAction::InstallLatest => {
                                        let Some(install_for) = install_for else {
                                            window.push_notification((NotificationType::Error, "Unable to find instance"), cx);
                                            return;
                                        };

                                        let Some(entry) = data.instances.read(cx).entries.get(&install_for) else {
                                            window.push_notification((NotificationType::Error, "Unable to find instance"), cx);
                                            return;
                                        };

                                        let instance = entry.read(cx);
                                        let loader = instance.configuration.loader;
                                        let minecraft_version = instance.configuration.minecraft_version;

                                        let content_install = ContentInstall {
                                            target: InstallTarget::Instance(instance.id),
                                            loader,
                                            minecraft_version,
                                            files: [
                                                ContentInstallFile {
                                                    replace_old: None,
                                                    path: bridge::install::ContentInstallPath::Automatic,
                                                    download: ContentDownload::Curseforge {
                                                        project_id: hit.id,
                                                        install_dependencies: true,
                                                    },
                                                    content_source: ContentSource::CurseforgeProject {
                                                        project_id: hit.id
                                                    },
                                                    reason: ContentInstallReason::Standalone,
                                                }
                                            ].into(),
                                        };

                                        crate::root::start_install(content_install, &data.backend_handle, window, cx);
                                    },
                                    PrimaryAction::CheckForUpdates => {
                                        let modal_action = ModalAction::default();
                                        data.backend_handle.send(MessageToBackend::UpdateCheck {
                                            instance: install_for.unwrap(),
                                            modal_action: modal_action.clone()
                                        });
                                        crate::modals::generic::show_notification(window, cx,
                                            t::instance::content::update::check::error().into(), modal_action);
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
                                                t::instance::content::update::error().into(), modal_action);
                                        }
                                    },
                                }
                            } else {
                                window.push_notification(
                                    (
                                        NotificationType::Error,
                                        t::instance::content::install::unknown_type(),
                                    ),
                                    cx,
                                );
                            }
                        }
                    });

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
                                    .child(PandoraIcon::Tags)
                                    .children(categories),
                            ),
                    )
                    .child(v_flex().items_end().child(downloads).child(install_button));

                div().pl_3().pt_3().child(item)
            })
            .collect();

        if should_load_more {
            self.load_more(cx);
        }

        items
    }

    pub fn get_primary_action(&self, project_id: u32, cx: &App) -> PrimaryAction {
        get_primary_action(project_id, self.can_install_latest, &self.all_installed_content_by_project, cx)
    }
}

#[derive(PartialEq, Eq)]
pub enum PrimaryAction {
    Install,
    Reinstall,
    InstallLatest,
    CheckForUpdates,
    ErrorCheckingForUpdates,
    UpToDate,
    Update(Vec<InstanceContentID>),
}

impl PrimaryAction {
    pub fn text(&self) -> SharedString {
        match self {
            PrimaryAction::Install => t::instance::content::install::label(),
            PrimaryAction::Reinstall => t::instance::content::install::reinstall(),
            PrimaryAction::InstallLatest => t::instance::content::install::latest(),
            PrimaryAction::CheckForUpdates => t::instance::content::update::check::label(true),
            PrimaryAction::ErrorCheckingForUpdates => t::common::error(),
            PrimaryAction::UpToDate => t::instance::content::update::check::up_to_date(),
            PrimaryAction::Update(..) => t::instance::content::update::label(),
        }.into()
    }

    pub fn icon(&self) -> PandoraIcon {
        match self {
            PrimaryAction::Install => PandoraIcon::Download,
            PrimaryAction::Reinstall => PandoraIcon::Download,
            PrimaryAction::InstallLatest => PandoraIcon::Download,
            PrimaryAction::CheckForUpdates => PandoraIcon::RefreshCcw,
            PrimaryAction::ErrorCheckingForUpdates => PandoraIcon::TriangleAlert,
            PrimaryAction::UpToDate => PandoraIcon::Check,
            PrimaryAction::Update(..) => PandoraIcon::Download,
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

impl Page for CurseforgeSearchPage {
    fn controls(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }

    fn scrollable(&self, _cx: &App) -> bool {
        false
    }
}

impl Render for CurseforgeSearchPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_load_more = self.total_hits > self.hits.len() as u64;
        let scroll_handle = self.scroll_handle.clone();

        let item_count = self.hits.len() + if can_load_more || self.search_error.is_some() { 1 } else { 0 };

        if let Some(content_states) = &self.content_states {
            content_states.observe_all();
        }

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
            let install_latest = InterfaceConfig::get(cx).content_install_latest;
            top_bar = top_bar.child(Checkbox::new("install-latest")
                .label(t::instance::content::install::latest())
                .tooltip(t::instance::content::install::always_latest())
                .checked(install_latest)
                .on_click({
                    move |value, _, cx| {
                        InterfaceConfig::get_mut(cx).content_install_latest = *value;
                    }
                })
            );
        }

        let theme = cx.theme();
        let content = v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .pl_0()
            .child(top_bar)
            .child(div().size_full().rounded_lg().border_1().border_color(theme.border).child(list));

        let config = InterfaceConfig::get(cx);
        let filter_project_type = config.curseforge_page_class_id;

        let type_button_group = ButtonGroup::new("type")
            .layout(Axis::Vertical)
            .outline()
            .child(Button::new("mods").label(t::instance::content::mods()).selected(filter_project_type == CurseforgeClassId::Mod))
            .child(
                Button::new("modpacks")
                    .label(t::instance::content::modpacks())
                    .selected(filter_project_type == CurseforgeClassId::Modpack),
            )
            .child(
                Button::new("resourcepacks")
                    .label(t::instance::content::resourcepacks())
                    .selected(filter_project_type == CurseforgeClassId::Resourcepack),
            )
            .child(Button::new("shaders").label(t::instance::content::shaders()).selected(filter_project_type == CurseforgeClassId::Shader))
            .on_click(cx.listener(|page, clicked: &Vec<usize>, window, cx| match clicked[0] {
                0 => page.set_project_type(CurseforgeClassId::Mod, window, cx),
                1 => page.set_project_type(CurseforgeClassId::Modpack, window, cx),
                2 => page.set_project_type(CurseforgeClassId::Resourcepack, window, cx),
                3 => page.set_project_type(CurseforgeClassId::Shader, window, cx),
                _ => {},
            }));

        let loader_button_group = if filter_project_type == CurseforgeClassId::Mod || filter_project_type == CurseforgeClassId::Modpack {
            Some(ButtonGroup::new("loader_group")
                .layout(Axis::Vertical)
                .outline()
                .multiple(true)
                .child(Button::new("fabric").label(t::modrinth::category::fabric()).selected(self.filter_loaders.contains(Loader::Fabric)))
                .child(Button::new("forge").label(t::modrinth::category::forge()).selected(self.filter_loaders.contains(Loader::Forge)))
                .child(Button::new("neoforge").label(t::modrinth::category::neoforge()).selected(self.filter_loaders.contains(Loader::NeoForge)))
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

        let categories = match filter_project_type {
            CurseforgeClassId::Mod => FILTER_MOD_CATEGORIES,
            CurseforgeClassId::Modpack => FILTER_MODPACK_CATEGORIES,
            CurseforgeClassId::Resourcepack => FILTER_RESOURCEPACK_CATEGORIES,
            CurseforgeClassId::Shader => FILTER_SHADERPACK_CATEGORIES,
            _ => &[],
        };

        let is_category_shown = self.show_categories.load(std::sync::atomic::Ordering::Relaxed);
        let show_categories = self.show_categories.clone();

        let category = v_flex()
            .gap_1()
            .child(
                Button::new("toggle-categories")
                    .label(t::instance::content::categories())
                    .icon(if is_category_shown { PandoraIcon::ChevronDown } else { PandoraIcon::ChevronRight })
                    .when(!is_category_shown, |this| this.outline())
                    .on_click(move |_, _, _| {
                        show_categories.store(!is_category_shown, std::sync::atomic::Ordering::Relaxed);
                    })
            )
            .when(is_category_shown, |this| this.child(ButtonGroup::new("category_group")
                .layout(Axis::Vertical)
                .outline()
                .multiple(true)
                .children(categories.iter().map(|(name, id)| {
                    Button::new(("category", *id))
                        .child(
                            h_flex().w_full().justify_start().gap_2()
                            .child(SharedString::new(*name)))
                        .selected(self.filter_categories.contains(id))
                }))
                .on_click(cx.listener(|page, clicked: &Vec<usize>, window, cx| {
                    page.set_filter_categories(clicked.iter()
                        .filter_map(|index| categories.get(*index).map(|(_, id)| *id))
                        .collect(), window, cx);
                }))))
            .into_any_element();

        let is_sort_shown = self.show_sort.load(std::sync::atomic::Ordering::Relaxed);
        let show_sort = self.show_sort.clone();

        let sort = v_flex()
            .gap_1()
            .child(
                Button::new("toggle-sort")
                    .label(t::instance::content::sort())
                    .icon(if is_sort_shown { PandoraIcon::ChevronDown } else { PandoraIcon::ChevronRight })
                    .when(!is_sort_shown, |this| this.outline())
                    .on_click(move |_, _, _| {
                        show_sort.store(!is_sort_shown, std::sync::atomic::Ordering::Relaxed);
                    })
            )
            .when(is_sort_shown, |this| this.child(ButtonGroup::new("sort_field")
                .layout(Axis::Vertical)
                .outline()
                .children(CurseforgeSortField::iter().map(|field| {
                    let id = field.clone() as u32;
                    Button::new(("sort", id))
                        .child(
                            h_flex().w_full().justify_start().gap_2()
                            .child(t::curseforge::sort::get(field.as_str()).unwrap_or("missing_translation")))
                        .selected(id == self.sort_field.clone() as u32)
                }))
                .on_click(cx.listener(move |page, clicked: &Vec<usize>, window, cx| {
                    let sort_field = CurseforgeSortField::iter().nth(clicked[0]).unwrap_or_default();
                    page.set_sort_field(sort_field, window, cx);
                }))))
            .into_any_element();

        let is_mod = filter_project_type == CurseforgeClassId::Mod || filter_project_type == CurseforgeClassId::Modpack;
        let filter_version_toggle = if is_mod && let Some(filter_version) = self.filter_version {
            let title = format!("{}: {}", t::instance::version(), filter_version);
            Some(Button::new("filter_version").label(title)
                .outline()
                .selected(InterfaceConfig::get(cx).content_filter_version)
                .on_click(cx.listener(|page, _, _, cx| {
                    let cfg = InterfaceConfig::get_mut(cx);
                    cfg.content_filter_version = !cfg.content_filter_version;
                    page.reload(cx);
                })))
        } else {
            None
        };

        let parameters = v_flex()
            .h_full()
            .overflow_y_scrollbar()
            .w_auto()
            .min_w(px(200.0))
            .p_3()
            .gap_3()
            .child(type_button_group)
            .when_some(loader_button_group, |this, group| this.child(group))
            .when_some(filter_version_toggle, |this, button| this.child(button))
            .child(category)
            .child(sort);

        h_flex().flex_1().min_h_0().size_full().child(parameters).child(content)
    }
}

pub fn format_downloads(downloads: u64) -> SharedString {
    if downloads >= 1_000_000_000 {
        t::instance::content::downloads::b((downloads / 10_000_000) as f64 / 100.0)
    } else if downloads >= 1_000_000 {
        t::instance::content::downloads::m((downloads / 10_000) as f64 / 100.0)
    } else if downloads >= 10_000 {
        t::instance::content::downloads::k((downloads / 10) as f64 / 100.0)
    } else {
        t::instance::content::downloads::n(downloads)
    }.into()
}

const FILTER_MOD_CATEGORIES: &[(&'static str, u32)] = &[
    ("Addons", 426),
    ("Adventure & RPG", 422),
    ("API and Library", 421),
    ("Equipment", 434),
    ("Bug Fixes", 6821),
    ("Cosmetic", 424),
    ("Education", 5299),
    ("Food", 436),
    ("Magic", 419),
    ("Map & Information", 423),
    ("Performance", 6814),
    ("Redstone", 4558),
    ("Server Utility", 435),
    ("Storage", 420),
    ("Technology", 412),
    ("Utility & QoL", 5191),
    ("World Gen", 406),
];

const FILTER_MODPACK_CATEGORIES: &[(&'static str, u32)] = &[
    ("Adventure & RPG", 4475),
    ("Combat / PvP", 4483),
    ("Expert", 9243),
    ("Exploration", 4476),
    ("Extra Large", 4482),
    ("FTB Official Pack", 4487),
    ("Hardcore", 4479),
    ("Horror", 7418),
    ("Magic", 4473),
    ("Map Based", 4480),
    ("Mini Game", 4477),
    ("Multiplayer", 4484),
    ("Quests", 4478),
    ("Sci-Fi", 4474),
    ("Skyblock", 4736),
    ("Small / Light", 4481),
    ("Tech", 4472),
    ("Vanilla+", 5128),
];

const FILTER_RESOURCEPACK_CATEGORIES: &[(&'static str, u32)] = &[
    ("16x", 393),
    ("32x", 394),
    ("64x", 395),
    ("128x", 396),
    ("256x", 397),
    ("512x and Higher", 398),
    ("Animated", 404),
    ("Data Packs", 5193),
    ("Font Packs", 5244),
    ("Medieval", 402),
    ("Mod Support", 4465),
    ("Modern", 401),
    ("Photo Realistic", 400),
    ("Steampunk", 399),
    ("Traditional", 403),
];

const FILTER_SHADERPACK_CATEGORIES: &[(&'static str, u32)] = &[
    ("Fantasy", 6554),
    ("Realistic", 6553),
    ("Vanilla", 6555),
];
