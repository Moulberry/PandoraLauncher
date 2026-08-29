use std::{path::Path, sync::Arc};

use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, instance::{ContentType, InstanceContentSummary, InstanceID}, meta::MetadataRequest
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, button::{Button, ButtonVariants}, checkbox::Checkbox, dialog::Dialog, h_flex, list::ListState, notification::NotificationType, select::{SearchableVec, Select, SelectItem, SelectState}, skeleton::Skeleton, spinner::Spinner, text::TextView, v_flex, IndexPath, WindowExt
};
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{
    content::{ContentInstallReason, ContentSource},
    curseforge::{CurseforgeChangelogRequest, CurseforgeChangelogResult, CurseforgeFile, CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeModLoaderType, CurseforgeReleaseType},
    instance::UpdateChannel,
    loader::Loader,
    modrinth::{ModrinthChangelogRequest, ModrinthChangelogResult, ModrinthLoader, ModrinthProjectVersion, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthVersionType}
};
use ustr::Ustr;

use crate::{
    component::{content_list::ContentListDelegate, error_alert::ErrorAlert},
    entity::{
        DataEntities, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}
    },
    root,
};

#[derive(Clone, PartialEq, Eq, Hash)]
enum VersionKey {
    Modrinth(Arc<str>),
    Curseforge(u32),
}

#[derive(Clone)]
enum VersionSource {
    Modrinth(ModrinthProjectVersion),
    Curseforge(CurseforgeFile),
}

#[derive(Clone)]
struct VersionItem {
    key: VersionKey,
    name: SharedString,
    raw_name: SharedString,
    date_published: Option<Arc<str>>,
    sha1: Option<Arc<str>>,
    source: VersionSource,
}

