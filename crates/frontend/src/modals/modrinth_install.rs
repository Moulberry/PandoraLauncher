use std::{cmp::Ordering, sync::Arc};

use bridge::{install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::InstanceID, meta::MetadataRequest, safe_path::SafePath};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable, button::{Button, ButtonVariants}, checkbox::Checkbox, dialog::Dialog, h_flex, notification::NotificationType, select::{SearchableVec, Select, SelectItem, SelectState}, spinner::Spinner, v_flex, Disableable, IndexPath, WindowExt
};
use relative_path::RelativePath;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{
    content::ContentSource, loader::Loader, modrinth::{
        ModrinthDependency, ModrinthDependencyType, ModrinthLoader, ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectType, ModrinthProjectVersion, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthVersionStatus, ModrinthVersionType
    }
};

use crate::{
    component::{error_alert::ErrorAlert, instance_dropdown::InstanceDropdown},
    entity::{
        DataEntities, instance::InstanceEntry, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}
    },
    icon::PandoraIcon, root, ts,
};

struct VersionMatrixLoaders {
    loaders: EnumSet<ModrinthLoader>,
    same_loaders_for_all_versions: bool,
}

struct InstallDialog {
    title: SharedString,
    name: SharedString,

    project_versions: Arc<[ModrinthProjectVersion]>,
    data: DataEntities,
    project_type: ModrinthProjectType,
    project_id: Arc<str>,

    version_matrix: FxHashMap<&'static str, VersionMatrixLoaders>,
    instances: Option<Entity<SelectState<InstanceDropdown>>>,
    unsupported_instances: usize,

    target: Option<InstallTarget>,

    last_selected_minecraft_version: Option<SharedString>,
    last_selected_loader: Option<SharedString>,

    fixed_minecraft_version: Option<&'static str>,
    minecraft_version_select_state: Option<Entity<SelectState<SearchableVec<SharedString>>>>,

    fixed_loader: Option<ModrinthLoader>,
    loader_select_state: Option<Entity<SelectState<Vec<SharedString>>>>,
    skip_loader_check_for_mod_version: bool,
    install_dependencies: bool,
    dep_selection: FxHashSet<SharedString>,
    dep_keys: FxHashSet<SharedString>,
    dependency_projects: FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    dep_project_versions: FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    show_dependency_list: bool,

    mod_version_select_state: Option<Entity<SelectState<SearchableVec<ModVersionItem>>>>,
}

pub fn open(
    name: &str,
    project_id: Arc<str>,
    project_type: ModrinthProjectType,
    install_for: Option<InstanceID>,
    data: &DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let project_versions = FrontendMetadata::request(
        &data.metadata,
        MetadataRequest::ModrinthProjectVersions(ModrinthProjectVersionsRequest {
            project_id: project_id.clone(),
            game_versions: None,
            loaders: None,
        }),
        cx,
    );

    open_from_entity(SharedString::new(name), project_versions, project_id, project_type, install_for, data.clone(), window, cx);
}

pub fn open_latest(
    name: &str,
    project_id: Arc<str>,
    project_type: ModrinthProjectType,
    install_for: InstanceID,
    data: &DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let project_versions = FrontendMetadata::request(
        &data.metadata,
        MetadataRequest::ModrinthProjectVersions(ModrinthProjectVersionsRequest {
            project_id: project_id.clone(),
            game_versions: None,
            loaders: None,
        }),
        cx,
    );

    open_latest_from_entity(
        SharedString::new(name),
        project_versions,
        project_id,
        project_type,
        install_for,
        data.clone(),
        window,
        cx,
    );
}