impl PartialEq for VersionItem {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl SelectItem for VersionItem {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

enum ChangelogContent {
    Markdown(SharedString),
    Html(SharedString),
}

enum ChangelogDisplay {
    Loading,
    Loaded(Option<ChangelogContent>),
}

fn is_project(source: &ContentSource) -> bool {
    matches!(source, ContentSource::ModrinthProject { .. } | ContentSource::CurseforgeProject { .. })
}

fn versions_request(
    project: &ContentSource,
    minecraft_version: Option<Ustr>,
    modrinth_loaders: Option<Arc<[ModrinthLoader]>>,
    curseforge_loader: Option<CurseforgeModLoaderType>,
) -> MetadataRequest {
    match project {
        ContentSource::ModrinthProject { project_id } => MetadataRequest::ModrinthProjectVersions(ModrinthProjectVersionsRequest {
            project_id: project_id.clone(),
            game_versions: minecraft_version.map(|minecraft_version| vec![Arc::<str>::from(minecraft_version.as_str())].into()),
            loaders: modrinth_loaders,
        }),
        ContentSource::CurseforgeProject { project_id } => MetadataRequest::CurseforgeGetModFiles(CurseforgeGetModFilesRequest {
            mod_id: *project_id,
            game_version: minecraft_version,
            mod_loader_type: curseforge_loader.map(|loader| loader as u32),
            release_types: None,
            page_size: Some(5000),
        }),
        _ => unreachable!(),
    }
}

fn modrinth_version_item(version: &ModrinthProjectVersion) -> Option<VersionItem> {
    if version.files.is_empty() {
        return None;
    }

    let mc_version = version.game_versions.as_deref()
        .and_then(display_game_version)
        .unwrap_or_else(|| t::common::unknown().to_string());
    let loader = version.loaders.as_deref().and_then(<[ModrinthLoader]>::first)
        .filter(|loader| !matches!(loader, ModrinthLoader::Unknown))
        .map(|loader| loader.pretty_name())
        .unwrap_or("Minecraft");

    let raw_name = version.version_number.clone().or_else(|| version.name.clone()).unwrap_or_else(|| version.id.clone());
    let base = t::instance::content::change_version::version(&raw_name, loader, &mc_version);
    let name: SharedString = match version.version_type {
        Some(ModrinthVersionType::Beta) => t::modrinth::versions::beta(&base).into(),
        Some(ModrinthVersionType::Alpha) => t::modrinth::versions::alpha(&base).into(),
        _ => base.into(),
    };

    Some(VersionItem {
        key: VersionKey::Modrinth(version.id.clone()),
        name,
        raw_name: raw_name.into(),
        date_published: version.date_published.clone(),
        sha1: version.files.iter().find(|file| file.primary)
            .or_else(|| version.files.first())
            .map(|file| file.hashes.sha1.clone()),
        source: VersionSource::Modrinth(version.clone()),
    })
}

fn curseforge_version_item(file: &CurseforgeFile) -> VersionItem {
    let game_versions = file.game_versions.as_deref().unwrap_or_default();
    let mc_version = display_game_version(game_versions).unwrap_or_else(|| t::common::unknown().to_string());
    let loader = game_versions.iter().find_map(|name| {
        let loader = CurseforgeModLoaderType::from_name(name.as_str());
        (loader != CurseforgeModLoaderType::Any).then(|| loader.pretty_name())
    })
    .unwrap_or("Minecraft");

    let base = t::instance::content::change_version::version(&file.file_name, loader, &mc_version);
    let name: SharedString = match CurseforgeReleaseType::from_u32(file.release_type) {
        CurseforgeReleaseType::Beta => t::modrinth::versions::beta(&base).into(),
        CurseforgeReleaseType::Alpha => t::modrinth::versions::alpha(&base).into(),
        _ => base.into(),
    };

    VersionItem {
        key: VersionKey::Curseforge(file.id),
        name,
        raw_name: file.file_name.to_string().into(),
        date_published: file.file_date.clone(),
        sha1: file.hashes.iter().find(|hash| hash.algo == 1).map(|hash| hash.value.clone()),
        source: VersionSource::Curseforge(file.clone()),
    }
}

fn is_release(item: &VersionItem) -> bool {
    match &item.source {
        VersionSource::Modrinth(version) => version.version_type == Some(ModrinthVersionType::Release),
        VersionSource::Curseforge(file) => CurseforgeReleaseType::from_u32(file.release_type) == CurseforgeReleaseType::Release,
    }
}

fn is_beta(item: &VersionItem) -> bool {
    match &item.source {
        VersionSource::Modrinth(version) => version.version_type == Some(ModrinthVersionType::Beta),
        VersionSource::Curseforge(file) => CurseforgeReleaseType::from_u32(file.release_type) == CurseforgeReleaseType::Beta,
    }
}

fn is_alpha(item: &VersionItem) -> bool {
    match &item.source {
        VersionSource::Modrinth(version) => version.version_type == Some(ModrinthVersionType::Alpha),
        VersionSource::Curseforge(file) => CurseforgeReleaseType::from_u32(file.release_type) == CurseforgeReleaseType::Alpha,
    }
}

fn display_game_version(game_versions: &[Ustr]) -> Option<String> {
    let first = game_versions.first()?.as_str();

    Some(match game_versions.len() {
        1 => first.to_owned(),
        2 => t::common::and(first, game_versions.get(1)?),
        _ => t::common::range(first, game_versions.last()?),
    })
}

struct ChangeVersionDialog {
    title: SharedString,
    instance_id: InstanceID,
    data: DataEntities,
    content_source: ContentSource,
    version_label: SharedString,
    installed_sha1: Arc<str>,
    installed_path: Arc<Path>,
    enabled: bool,
    updating: Arc<Mutex<FxHashSet<u64>>>,
    content_list: Entity<ListState<ContentListDelegate>>,
    filename_hash: u64,
    loader: Loader,
    minecraft_version: Ustr,
    update_channel: UpdateChannel,
    modrinth_loaders: Arc<[ModrinthLoader]>,
    curseforge_loader: Option<CurseforgeModLoaderType>,