fn open_from_entity(
    name: SharedString,
    project_versions: Entity<FrontendMetadataState>,
    project_id: Arc<str>,
    project_type: ModrinthProjectType,
    install_for: Option<InstanceID>,
    data: DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let title = ts!("instance.content.install.title", name = name);

    let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = project_versions.read(cx).result();
    match result {
        FrontendMetadataResult::Loading => {
            let _subscription = window.observe(&project_versions, cx, move |project_versions, window, cx| {
                window.close_all_dialogs(cx);
                open_from_entity(name.clone(), project_versions, project_id.clone(), project_type, install_for, data.clone(), window, cx);
            });
            window.open_dialog(cx, move |dialog, _, _| {
                let _ = &_subscription;
                dialog.title(title.clone()).child(h_flex().gap_2().child(ts!("instance.content.load.versions.title")).child(Spinner::new()))
            });
        },
        FrontendMetadataResult::Loaded(versions) => {
            let mut valid_project_versions = Vec::with_capacity(versions.0.len());

            let mut version_matrix: FxHashMap<&'static str, VersionMatrixLoaders> = FxHashMap::default();
            for version in versions.0.iter() {
                let Some(loaders) = version.loaders.clone() else {
                    continue;
                };
                let Some(game_versions) = &version.game_versions else {
                    continue;
                };
                if version.files.is_empty() {
                    continue;
                }
                if let Some(status) = version.status
                    && !matches!(status, ModrinthVersionStatus::Listed | ModrinthVersionStatus::Archived)
                {
                    continue;
                }

                let mut loaders = EnumSet::from_iter(loaders.iter().copied());
                loaders.remove(ModrinthLoader::Unknown);
                if loaders.is_empty() {
                    continue;
                }

                valid_project_versions.push(version.clone());

                for game_version in game_versions.iter() {
                    match version_matrix.entry(game_version.as_str()) {
                        std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                            occupied_entry.get_mut().same_loaders_for_all_versions &=
                                occupied_entry.get().loaders == loaders;
                            occupied_entry.get_mut().loaders |= loaders;
                        },
                        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                            vacant_entry.insert(VersionMatrixLoaders {
                                loaders,
                                same_loaders_for_all_versions: true,
                            });
                        },
                    }
                }
            }

            if version_matrix.is_empty() {
                open_error_dialog(title.clone(), ts!("instance.content.load.versions.not_found"), window, cx);
                return;
            }
            if let Some(install_for) = install_for {
                let Some(instance) = data.instances.read(cx).entries.get(&install_for) else {
                    open_error_dialog(title.clone(), ts!("instance.unable_to_find"), window, cx);
                    return;
                };

                let instance = instance.read(cx);

                let minecraft_version = instance.configuration.minecraft_version.as_str();
                let instance_loader = instance.configuration.loader;

                let Some(loaders) = version_matrix.get(minecraft_version) else {
                    let error_message = ts!("instance.content.load.versions.not_found_for", ver = minecraft_version);
                    open_error_dialog(title.clone(), error_message, window, cx);
                    return;
                };

                let mut valid_loader = true;
                if project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack {
                    valid_loader = instance_loader == Loader::Vanilla
                        || loaders.loaders.contains(instance_loader.as_modrinth_loader());
                }
                if !valid_loader {
                    let error_message = ts!("instance.content.load.versions.not_found_for", ver = format!("{} {}", instance_loader.name(), minecraft_version));
                    open_error_dialog(title.clone(), error_message, window, cx);
                    return;
                }

                let title = title.clone();
                let instance_id = instance.id;
                let fixed_minecraft_version = Some(minecraft_version);
                let fixed_loader = if (project_type == ModrinthProjectType::Mod
                    || project_type == ModrinthProjectType::Modpack)
                    && instance_loader != Loader::Vanilla
                {
                    Some(instance_loader.as_modrinth_loader())
                } else {
                    None
                };
                let install_dialog = InstallDialog {
                    title,
                    name: name.into(),
                    project_versions: valid_project_versions.into(),
                    data,
                    project_type,
                    project_id,
                    version_matrix,
                    instances: None,
                    unsupported_instances: 0,
                    target: Some(InstallTarget::Instance(instance_id)),
                    fixed_minecraft_version,
                    minecraft_version_select_state: None,
                    fixed_loader,
                    loader_select_state: None,
                    last_selected_minecraft_version: None,
                    skip_loader_check_for_mod_version: false,
                    install_dependencies: true,
                    dep_selection: Default::default(),
                    dep_keys: Default::default(),
                    dependency_projects: Default::default(),
                    dep_project_versions: Default::default(),
                    show_dependency_list: false,
                    mod_version_select_state: None,
                    last_selected_loader: None,
                };
                install_dialog.show(window, cx);
            } else {
                let instance_entries = data.instances.clone();

                let entries: Arc<[InstanceEntry]> = instance_entries
                    .read(cx)
                    .entries
                    .iter()
                    .filter_map(|(_, instance)| {
                        let instance = instance.read(cx);

                        let minecraft_version = instance.configuration.minecraft_version.as_str();
                        let instance_loader = instance.configuration.loader;

                        if let Some(loaders) = version_matrix.get(minecraft_version) {
                            let mut valid_loader = true;
                            if project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack {
                                valid_loader = instance_loader == Loader::Vanilla
                                    || loaders.loaders.contains(instance_loader.as_modrinth_loader());
                            }
                            if valid_loader {
                                return Some(instance.clone());
                            }
                        }

                        None
                    })
                    .collect();

                let unsupported_instances = instance_entries.read(cx).entries.len().saturating_sub(entries.len());
                let instances = if !entries.is_empty() {
                    let dropdown = InstanceDropdown::create(entries, window, cx);
                    dropdown.update(cx, |dropdown, cx| {
                        dropdown.set_selected_index(Some(IndexPath::default()), window, cx)
                    });
                    Some(dropdown)
                } else {
                    None
                };

                let install_dialog = InstallDialog {
                    title,
                    name: name.into(),
                    project_versions: valid_project_versions.into(),
                    data,
                    project_type,
                    project_id,
                    version_matrix,
                    instances,
                    unsupported_instances,
                    target: None,
                    fixed_minecraft_version: None,
                    minecraft_version_select_state: None,
                    fixed_loader: None,
                    loader_select_state: None,
                    last_selected_minecraft_version: None,
                    skip_loader_check_for_mod_version: false,
                    install_dependencies: true,
                    dep_selection: Default::default(),
                    dep_keys: Default::default(),
                    dependency_projects: Default::default(),
                    dep_project_versions: Default::default(),
                    show_dependency_list: false,
                    mod_version_select_state: None,
                    last_selected_loader: None,
                };
                install_dialog.show(window, cx);
            }
        },
        FrontendMetadataResult::Error(message) => {
            window.open_dialog(cx, move |modal, _, _| {
                modal.title(title.clone()).child(ErrorAlert::new(ts!("instance.content.requesting_from_modrinth_error"), message.clone()))
            });
        },
    }
}

fn open_latest_from_entity(
    name: SharedString,
    project_versions: Entity<FrontendMetadataState>,
    project_id: Arc<str>,
    project_type: ModrinthProjectType,
    install_for: InstanceID,
    data: DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let title = ts!("instance.content.install.title", name = name);

    let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = project_versions.read(cx).result();
    match result {
        FrontendMetadataResult::Loading => {
            let _subscription = window.observe(&project_versions, cx, move |project_versions, window, cx| {
                window.close_all_dialogs(cx);
                open_latest_from_entity(
                    name.clone(),
                    project_versions,
                    project_id.clone(),
                    project_type,
                    install_for,
                    data.clone(),
                    window,
                    cx,
                );
            });
            window.open_dialog(cx, move |dialog, _, _| {
                let _ = &_subscription;
                dialog.title(title.clone()).child(h_flex().gap_2().child(ts!("instance.content.load.versions.title")).child(Spinner::new()))
            });
        },
        FrontendMetadataResult::Loaded(versions) => {
            let Some(instance) = data.instances.read(cx).entries.get(&install_for) else {
                open_error_dialog(title.clone(), ts!("instance.unable_to_find"), window, cx);
                return;
            };

            let instance = instance.read(cx);
            let minecraft_version = instance.configuration.minecraft_version.as_str();
            let instance_loader = instance.configuration.loader;

            let selected_version = select_latest_modrinth_version(
                &versions.0,
                instance_loader,
                minecraft_version,
                project_type,
            );

            let Some(selected_version) = selected_version else {
                let error_message = ts!("instance.content.load.versions.not_found_for", ver = minecraft_version);
                open_error_dialog(title.clone(), error_message, window, cx);
                return;
            };

            let required_dependencies = required_modrinth_dependencies(
                Some(&selected_version),
                Some(InstallTarget::Instance(instance.id)),
                &data,
                cx,
            );

            let target = InstallTarget::Instance(instance.id);
            let installed_projects = modrinth_installed_projects(Some(&target), &data, cx);
            let selected_dependencies = required_dependencies
                .into_iter()
                .filter(|dep| dep.project_id.as_ref().map(|id| !installed_projects.contains(id)).unwrap_or(false))
                .collect::<Vec<_>>();

            start_modrinth_install(
                &data,
                project_type,
                &project_id,
                &selected_version,
                target,
                instance_loader,
                minecraft_version,
                selected_dependencies,
                window,
                cx,
            );
        },
        FrontendMetadataResult::Error(message) => {
            window.open_dialog(cx, move |modal, _, _| {
                modal.title(title.clone()).child(ErrorAlert::new(ts!("instance.content.requesting_from_modrinth_error"), message.clone()))
            });
        },
    }
}

fn open_error_dialog(title: SharedString, text: SharedString, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |modal, _, _| {
        modal.title(title.clone()).child(text.clone())
    });
}

impl InstallDialog {
    fn show(self, window: &mut Window, cx: &mut App) {
        let install_dialog = cx.new(|_| self);
        window.open_dialog(cx, move |modal, window, cx| {
            install_dialog.update(cx, |this, cx| this.render(modal, window, cx))
        });
    }

    fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
        let modal = modal.title(self.title.clone());

        if self.target.is_none() {
            let create_instance_label = match self.project_type {
                ModrinthProjectType::Mod => ts!("instance.content.install.new_instance_with.mod"),
                ModrinthProjectType::Modpack => ts!("instance.content.install.new_instance_with.modpack"),
                ModrinthProjectType::Resourcepack => ts!("instance.content.install.new_instance_with.resourcepack"),
                ModrinthProjectType::Shader => ts!("instance.content.install.new_instance_with.shader"),
                ModrinthProjectType::Other => ts!("instance.content.install.new_instance_with.file"),
            };

            let content = v_flex()
                .gap_2()
                .text_center()
                .when_some(self.instances.as_ref(), |content, instances| {
                    let read_instances = instances.read(cx);
                    let selected_instance: Option<InstanceEntry> = read_instances.selected_value().cloned();

                    let button_and_dropdown = h_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .w_full()
                                .gap_0p5()
                                .child(
                                    Select::new(instances).placeholder(ts!("instance.none_selected")).title_prefix(format!("{}: ", ts!("instance.label"))),
                                )
                                .when(self.unsupported_instances > 0, |content| {
                                    content
                                        .child(ts!("instance.incompatible", num = self.unsupported_instances))
                                }),
                        )
                        .when_some(selected_instance, |dialog, instance| {
                            dialog.child(Button::new("instance").success().h_full().label(ts!("instance.content.install.add_to_instance")).on_click(
                                cx.listener(move |this, _, _, _| {
                                    this.target = Some(InstallTarget::Instance(instance.id));
                                    this.fixed_minecraft_version = Some(instance.configuration.minecraft_version.as_str());
                                    if (this.project_type == ModrinthProjectType::Mod
                                        || this.project_type == ModrinthProjectType::Modpack)
                                        && instance.configuration.loader != Loader::Vanilla
                                    {
                                        this.fixed_loader = Some(instance.configuration.loader.as_modrinth_loader());
                                    }
                                }),
                            ))
                        });

                    content.child(button_and_dropdown).child(format!("— {} —", ts!("common.or_upper")))
                })
                .child(Button::new("create").success().label(create_instance_label).on_click(cx.listener(
                    |this, _, _, _| {
                        this.target = Some(InstallTarget::NewInstance {
                            name: None,
                        });
                    },
                )));