    versions_filtered: Option<Entity<FrontendMetadataState>>,
    versions_unfiltered: Option<Entity<FrontendMetadataState>>,
    items: Option<Vec<VersionItem>>,
    version_select_state: Option<Entity<SelectState<SearchableVec<VersionItem>>>>,
    current_date_published: Option<Arc<str>>,
    show_incompatible: bool,
    changelogs: FxHashMap<VersionKey, Entity<FrontendMetadataState>>,
    versions_error: Option<SharedString>,
    changelog_error: Option<SharedString>,
    _versions_subscription: Option<Subscription>,
    _changelog_subscription: Option<Subscription>,
}

pub fn open(
    instance_id: InstanceID,
    summary: &InstanceContentSummary,
    data: &DataEntities,
    updating: Arc<Mutex<FxHashSet<u64>>>,
    content_list: Entity<ListState<ContentListDelegate>>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(instance_entry) = data.instances.read(cx).entries.get(&instance_id) else {
        return;
    };
    let loader = instance_entry.read(cx).configuration.loader;
    let minecraft_version = instance_entry.read(cx).configuration.minecraft_version;
    let update_channel = instance_entry.read(cx).configuration.update_channel;

    let mut sha1 = [0_u8; 20];
    sha1.copy_from_slice(&summary.content_summary.hash);
    let installed_sha1: Arc<str> = hex::encode(sha1).into();

    let modrinth_loaders = summary.content_summary.extra.modrinth_loaders(loader.as_modrinth_loader());
    let curseforge_loader = summary.content_summary.extra.curseforge_loader();

    let content_name: SharedString = summary.content_summary.name.clone()
        .unwrap_or_else(|| summary.filename.clone())
        .into();
    let title: SharedString = t::instance::content::change_version::title(&content_name).into();
    let version_label: SharedString = match &summary.content_summary.extra {
        ContentType::ResourcePack => t::instance::content::version::resourcepack(),
        ContentType::ShaderPack => t::instance::content::version::shader(),
        ContentType::ModrinthModpack { .. } | ContentType::CurseforgeModpack { .. } => t::instance::content::version::modpack(),
        _ => t::instance::content::version::mod_(),
    }.into();

    let dialog = ChangeVersionDialog {
        title,
        instance_id,
        data: data.clone(),
        content_source: summary.content_source.clone(),
        version_label,
        installed_sha1,
        installed_path: summary.path.clone(),
        enabled: summary.enabled,
        updating,
        content_list,
        filename_hash: summary.filename_hash,
        loader,
        minecraft_version,
        update_channel,
        modrinth_loaders,
        curseforge_loader,
        versions_filtered: None,
        versions_unfiltered: None,
        items: None,
        version_select_state: None,
        current_date_published: None,
        show_incompatible: false,
        changelogs: Default::default(),
        versions_error: None,
        changelog_error: None,
        _versions_subscription: None,
        _changelog_subscription: None,
    };

    dialog.show(window, cx);
}

impl ChangeVersionDialog {
    fn show(self, window: &mut Window, cx: &mut App) {
        let dialog = cx.new(|_| self);
        window.open_dialog(cx, move |modal, window, cx| {
            dialog.update(cx, |this, cx| this.render(modal, window, cx))
        });
    }

    fn active_versions(&self) -> Option<&Entity<FrontendMetadataState>> {
        if self.show_incompatible {
            self.versions_unfiltered.as_ref()
        } else {
            self.versions_filtered.as_ref()
        }
    }

    fn request_versions(&mut self, cx: &mut Context<Self>) {
        if self.content_source == ContentSource::ModrinthUnknown {
            self.versions_error = Some("Unable to get versions for this project. Please reinstall it.".into());
            return;
        }

        if is_project(&self.content_source) {
            if self.active_versions().is_some() {
                return;
            }

            let request = if self.show_incompatible {
                versions_request(&self.content_source, None, None, None)
            } else {
                versions_request(&self.content_source, Some(self.minecraft_version), Some(self.modrinth_loaders.clone()), self.curseforge_loader)
            };

            let entity = FrontendMetadata::request(&self.data.metadata, request, cx);
            self._versions_subscription = Some(cx.observe(&entity, |_, _, cx| cx.notify()));

            if self.show_incompatible {
                self.versions_unfiltered = Some(entity);
            } else {
                self.versions_filtered = Some(entity);
            }
        }
    }