            return modal.child(content);
        }

        if self.minecraft_version_select_state.is_none() {
            if let Some(minecraft_version) = self.fixed_minecraft_version.clone() {
                self.minecraft_version_select_state = Some(cx.new(|cx| {
                    let mut select_state =
                        SelectState::new(SearchableVec::new(vec![SharedString::new_static(minecraft_version)]), None, window, cx)
                            .searchable(true);
                    select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                    select_state
                }));
            } else {
                let mut keys: Vec<SharedString> =
                    self.version_matrix.keys().cloned().map(SharedString::new_static).collect();
                keys.sort_by(|a, b| {
                    let a_is_snapshot = a.contains("w") || a.contains("pre") || a.contains("rc");
                    let b_is_snapshot = b.contains("w") || b.contains("pre") || b.contains("rc");
                    if a_is_snapshot != b_is_snapshot {
                        if a_is_snapshot {
                            Ordering::Greater
                        } else {
                            Ordering::Less
                        }
                    } else {
                        lexical_sort::natural_lexical_cmp(a, b).reverse()
                    }
                });
                self.minecraft_version_select_state = Some(cx.new(|cx| {
                    let mut select_state =
                        SelectState::new(SearchableVec::new(keys), None, window, cx).searchable(true);
                    select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                    select_state
                }));
            }
        }

        let selected_minecraft_version = self
            .minecraft_version_select_state
            .as_ref()
            .and_then(|v| v.read(cx).selected_value())
            .cloned();
        let game_version_changed = self.last_selected_minecraft_version != selected_minecraft_version;
        self.last_selected_minecraft_version = selected_minecraft_version.clone();

        if self.loader_select_state.is_none() || game_version_changed {
            self.last_selected_minecraft_version = selected_minecraft_version.clone();
            self.skip_loader_check_for_mod_version = false;

            if let Some(loader) = self.fixed_loader {
                let loader = SharedString::new_static(loader.pretty_name());
                self.loader_select_state = Some(cx.new(|cx| {
                    let mut select_state = SelectState::new(vec![loader], None, window, cx);
                    select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                    select_state
                }));
            } else if let Some(selected_minecraft_version) = selected_minecraft_version.clone()
                && let Some(loaders) = self.version_matrix.get(selected_minecraft_version.as_str())
            {
                if loaders.same_loaders_for_all_versions {
                    let single_loader = if loaders.loaders.len() == 1 {
                        SharedString::new_static(loaders.loaders.iter().next().unwrap().pretty_name())
                    } else {
                        let mut string = String::new();
                        let mut first = true;
                        for loader in loaders.loaders.iter() {
                            if first {
                                first = false;
                            } else {
                                string.push_str(" / ");
                            }
                            string.push_str(loader.pretty_name());
                        }
                        SharedString::new(string)
                    };

                    self.skip_loader_check_for_mod_version = true;
                    self.loader_select_state = Some(cx.new(|cx| {
                        let mut select_state = SelectState::new(vec![single_loader], None, window, cx);
                        select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                        select_state
                    }));
                } else {
                    let keys: Vec<SharedString> =
                        loaders.loaders.iter().map(ModrinthLoader::pretty_name).map(SharedString::new_static).collect();

                    let previous = self
                        .loader_select_state
                        .as_ref()
                        .and_then(|state| state.read(cx).selected_value().cloned());
                    self.loader_select_state = Some(cx.new(|cx| {
                        let mut select_state = SelectState::new(keys, None, window, cx);
                        if let Some(previous) = previous {
                            select_state.set_selected_value(&previous, window, cx);
                        }
                        if select_state.selected_index(cx).is_none() {
                            select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                        }
                        select_state
                    }));
                }
            }
            if self.loader_select_state.is_none() {
                self.loader_select_state = Some(cx.new(|cx| {
                    let mut select_state = SelectState::new(Vec::new(), None, window, cx);
                    select_state.set_selected_index(Some(IndexPath::default()), window, cx);
                    select_state
                }));
            }
        }

        let selected_loader = self.loader_select_state.as_ref().and_then(|v| v.read(cx).selected_value()).cloned();
        let loader_changed = self.last_selected_loader != selected_loader;
        self.last_selected_loader = selected_loader.clone();

        if (self.mod_version_select_state.is_none() || game_version_changed || loader_changed)
            && let Some(selected_game_version) = selected_minecraft_version.clone()
            && let Some(selected_loader) = self.last_selected_loader.clone()
        {
            let selected_game_version = selected_game_version.as_str();

            let selected_loader = if self.skip_loader_check_for_mod_version {
                None
            } else {
                Some(ModrinthLoader::from_name(selected_loader.as_str()))
            };

            let mod_versions: Vec<ModVersionItem> = self
                .project_versions
                .iter()
                .filter_map(|version| {
                    let Some(game_versions) = &version.game_versions else {
                        return None;
                    };
                    let Some(loaders) = &version.loaders else {
                        return None;
                    };
                    if version.files.is_empty() {
                        return None;
                    }
                    let matches_game_version = game_versions.iter().any(|v| v.as_str() == selected_game_version);
                    let matches_loader = if let Some(selected_loader) = selected_loader {
                        loaders.contains(&selected_loader)
                    } else {
                        true
                    };
                    if matches_game_version && matches_loader {
                        let name = version
                            .version_number
                            .clone()
                            .unwrap_or(version.name.clone().unwrap_or(version.id.clone()));
                        let mut name = SharedString::new(name);

                        match version.version_type {
                            Some(ModrinthVersionType::Beta) => name = ts!("modrinth.versions.beta", name = name),
                            Some(ModrinthVersionType::Alpha) => name = ts!("modrinth.versions.alpha", name = name),
                            _ => {},
                        }

                        Some(ModVersionItem {
                            name,
                            version: version.clone(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let mut highest_release = None;
            let mut highest_beta = None;
            let mut highest_alpha = None;

            for (index, version) in mod_versions.iter().enumerate() {
                match version.version.version_type {
                    Some(ModrinthVersionType::Release) => {
                        highest_release = Some(index);
                        break;
                    },
                    Some(ModrinthVersionType::Beta) => {
                        if highest_beta.is_none() {
                            highest_beta = Some(index);
                        }
                    },
                    Some(ModrinthVersionType::Alpha) => {
                        if highest_alpha.is_none() {
                            highest_alpha = Some(index);
                        }
                    },
                    _ => {},
                }
            }

            let highest = highest_release.or(highest_beta).or(highest_alpha);

            self.mod_version_select_state = Some(cx.new(|cx| {
                let mut select_state =
                    SelectState::new(SearchableVec::new(mod_versions), None, window, cx).searchable(true);
                if let Some(index) = highest {
                    select_state.set_selected_index(Some(IndexPath::default().row(index)), window, cx);
                }
                select_state
            }));
        }

        let selected_mod_version = self
            .mod_version_select_state
            .as_ref()
            .and_then(|state| state.read(cx).selected_value())
            .cloned();

        let mod_version_prefix = match self.project_type {
            ModrinthProjectType::Mod => format!("{}: ", ts!("instance.content.version.mod")),
            ModrinthProjectType::Modpack => format!("{}: ", ts!("instance.content.version.modpack")),
            ModrinthProjectType::Resourcepack => format!("{}: ", ts!("instance.content.version.resourcepack")),
            ModrinthProjectType::Shader => format!("{}: ", ts!("instance.content.version.shader")),
            ModrinthProjectType::Other => format!("{}: ", ts!("instance.content.version.file")),
        };

        let required_dependencies = required_modrinth_dependencies(
            selected_mod_version.as_ref(),
            self.target.clone(),
            &self.data,
            cx,
        );

        let installed_projects = modrinth_installed_projects(self.target.as_ref(), &self.data, cx);
        let loader_hint = loader_for_selection(selected_loader.as_ref());
        let version_hint = selected_minecraft_version.as_ref().map(|v| v.as_str()).unwrap_or_default();
        let dep_tree = build_modrinth_dep_tree(
            &required_dependencies,
            &self.data,
            &mut self.dependency_projects,
            &mut self.dep_project_versions,
            &installed_projects,
            cx,
            loader_hint,
            version_hint,
        );
        let flat_deps = flatten_modrinth_deps(&dep_tree);
        let selectable_keys: Vec<SharedString> = flat_deps.iter().map(|dep| dep.key.clone()).collect();

        let required_keys: FxHashSet<SharedString> = selectable_keys.iter().cloned().collect();
        if required_keys != self.dep_keys {
            self.dep_keys = required_keys.clone();
            self.dep_selection = required_keys;
        }
        self.install_dependencies = !self.dep_selection.is_empty();

        let border = cx.theme().border;
        let content = v_flex()
            .gap_2()
            .child(
                Select::new(self.minecraft_version_select_state.as_ref().unwrap())
                    .disabled(self.fixed_minecraft_version.is_some())
                    .title_prefix(format!("{}: ", ts!("instance.game_version"))),
            )
            .child(
                Select::new(self.loader_select_state.as_ref().unwrap())
                    .disabled(self.fixed_loader.is_some() || self.skip_loader_check_for_mod_version)
                    .title_prefix(format!("{}: ", ts!("instance.loader"))),
            )
            .when_some(self.mod_version_select_state.as_ref(), |modal, mod_versions| {
                modal
                    .child(Select::new(mod_versions).title_prefix(mod_version_prefix))
                    .when(!flat_deps.is_empty(), |modal| {
                        let dep_ui = modrinth_dep_ui_state(&flat_deps, &self.dep_selection);
                        let dep_list = build_modrinth_dep_list(&flat_deps, &self.dep_selection, "install_dep", cx);
                        let header = build_modrinth_dep_header(
                            dep_ui.label,
                            dep_ui.selectable_keys,
                            dep_ui.selected_count,
                            dep_ui.total_selectable,
                            self.show_dependency_list,
                            "install_deps_select_all",
                            "install_deps_select_all_btn",
                            "install_deps_toggle",
                            cx,
                        );

                        modal
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(header)
                                    .child(div().flex_grow())
                            )
                            .when(self.show_dependency_list, |modal| modal.child(dep_list))
                    })
                    .child(div().border_t_1().border_color(border).mt_2())
                    .child(Button::new("install").success().label(ts!("instance.content.install.label")).on_click(cx.listener(
                        move |this, _, window, cx| {
                            let Some(selected_mod_version) = selected_mod_version.as_ref() else {
                                window.push_notification((NotificationType::Error, ts!("instance.content.install.no_mod_version_selected")), cx);
                                return;
                            };

                            let install_file = selected_mod_version
                                .files
                                .iter()
                                .find(|file| file.primary)
                                .unwrap_or(selected_mod_version.files.first().unwrap());

                            let path = match this.project_type {
                                ModrinthProjectType::Mod => RelativePath::new("mods").join(&*install_file.filename),
                                ModrinthProjectType::Modpack => RelativePath::new("mods").join(&*install_file.filename),
                                ModrinthProjectType::Resourcepack => RelativePath::new("resourcepacks").join(&*install_file.filename),
                                ModrinthProjectType::Shader => RelativePath::new("shaderpacks").join(&*install_file.filename),
                                ModrinthProjectType::Other => {
                                    window.push_notification((NotificationType::Error, ts!("instance.content.install.unable_install_other")), cx);
                                    return;
                                },
                            };

                            let Some(path) = SafePath::from_relative_path(&path) else {
                                window.push_notification((NotificationType::Error, ts!("instance.content.install.invalid_filename")), cx);
                                return;
                            };

                            let mut target = this.target.clone().unwrap();

                            let mut loader_hint = Loader::Unknown;
                            if let Some(selected_loader) = &selected_loader {
                                let modrinth_loader = ModrinthLoader::from_name(selected_loader);
                                match modrinth_loader {
                                    ModrinthLoader::Fabric => loader_hint = Loader::Fabric,
                                    ModrinthLoader::Forge => loader_hint = Loader::Forge,
                                    ModrinthLoader::NeoForge => loader_hint = Loader::NeoForge,
                                    _ => {}
                                }
                            }

                            let mut version_hint = None;
                            if let Some(selected_minecraft_version) = &selected_minecraft_version {
                                version_hint = Some(selected_minecraft_version.as_str().into());
                            }

                            if let InstallTarget::NewInstance { name } = &mut target {
                                *name = Some(this.name.as_str().into());
                            }

                            let mut files = Vec::new();

                            if this.install_dependencies {
                                for dep in required_dependencies.iter() {
                                    let key = modrinth_dep_key(dep);
                                    if !this.dep_selection.contains(&key) {
                                        continue;
                                    }
                                    files.push(ContentInstallFile {
                                        replace_old: None,
                                        path: bridge::install::ContentInstallPath::Automatic,
                                        download: ContentDownload::Modrinth {
                                            project_id: dep.project_id.clone().unwrap(),
                                            version_id: dep.version_id.clone(),
                                            install_dependencies: true,
                                        },
                                        content_source: ContentSource::ModrinthProject { project_id: dep.project_id.clone().unwrap() },
                                    })
                                }
                            }

                            files.push(ContentInstallFile {
                                replace_old: None,
                                path: bridge::install::ContentInstallPath::Safe(path),
                                download: ContentDownload::Url {
                                    url: install_file.url.clone(),
                                    sha1: install_file.hashes.sha1.clone(),
                                    size: install_file.size,
                                },
                                content_source: ContentSource::ModrinthProject {
                                    project_id: this.project_id.clone()
                                },
                            });

                            let content_install = ContentInstall {
                                target,
                                loader_hint,
                                version_hint,
                                files: files.into(),
                            };

                            window.close_dialog(cx);
                            root::start_install(content_install, &this.data.backend_handle, window, cx);
                        },
                    )),
                )
            });

        modal.child(content)
    }
}

#[derive(Clone)]
struct ModVersionItem {
    name: SharedString,
    version: ModrinthProjectVersion,
}

impl SelectItem for ModVersionItem {
    type Value = ModrinthProjectVersion;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.version
    }
}

#[derive(Clone)]
struct ModrinthDepNode {
    key: SharedString,
    label: SharedString,
    installed: bool,
    children: Vec<ModrinthDepNode>,
}

#[derive(Clone)]
struct ModrinthFlatDep {
    key: SharedString,
    label: SharedString,
}

fn modrinth_dep_key(dep: &ModrinthDependency) -> SharedString {
    let project_id = dep.project_id.as_deref().unwrap_or("unknown");
    if let Some(version_id) = dep.version_id.as_ref() {
        format!("{}:{}", project_id, version_id).into()
    } else {
        project_id.to_string().into()
    }
}

fn modrinth_dep_fallback(dep: &ModrinthDependency) -> SharedString {
    if let Some(file_name) = dep.file_name.as_deref() {
        return SharedString::new(file_name.to_string());
    }
    let project_id = dep.project_id.as_deref().unwrap_or("unknown");
    if let Some(version_id) = dep.version_id.as_ref() {
        format!("{} ({})", project_id, version_id).into()
    } else {
        project_id.to_string().into()
    }
}

fn modrinth_dep_display(
    dep: &ModrinthDependency,
    data: &DataEntities,
    dependency_projects: &mut FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    cx: &mut App,
) -> SharedString {
    if let Some(project_id) = dep.project_id.as_ref() {
        let request = dependency_projects.entry(project_id.clone()).or_insert_with(|| {
            FrontendMetadata::request(
                &data.metadata,
                MetadataRequest::ModrinthProject(ModrinthProjectRequest {
                    project_id: project_id.clone(),
                }),
                cx,
            )
        });

        let result: FrontendMetadataResult<ModrinthProjectResult> = request.read(cx).result();
        if let FrontendMetadataResult::Loaded(project) = result {
            if let Some(title) = project.title.as_ref() {
                return SharedString::new(title.to_string());
            }
        }
    }

    modrinth_dep_fallback(dep)
}

fn loader_for_selection(selected_loader: Option<&SharedString>) -> Loader {
    if let Some(selected_loader) = selected_loader {
        let modrinth_loader = ModrinthLoader::from_name(selected_loader.as_str());
        match modrinth_loader {
            ModrinthLoader::Fabric => Loader::Fabric,
            ModrinthLoader::Forge => Loader::Forge,
            ModrinthLoader::NeoForge => Loader::NeoForge,
            _ => Loader::Unknown,
        }
    } else {
        Loader::Unknown
    }
}

fn build_modrinth_dep_tree(
    deps: &[ModrinthDependency],
    data: &DataEntities,
    dependency_projects: &mut FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    dep_project_versions: &mut FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    installed_projects: &FxHashSet<Arc<str>>,
    cx: &mut App,
    loader_hint: Loader,
    minecraft_version: &str,
) -> Vec<ModrinthDepNode> {
    let mut visited = FxHashSet::default();
    build_modrinth_dep_nodes(
        deps,
        data,
        dependency_projects,
        dep_project_versions,
        installed_projects,
        cx,
        loader_hint,
        minecraft_version,
        &mut visited,
    )
}

fn build_modrinth_dep_nodes(
    deps: &[ModrinthDependency],
    data: &DataEntities,
    dependency_projects: &mut FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    dep_project_versions: &mut FxHashMap<Arc<str>, Entity<FrontendMetadataState>>,
    installed_projects: &FxHashSet<Arc<str>>,
    cx: &mut App,
    loader_hint: Loader,
    minecraft_version: &str,
    visited: &mut FxHashSet<Arc<str>>,
) -> Vec<ModrinthDepNode> {
    let mut nodes = Vec::new();
    for dep in deps.iter() {
        let installed = dep.project_id.as_ref().map(|id| installed_projects.contains(id)).unwrap_or(false);
        let key = dep.project_id.as_ref().map(|id| SharedString::new(id.to_string())).unwrap_or_else(|| modrinth_dep_key(dep));
        let label = modrinth_dep_display(dep, data, dependency_projects, cx);

        let mut children = Vec::new();
        if let Some(project_id) = dep.project_id.as_ref() {
            if visited.insert(project_id.clone()) {
                let request = dep_project_versions.entry(project_id.clone()).or_insert_with(|| {
                    FrontendMetadata::request(
                        &data.metadata,
                        MetadataRequest::ModrinthProjectVersions(ModrinthProjectVersionsRequest {
                            project_id: project_id.clone(),
                            game_versions: None,
                            loaders: None,
                        }),
                        cx,
                    )
                });

                let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = request.read(cx).result();
                if let FrontendMetadataResult::Loaded(versions) = result {
                    if let Some(version) = select_latest_modrinth_version(
                        &versions.0,
                        loader_hint,
                        minecraft_version,
                        ModrinthProjectType::Mod,
                    ) {
                        let required = required_modrinth_dependencies(Some(&version), None, data, cx);
                        children = build_modrinth_dep_nodes(
                            &required,
                            data,
                            dependency_projects,
                            dep_project_versions,
                            installed_projects,
                            cx,
                            loader_hint,
                            minecraft_version,
                            visited,
                        );
                    }
                }
            }
        }

        nodes.push(ModrinthDepNode {
            key,
            label,
            installed,
            children,
        });
    }
    nodes
}

fn flatten_modrinth_deps(nodes: &[ModrinthDepNode]) -> Vec<ModrinthFlatDep> {
    let mut deps = Vec::new();
    flatten_modrinth_deps_inner(nodes, &mut deps);
    deps
}

fn flatten_modrinth_deps_inner(nodes: &[ModrinthDepNode], deps: &mut Vec<ModrinthFlatDep>) {
    for node in nodes {
        if !node.installed {
            deps.push(ModrinthFlatDep {
                key: node.key.clone(),
                label: node.label.clone(),
            });
        }
        if !node.children.is_empty() {
            flatten_modrinth_deps_inner(&node.children, deps);
        }
    }
}

trait ModrinthDepSelection {
    fn set_selected(&mut self, key: SharedString, selected: bool);
    fn set_all_selected(&mut self, keys: &[SharedString], selected: bool);
    fn toggle_dependency_list(&mut self);
}

impl ModrinthDepSelection for InstallDialog {
    fn set_selected(&mut self, key: SharedString, selected: bool) {
        if selected {
            self.dep_selection.insert(key);
        } else {
            self.dep_selection.remove(&key);
        }
    }

    fn set_all_selected(&mut self, keys: &[SharedString], selected: bool) {
        if selected {
            self.dep_selection = keys.iter().cloned().collect();
        } else {
            self.dep_selection.clear();
        }
    }

    fn toggle_dependency_list(&mut self) {
        self.show_dependency_list = !self.show_dependency_list;
    }
}


struct ModrinthDepUiState {
    selectable_keys: Vec<SharedString>,
    total_selectable: usize,
    selected_count: usize,
    label: SharedString,
}

fn modrinth_dep_ui_state(flat_deps: &[ModrinthFlatDep], selection: &FxHashSet<SharedString>) -> ModrinthDepUiState {
    let selectable_keys: Vec<SharedString> = flat_deps.iter().map(|dep| dep.key.clone()).collect();
    let total_selectable = selectable_keys.len();
    let selected_count = selection.len();
    let label = if selected_count == 1 {
        ts!("instance.content.install.install_dependency")
    } else {
        ts!("instance.content.install.install_dependencies", num = selected_count)
    };

    ModrinthDepUiState {
        selectable_keys,
        total_selectable,
        selected_count,
        label,
    }
}

fn build_modrinth_dep_list<T: ModrinthDepSelection + 'static>(
    flat_deps: &[ModrinthFlatDep],
    selection: &FxHashSet<SharedString>,
    id_prefix: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut dep_elements = Vec::new();
    for (index, dep) in flat_deps.iter().enumerate() {
        let key = dep.key.clone();
        let label = dep.label.clone();
        dep_elements.push(
            Checkbox::new(format!("{id_prefix}_{index}"))
                .checked(selection.contains(&key))
                .label(label)
                .on_click(cx.listener(move |dialog, value, _, _| {
                    dialog.set_selected(key.clone(), *value);
                }))
                .into_any_element()
        );
    }

    v_flex().gap_1().pl_3().children(dep_elements).into_any_element()
}

fn build_modrinth_dep_header<T: ModrinthDepSelection + 'static>(
    label: SharedString,
    selectable_keys: Vec<SharedString>,
    selected_count: usize,
    total_selectable: usize,
    show_dependency_list: bool,
    checkbox_id: &'static str,
    button_id: &'static str,
    toggle_id: &'static str,
    cx: &mut Context<T>,
) -> AnyElement {
    let header_checkbox_state = if selected_count == 0 {
        0
    } else if selected_count == total_selectable {
        2
    } else {
        1
    };

    let header_checkbox = {
        let keys = selectable_keys;
        let state = header_checkbox_state;
        let mut checkbox = Checkbox::new(checkbox_id)
            .checked(state == 2)
            .on_click(move |_, _, _| {});

        if total_selectable == 0 {
            checkbox = checkbox.disabled(true);
        }

        let click_handler = cx.listener(move |dialog, _, _, _| {
            if state == 2 {
                dialog.set_all_selected(&keys, false);
            } else {
                dialog.set_all_selected(&keys, true);
            }
        });

        Button::new(button_id)
            .ghost()
            .compact()
            .p_0()
            .min_w_4()
            .min_h_4()
            .child(checkbox)
            .on_click(click_handler)
    };

    let chevron = Button::new(toggle_id)
        .icon(if show_dependency_list { PandoraIcon::ChevronDown } else { PandoraIcon::ChevronRight })
        .ghost()
        .compact()
        .small()
        .on_click(cx.listener(|dialog, _, _, _| {
            dialog.toggle_dependency_list();
        }));

    h_flex()
        .items_center()
        .gap_2()
        .child(header_checkbox)
        .child(label)
        .child(chevron)
        .into_any_element()
}
fn required_modrinth_dependencies(
    selected_mod_version: Option<&ModrinthProjectVersion>,
    _target: Option<InstallTarget>,
    _data: &DataEntities,
    _cx: &App,
) -> Vec<ModrinthDependency> {
    let required_dependencies = selected_mod_version.and_then(|version| {
        version.dependencies.as_ref().map(|deps| {
            let required = deps
                .iter()
                .filter(|dep| {
                    dep.project_id.is_some() && dep.dependency_type == ModrinthDependencyType::Required
                })
                .cloned()
                .collect::<Vec<_>>();

            required
        })
    }).unwrap_or_default();

    required_dependencies
}