    fn request_changelog(&mut self, key: &VersionKey, cx: &mut Context<Self>) {
        let entity = if let Some(entity) = self.changelogs.get(key) {
            entity.clone()
        } else {
            let request = match (key, &self.content_source) {
                (VersionKey::Modrinth(version_id), _) => MetadataRequest::ModrinthChangelog(ModrinthChangelogRequest {
                    version_id: version_id.clone(),
                }),
                (VersionKey::Curseforge(file_id), ContentSource::CurseforgeProject { project_id }) => MetadataRequest::CurseforgeChangelog(CurseforgeChangelogRequest {
                    mod_id: *project_id,
                    file_id: *file_id,
                }),
                _ => return,
            };

            let entity = FrontendMetadata::request(&self.data.metadata, request, cx);
            self.changelogs.insert(key.clone(), entity.clone());
            entity
        };

        self._changelog_subscription = Some(cx.observe(&entity, |_, _, cx| cx.notify()));
    }

    fn render_version_select(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.items.is_none() {
            match self.active_versions() {
                Some(versions) => match &self.content_source {
                    ContentSource::ModrinthProject { .. } => {
                        let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = versions.read(cx).result();
                        match result {
                            FrontendMetadataResult::Loaded(versions) => {
                                self.versions_error = None;
                                self.items = Some(versions.0.iter().filter_map(modrinth_version_item).collect());
                            },
                            FrontendMetadataResult::Loading => self.versions_error = None,
                            FrontendMetadataResult::Error(error) => {
                                self.versions_error = Some(error);
                            },
                        }
                    },
                    ContentSource::CurseforgeProject { .. } => {
                        let result: FrontendMetadataResult<CurseforgeGetModFilesResult> = versions.read(cx).result();
                        match result {
                            FrontendMetadataResult::Loaded(result) => {
                                self.versions_error = None;
                                self.items = Some(result.data.iter().map(curseforge_version_item).collect());
                            },
                            FrontendMetadataResult::Loading => self.versions_error = None,
                            FrontendMetadataResult::Error(error) => {
                                self.versions_error = Some(error);
                            },
                        }
                    },
                    _ => self.items = Some(Vec::new()),
                },
                None => self.items = Some(Vec::new()),
            }
        }

        let Some(items) = self.items.as_ref() else {
            return Skeleton::new().w_full().min_h_8().max_h_8().rounded_md().into_any_element();
        };

        let state = if let Some(state) = self.version_select_state.clone() {
            state
        } else {
            self.current_date_published = items.iter()
                .find(|item| item.sha1.as_deref() == Some(self.installed_sha1.as_ref()))
                .and_then(|item| item.date_published.clone());

            let preselected = match self.update_channel {
                UpdateChannel::Release => items.iter().position(is_release)
                    .or_else(|| items.iter().position(is_beta))
                    .or_else(|| items.iter().position(is_alpha)),
                UpdateChannel::Beta => items.iter().position(|item| is_release(item) || is_beta(item))
                    .or_else(|| items.iter().position(is_alpha)),
                UpdateChannel::Alpha => items.first().map(|_| 0),
            };

            let state = cx.new(|cx| {
                let mut select_state =
                    SelectState::new(SearchableVec::new(items.clone()), None, window, cx).searchable(true);
                if let Some(row) = preselected {
                    select_state.set_selected_index(Some(IndexPath::default().row(row)), window, cx);
                }
                select_state
            });
            self.version_select_state = Some(state.clone());
            state
        };

        let select = Select::new(&state).search_placeholder(t::common::search());

        if items.is_empty() {
            select
                .placeholder(t::instance::content::change_version::no_compatible_versions())
                .disabled(true)
                .into_any_element()
        } else {
            select.into_any_element()
        }
    }