fn modrinth_installed_projects(
    target: Option<&InstallTarget>,
    data: &DataEntities,
    cx: &App,
) -> FxHashSet<Arc<str>> {
    let mut existing_projects = FxHashSet::default();
    if let Some(InstallTarget::Instance(instance_id)) = target
        && let Some(instance) = data.instances.read(cx).entries.get(instance_id)
    {
        let existing_mods = instance.read(cx).mods.read(cx);
        for summary in existing_mods.iter() {
            let ContentSource::ModrinthProject { project_id } = &summary.content_source else {
                continue;
            };
            existing_projects.insert(project_id.clone());
        }
    }
    existing_projects
}

fn select_latest_modrinth_version(
    versions: &[ModrinthProjectVersion],
    instance_loader: Loader,
    minecraft_version: &str,
    project_type: ModrinthProjectType,
) -> Option<ModrinthProjectVersion> {
    let mut candidates: Vec<ModrinthProjectVersion> = versions
        .iter()
        .filter(|version| {
            let Some(loaders) = version.loaders.clone() else {
                return false;
            };
            let Some(game_versions) = &version.game_versions else {
                return false;
            };
            if version.files.is_empty() {
                return false;
            }
            if let Some(status) = version.status
                && !matches!(status, ModrinthVersionStatus::Listed | ModrinthVersionStatus::Archived)
            {
                return false;
            }

            let mut loaders = EnumSet::from_iter(loaders.iter().copied());
            loaders.remove(ModrinthLoader::Unknown);
            if loaders.is_empty() {
                return false;
            }

            let matches_game_version = game_versions.iter().any(|v| v.as_str() == minecraft_version);

            if !matches_game_version {
                return false;
            }

            if project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack {
                if instance_loader != Loader::Vanilla {
                    return loaders.contains(instance_loader.as_modrinth_loader());
                }
            }

            true
        })
        .cloned()
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let mut highest_release = None;
    let mut highest_beta = None;
    let mut highest_alpha = None;

    for (index, version) in candidates.iter().enumerate() {
        match version.version_type {
            Some(ModrinthVersionType::Release) => {
                highest_release = Some(index);
                break;
            },
            Some(ModrinthVersionType::Beta) => {
                if highest_beta.is_none() {
                    highest_beta = Some(index);
                }
            },
            Some(ModrinthVersionType::Alpha) => {
                if highest_alpha.is_none() {
                    highest_alpha = Some(index);
                }
            },
            _ => {},
        }
    }

    let highest = highest_release.or(highest_beta).or(highest_alpha);
    highest.map(|index| candidates.swap_remove(index))
}