    fn changelog_display(&mut self, key: &VersionKey, cx: &App) -> ChangelogDisplay {
        let Some(entity) = self.changelogs.get(key) else {
            return ChangelogDisplay::Loading;
        };
        self.changelog_error = None;

        match key {
            VersionKey::Modrinth(_) => {
                let result: FrontendMetadataResult<ModrinthChangelogResult> = entity.read(cx).result();
                match result {
                    FrontendMetadataResult::Loading => ChangelogDisplay::Loading,
                    FrontendMetadataResult::Loaded(changelog) => ChangelogDisplay::Loaded(
                        changelog.changelog.clone()
                            .filter(|changelog| !changelog.trim_ascii().is_empty())
                            .map(|changelog| ChangelogContent::Markdown(SharedString::from(changelog.to_string()))),
                    ),
                    FrontendMetadataResult::Error(error) => {
                        self.changelog_error = Some(error);
                        ChangelogDisplay::Loaded(None)
                    },
                }
            },
            VersionKey::Curseforge(_) => {
                let result: FrontendMetadataResult<CurseforgeChangelogResult> = entity.read(cx).result();
                match result {
                    FrontendMetadataResult::Loading => ChangelogDisplay::Loading,
                    FrontendMetadataResult::Loaded(changelog) => ChangelogDisplay::Loaded(
                        changelog.data.clone()
                            .filter(|data| !data.trim_ascii().is_empty())
                            .map(|data| ChangelogContent::Html(SharedString::from(data.to_string()))),
                    ),
                    FrontendMetadataResult::Error(error) => {
                        self.changelog_error = Some(error);
                        ChangelogDisplay::Loaded(None)
                    },
                }
            },
        }
    }

    fn render_changelog_area(&mut self, selected_key: Option<&VersionKey>, cx: &App) -> AnyElement {
        let display = match (selected_key, self.version_select_state.is_none()) {
            (None, true) => ChangelogDisplay::Loading,
            (None, false) => ChangelogDisplay::Loaded(None),
            (Some(key), _) => self.changelog_display(key, cx),
        };

        let body = match display {
            ChangelogDisplay::Loading => h_flex()
                .h_full()
                .w_full()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(Spinner::new().color(cx.theme().muted_foreground).with_size(px(18.0)))
                .child(t::instance::content::change_version::loading_changelog()).into_any_element(),
            ChangelogDisplay::Loaded(None) => h_flex()
                .h_full()
                .w_full()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(t::instance::content::change_version::no_changelog()).into_any_element(),
            ChangelogDisplay::Loaded(Some(ChangelogContent::Markdown(changelog))) => TextView::markdown("changelog", changelog)
                .scrollable(true)
                .h_full()
                .w_full()
                .into_any_element(),
            ChangelogDisplay::Loaded(Some(ChangelogContent::Html(changelog))) => TextView::html("changelog", changelog)
                .scrollable(true)
                .h_full()
                .w_full()
                .into_any_element(),
        };

        v_flex()
            .gap_0p5()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t::instance::content::change_version::changelog()),
            )
            .child(
                div()
                    .h(px(220.0))
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .p_2()
                    .bg(cx.theme().background)
                    .child(body),
            )
            .into_any_element()
    }

    fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let modal = modal.title(self.title.clone()).width(px(560.0));

        if self.items.is_none() {
            self.request_versions(cx);
        }
        let version_select = self.render_version_select(window, cx);

        if let Some(error) = self.versions_error.clone() {
            return modal.child(v_flex().gap_3().child(ErrorAlert::new(t::common::error().into(), error)));
        }

        let selected = self.version_select_state.as_ref().and_then(|state| state.read(cx).selected_value()).cloned();

        if let Some(selected) = &selected {
            self.request_changelog(&selected.key, cx);
        }

        let selected_date = selected.as_ref().and_then(|selected| selected.date_published.clone());
        let is_downgrade = match (&selected_date, &self.current_date_published) {
            (Some(selected_date), Some(current_date)) => selected_date < current_date,
            _ => false,
        };

        let same_version = selected.as_ref().and_then(|selected| selected.sha1.as_deref()) == Some(self.installed_sha1.as_ref());

        let label: SharedString = match (&selected, is_downgrade) {
            (Some(selected), true) => t::instance::content::change_version::downgrade_to(selected.raw_name.as_str()).into(),
            (Some(selected), false) => t::instance::content::change_version::update_to(selected.raw_name.as_str()).into(),
            (None, _) => t::common::update().into(),
        };