fn start_modrinth_install(
    data: &DataEntities,
    project_type: ModrinthProjectType,
    project_id: &Arc<str>,
    selected_version: &ModrinthProjectVersion,
    target: InstallTarget,
    loader_hint: Loader,
    minecraft_version: &str,
    selected_dependencies: Vec<ModrinthDependency>,
    window: &mut Window,
    cx: &mut App,
) {
    let install_file = selected_version
        .files
        .iter()
        .find(|file| file.primary)
        .unwrap_or(selected_version.files.first().unwrap());

    let path = match project_type {
        ModrinthProjectType::Mod => RelativePath::new("mods").join(&*install_file.filename),
        ModrinthProjectType::Modpack => RelativePath::new("mods").join(&*install_file.filename),
        ModrinthProjectType::Resourcepack => RelativePath::new("resourcepacks").join(&*install_file.filename),
        ModrinthProjectType::Shader => RelativePath::new("shaderpacks").join(&*install_file.filename),
        ModrinthProjectType::Other => {
            window.push_notification((NotificationType::Error, ts!("instance.content.install.unable_install_other")), cx);
            return;
        },
    };

    let Some(path) = SafePath::from_relative_path(&path) else {
        window.push_notification((NotificationType::Error, ts!("instance.content.install.invalid_filename")), cx);
        return;
    };

    let mut files = Vec::new();

    for dep in selected_dependencies.iter() {
        files.push(ContentInstallFile {
            replace_old: None,
            path: bridge::install::ContentInstallPath::Automatic,
            download: ContentDownload::Modrinth {
                project_id: dep.project_id.clone().unwrap(),
                version_id: dep.version_id.clone(),
                install_dependencies: true,
            },
            content_source: ContentSource::ModrinthProject { project_id: dep.project_id.clone().unwrap() },
        })
    }

    files.push(ContentInstallFile {
        replace_old: None,
        path: bridge::install::ContentInstallPath::Safe(path),
        download: ContentDownload::Url {
            url: install_file.url.clone(),
            sha1: install_file.hashes.sha1.clone(),
            size: install_file.size,
        },
        content_source: ContentSource::ModrinthProject {
            project_id: project_id.clone()
        },
    });

    let content_install = ContentInstall {
        target,
        loader_hint,
        version_hint: Some(minecraft_version.into()),
        files: files.into(),
    };

    root::start_install(content_install, &data.backend_handle, window, cx);
}