        let action_button = Button::new("change_version_action")
            .label(label)
            .disabled(selected.is_none() || same_version)
            .when(is_downgrade, |button| button.warning())
            .when(!is_downgrade, |button| button.success())
            .when_some(selected.clone(), |button, selected| button.on_click({
                let instance_id = self.instance_id;
                let installed_path = self.installed_path.clone();
                let enabled = self.enabled;
                let content_source = self.content_source.clone();
                let backend_handle = self.data.backend_handle.clone();
                let loader = self.loader;
                let minecraft_version = self.minecraft_version;
                let updating = self.updating.clone();
                let content_list = self.content_list.clone();
                let filename_hash = self.filename_hash;

                move |_, window, cx| {
                    if !is_project(&content_source) {
                        return;
                    }

                    let install_file = match &selected.source {
                        VersionSource::Modrinth(version) => {
                            let Some(file) = version.files.iter().find(|file| file.primary).or_else(|| version.files.first()) else {
                                return;
                            };

                            let mut hash = [0_u8; 20];
                            if hex::decode_to_slice(&*file.hashes.sha1, &mut hash).is_err() {
                                let warning = t::instance::content::install::file_invalid_sha1(&file.filename, &file.hashes.sha1);
                                window.push_notification((NotificationType::Error, warning), cx);
                                return;
                            }

                            let mut path = installed_path.with_file_name(&*file.filename);
                            if !enabled {
                                path.add_extension("disabled");
                            }

                            ContentInstallFile {
                                replace_old: Some(installed_path.clone()),
                                path: ContentInstallPath::Raw(path.into()),
                                download: ContentDownload::Url {
                                    url: file.url.clone(),
                                    sha1: hash,
                                    size: file.size,
                                },
                                content_source: content_source.clone(),
                                reason: ContentInstallReason::Update,
                            }
                        },
                        VersionSource::Curseforge(file) => {
                            let Some(sha1) = file.hashes.iter().find(|hash| hash.algo == 1).map(|hash| &hash.value) else {
                                window.push_notification((NotificationType::Error, t::instance::content::install::missing_sha1_hash()), cx);
                                return;
                            };

                            let mut hash = [0_u8; 20];
                            if hex::decode_to_slice(&**sha1, &mut hash).is_err() {
                                let warning = t::instance::content::install::file_invalid_sha1(&file.file_name, sha1);
                                window.push_notification((NotificationType::Error, warning), cx);
                                return;
                            }

                            let Some(url) = file.download_url.clone() else {
                                let warning = t::instance::content::install::no_third_party_downloads();
                                window.push_notification((NotificationType::Error, warning), cx);
                                return;
                            };

                            let mut path = installed_path.with_file_name(&*file.file_name);
                            if !enabled {
                                path.add_extension("disabled");
                            }

                            ContentInstallFile {
                                replace_old: Some(installed_path.clone()),
                                path: ContentInstallPath::Raw(path.into()),
                                download: ContentDownload::Url {
                                    url,
                                    sha1: hash,
                                    size: file.file_length as usize,
                                },
                                content_source: content_source.clone(),
                                reason: ContentInstallReason::Update,
                            }
                        },
                    };

                    let content_install = ContentInstall {
                        target: InstallTarget::Instance(instance_id),
                        loader,
                        minecraft_version,
                        files: vec![install_file].into(),
                    };

                    updating.lock().insert(filename_hash);
                    content_list.update(cx, |_, cx| cx.notify());
                    window.close_dialog(cx);
                    root::start_install(content_install, &backend_handle, window, cx);
                }
            }));

        let buttons = h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(Button::new("cancel").label(t::common::cancel()).on_click(|_, window, cx| {
                window.close_dialog(cx);
            }))
            .child(action_button);

        let changelog_area = self.render_changelog_area(selected.as_ref().map(|selected| &selected.key), cx);

        if let Some(error) = self.changelog_error.clone() {
            return modal.child(v_flex().gap_3().child(ErrorAlert::new(t::common::error().into(), error)));
        }

        let content = v_flex().gap_2()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.version_label.clone()),
                    )
                    .child(version_select),
            )
            .child(
                Checkbox::new("show_incompatible")
                    .label(t::instance::content::change_version::show_incompatible())
                    .checked(self.show_incompatible)
                    .disabled(self.version_select_state.is_none())
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.show_incompatible = *checked;
                        this.items = None;
                        this.version_select_state = None;
                        cx.notify();
                    })),
            )
            .child(changelog_area)
            .child(buttons);

        modal.child(content)
    }
}
