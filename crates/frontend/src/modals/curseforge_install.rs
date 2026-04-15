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
    content::ContentSource, curseforge::{CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY, CurseforgeClassId, CurseforgeFile, CurseforgeFileDependency, CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeHit, CurseforgeModLoaderType, CurseforgeReleaseType}, loader::Loader
};
use ustr::Ustr;

use crate::{
    component::instance_dropdown::InstanceDropdown,
    entity::{
        DataEntities, instance::InstanceEntry, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}
    },
    icon::PandoraIcon, root, ts,
};

struct VersionMatrixLoaders {
    loaders: EnumSet<CurseforgeModLoaderType>,
    same_loaders_for_all_versions: bool,
}

struct InstallDialog {
    title: SharedString,
    name: SharedString,

    data: DataEntities,
    project_type: CurseforgeClassId,
    project_id: u32,

    version_matrix: FxHashMap<&'static str, VersionMatrixLoaders>,
    instances: Option<Entity<SelectState<InstanceDropdown>>>,
    unsupported_instances: usize,

    mod_files: FxHashMap<(Ustr, Option<u32>), Entity<FrontendMetadataState>>,

    target: Option<InstallTarget>,

    last_selected_minecraft_version: Option<SharedString>,
    last_selected_loader: Option<SharedString>,

    fixed_minecraft_version: Option<&'static str>,
    minecraft_version_select_state: Option<Entity<SelectState<SearchableVec<SharedString>>>>,

    fixed_loader: Option<CurseforgeModLoaderType>,
    loader_select_state: Option<Entity<SelectState<Vec<SharedString>>>>,
    skip_loader_check_for_mod_version: bool,
    install_dependencies: bool,
    dep_selection: FxHashSet<u32>,
    dep_keys: FxHashSet<u32>,
    show_dependency_list: bool,
    dep_mod_files: FxHashMap<(u32, Ustr, Option<u32>), Entity<FrontendMetadataState>>,

    mod_version_not_loaded_message: Option<SharedString>,
    mod_version_select_state: Option<Entity<SelectState<SearchableVec<ModVersionItem>>>>,
}

pub fn open(
    hit: CurseforgeHit,
    install_for: Option<InstanceID>,
    data: &DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let name = SharedString::new(hit.name.clone());
    let title = ts!("instance.content.install.title", name = name);
    let project_type = hit.class_id
        .map(CurseforgeClassId::from_u32)
        .unwrap_or_default();

    let mut version_matrix: FxHashMap<&'static str, VersionMatrixLoaders> = FxHashMap::default();
    for version in hit.latest_files_indexes.iter() {
        let mod_loader = version.mod_loader
            .map(CurseforgeModLoaderType::from_u32)
            .unwrap_or(CurseforgeModLoaderType::Any);

        let loaders = EnumSet::only(mod_loader);

        match version_matrix.entry(version.game_version.as_str()) {
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
        if project_type == CurseforgeClassId::Mod || project_type == CurseforgeClassId::Modpack {
            valid_loader = instance_loader == Loader::Vanilla
                || loaders.loaders.contains(instance_loader.as_curseforge_loader());
        }
        if !valid_loader {
            let error_message = ts!("instance.content.load.versions.not_found_for", ver = format!("{} {}", instance_loader.name(), minecraft_version));
            open_error_dialog(title.clone(), error_message, window, cx);
            return;
        }

        let title = title.clone();
        let instance_id = instance.id;
        let fixed_minecraft_version = Some(minecraft_version);
        let fixed_loader = if (project_type == CurseforgeClassId::Mod
            || project_type == CurseforgeClassId::Modpack)
            && instance_loader != Loader::Vanilla
        {
            Some(instance_loader.as_curseforge_loader())
        } else {
            None
        };
        let install_dialog = InstallDialog {
            title,
            name: name.into(),
            data: data.clone(),
            project_type,
            project_id: hit.id,
            version_matrix,
            instances: None,
            unsupported_instances: 0,
            mod_files: Default::default(),
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
            show_dependency_list: false,
            dep_mod_files: Default::default(),
            mod_version_not_loaded_message: None,
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
                    if project_type == CurseforgeClassId::Mod || project_type == CurseforgeClassId::Modpack {
                        valid_loader = instance_loader == Loader::Vanilla
                            || loaders.loaders.contains(instance_loader.as_curseforge_loader());
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
            data: data.clone(),
            project_type,
            project_id: hit.id,
            version_matrix,
            instances,
            unsupported_instances,
            mod_files: Default::default(),
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
            show_dependency_list: false,
            dep_mod_files: Default::default(),
            mod_version_not_loaded_message: None,
            mod_version_select_state: None,
            last_selected_loader: None,
        };
        install_dialog.show(window, cx);
    }
}

pub fn open_latest(
    hit: CurseforgeHit,
    install_for: InstanceID,
    data: &DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let name = SharedString::new(hit.name.clone());
    let title = ts!("instance.content.install.title", name = name);
    let project_type = hit.class_id
        .map(CurseforgeClassId::from_u32)
        .unwrap_or_default();

    let mut version_matrix: FxHashMap<&'static str, VersionMatrixLoaders> = FxHashMap::default();
    for version in hit.latest_files_indexes.iter() {
        let mod_loader = version.mod_loader
            .map(CurseforgeModLoaderType::from_u32)
            .unwrap_or(CurseforgeModLoaderType::Any);

        let loaders = EnumSet::only(mod_loader);

        match version_matrix.entry(version.game_version.as_str()) {
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

    if version_matrix.is_empty() {
        open_error_dialog(title.clone(), ts!("instance.content.load.versions.not_found"), window, cx);
        return;
    }

    let Some(instance) = data.instances.read(cx).entries.get(&install_for) else {
        open_error_dialog(title.clone(), ts!("instance.unable_to_find"), window, cx);
        return;
    };

    let (minecraft_version, instance_loader, instance_id) = {
        let instance = instance.read(cx);
        (
            instance.configuration.minecraft_version.as_str().to_string(),
            instance.configuration.loader,
            instance.id,
        )
    };

    let Some(loaders) = version_matrix.get(minecraft_version.as_str()) else {
        let error_message = ts!("instance.content.load.versions.not_found_for", ver = minecraft_version.as_str());
        open_error_dialog(title.clone(), error_message, window, cx);
        return;
    };

    let mut valid_loader = true;
    if project_type == CurseforgeClassId::Mod || project_type == CurseforgeClassId::Modpack {
        valid_loader = instance_loader == Loader::Vanilla
            || loaders.loaders.contains(instance_loader.as_curseforge_loader());
    }
    if !valid_loader {
        let error_message = ts!("instance.content.load.versions.not_found_for", ver = format!("{} {}", instance_loader.name(), minecraft_version));
        open_error_dialog(title.clone(), error_message, window, cx);
        return;
    }

    let mod_loader_type = if (project_type == CurseforgeClassId::Mod || project_type == CurseforgeClassId::Modpack)
        && instance_loader != Loader::Vanilla
    {
        Some(instance_loader.as_curseforge_loader() as u32)
    } else {
        None
    };

    let request = FrontendMetadata::request(
        &data.metadata,
        MetadataRequest::CurseforgeGetModFiles(CurseforgeGetModFilesRequest {
            mod_id: hit.id,
            game_version: Some(minecraft_version.clone().into()),
            mod_loader_type,
            page_size: None,
        }),
        cx,
    );

    open_latest_from_entity(
        title,
        request,
        hit.id,
        project_type,
        InstallTarget::Instance(instance_id),
        instance_loader,
        minecraft_version.into(),
        data.clone(),
        window,
        cx,
    );
}

fn open_error_dialog(title: SharedString, text: SharedString, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |modal, _, _| {
        modal.title(title.clone()).child(text.clone())
    });
}

fn open_latest_from_entity(
    title: SharedString,
    mod_files: Entity<FrontendMetadataState>,
    project_id: u32,
    project_type: CurseforgeClassId,
    target: InstallTarget,
    loader_hint: Loader,
    minecraft_version: SharedString,
    data: DataEntities,
    window: &mut Window,
    cx: &mut App,
) {
    let result: FrontendMetadataResult<CurseforgeGetModFilesResult> = mod_files.read(cx).result();
    match result {
        FrontendMetadataResult::Loading => {
            let observe_title = title.clone();
            let _subscription = window.observe(&mod_files, cx, move |mod_files, window, cx| {
                window.close_all_dialogs(cx);
                open_latest_from_entity(
                    observe_title.clone(),
                    mod_files,
                    project_id,
                    project_type,
                    target.clone(),
                    loader_hint,
                    minecraft_version.clone(),
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
        FrontendMetadataResult::Loaded(result) => {
            let selected_file = select_latest_curseforge_file(&result.data);
            let Some(selected_file) = selected_file else {
                open_error_dialog(title.clone(), ts!("instance.content.load.versions.not_found"), window, cx);
                return;
            };

            let required_dependencies = required_curseforge_dependencies(&selected_file);
            let installed_projects = curseforge_installed_projects(&target, &data, cx);
            let selected_dependencies = required_dependencies
                .into_iter()
                .filter(|dep| !installed_projects.contains(&dep.mod_id))
                .collect::<Vec<_>>();

            start_curseforge_install(
                &data,
                project_type,
                project_id,
                &selected_file,
                target,
                loader_hint,
                minecraft_version.as_str(),
                selected_dependencies,
                window,
                cx,
            );
        },
        FrontendMetadataResult::Error(shared_string) => {
            window.open_dialog(cx, move |modal, _, _| {
                modal.title(title.clone()).child(shared_string.clone())
            });
        },
    }
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
                CurseforgeClassId::Mod => ts!("instance.content.install.new_instance_with.mod"),
                CurseforgeClassId::Modpack => ts!("instance.content.install.new_instance_with.modpack"),
                CurseforgeClassId::Resourcepack => ts!("instance.content.install.new_instance_with.resourcepack"),
                CurseforgeClassId::Shader => ts!("instance.content.install.new_instance_with.shader"),
                _ => ts!("instance.content.install.new_instance_with.file"),
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
                                    if (this.project_type == CurseforgeClassId::Mod
                                        || this.project_type == CurseforgeClassId::Modpack)
                                        && instance.configuration.loader != Loader::Vanilla
                                    {
                                        this.fixed_loader = Some(instance.configuration.loader.as_curseforge_loader());
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
                        loaders.loaders.iter().map(CurseforgeModLoaderType::pretty_name).map(SharedString::new_static).collect();

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
            let selected_game_version: Ustr = selected_game_version.as_str().into();

            let mod_loader_type = if self.skip_loader_check_for_mod_version {
                None
            } else {
                Some(CurseforgeModLoaderType::from_name(selected_loader.as_str()) as u32)
            };

            let request = self.mod_files
                .entry((selected_game_version, mod_loader_type))
                .or_insert_with(|| {
                    FrontendMetadata::request(
                        &self.data.metadata,
                        MetadataRequest::CurseforgeGetModFiles(CurseforgeGetModFilesRequest {
                            mod_id: self.project_id,
                            game_version: Some(selected_game_version),
                            mod_loader_type,
                            page_size: None,
                        }),
                        cx,
                    )
                });

            let result: FrontendMetadataResult<CurseforgeGetModFilesResult> = request.read(cx).result();

            match result {
                FrontendMetadataResult::Loading => {
                    self.mod_version_not_loaded_message = Some("Loading files...".into());
                },
                FrontendMetadataResult::Loaded(result) => {
                    self.mod_version_not_loaded_message = None;

                    let mod_versions: Vec<ModVersionItem> = result.data.iter().map(|file| {
                        ModVersionItem {
                            name: file.file_name.clone().into(),
                            file: file.clone(),
                        }
                    }).collect();

                    let mut highest_release = None;
                    let mut highest_beta = None;
                    let mut highest_alpha = None;

                    for (index, version) in mod_versions.iter().enumerate() {
                        match CurseforgeReleaseType::from_u32(version.file.release_type) {
                            CurseforgeReleaseType::Release => {
                                highest_release = Some(index);
                                break;
                            },
                            CurseforgeReleaseType::Beta => {
                                if highest_beta.is_none() {
                                    highest_beta = Some(index);
                                }
                            },
                            _ => {
                                if highest_alpha.is_none() {
                                    highest_alpha = Some(index);
                                }
                            },
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
                },
                FrontendMetadataResult::Error(shared_string) => {
                    self.mod_version_not_loaded_message = Some(format!("Error loading files: {}", shared_string).into());
                },
            }
        }

        let selected_file = self
            .mod_version_select_state
            .as_ref()
            .and_then(|state| state.read(cx).selected_value())
            .cloned();

        let filename_prefix = ts!("instance.content.filename_prefix");

        let required_dependencies = selected_file
            .as_ref()
            .map(required_curseforge_dependencies)
            .unwrap_or_default();

        let installed_projects = curseforge_installed_projects(&self.target.clone().unwrap(), &self.data, cx);
        let loader_hint = loader_for_curseforge_selection(selected_loader.as_ref());
        let version_hint = selected_minecraft_version.as_ref().map(|v| v.as_str()).unwrap_or_default();
        let dep_tree = build_curseforge_dep_tree(
            &required_dependencies,
            &self.data,
            &mut self.dep_mod_files,
            &installed_projects,
            cx,
            loader_hint,
            version_hint,
        );
        let flat_deps = flatten_curseforge_deps(&dep_tree);
        let selectable_keys: Vec<u32> = flat_deps.iter().map(|dep| dep.key).collect();

        let required_keys: FxHashSet<u32> = selectable_keys.iter().copied().collect();
        if required_keys != self.dep_keys {
            self.dep_keys = required_keys.clone();
            self.dep_selection = required_keys;
        }
        self.install_dependencies = !self.dep_selection.is_empty();

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
            .when_some(self.mod_version_not_loaded_message.clone(), |modal, message| modal.child(message))
            .when_some(self.mod_version_select_state.as_ref(), |modal, mod_versions| {
                modal
                    .child(Select::new(mod_versions).title_prefix(filename_prefix))
                    .when(!flat_deps.is_empty(), |modal| {
                        let dep_ui = curseforge_dep_ui_state(&flat_deps, &self.dep_selection);
                        let dep_list = build_curseforge_dep_list(&flat_deps, &self.dep_selection, "install_dep", cx);
                        let header = build_curseforge_dep_header(
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
                    .child(div().border_t_1().border_color(cx.theme().border).mt_2())
                    .child(Button::new("install").success().label(ts!("instance.content.install.label")).on_click(cx.listener(
                        move |this, _, window, cx| {
                            let Some(selected_file) = selected_file.as_ref() else {
                                window.push_notification((NotificationType::Error, ts!("instance.content.install.no_mod_version_selected")), cx);
                                return;
                            };

                            let path = match this.project_type {
                                CurseforgeClassId::Mod => RelativePath::new("mods").join(&*selected_file.file_name),
                                CurseforgeClassId::Modpack => RelativePath::new("mods").join(&*selected_file.file_name),
                                CurseforgeClassId::Resourcepack => RelativePath::new("resourcepacks").join(&*selected_file.file_name),
                                CurseforgeClassId::Shader => RelativePath::new("shaderpacks").join(&*selected_file.file_name),
                                _ => {
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
                                let curseforge_loader = CurseforgeModLoaderType::from_name(selected_loader);
                                match curseforge_loader {
                                    CurseforgeModLoaderType::Fabric => loader_hint = Loader::Fabric,
                                    CurseforgeModLoaderType::Forge => loader_hint = Loader::Forge,
                                    CurseforgeModLoaderType::NeoForge => loader_hint = Loader::NeoForge,
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
                                    if !this.dep_selection.contains(&dep.mod_id) {
                                        continue;
                                    }
                                    files.push(ContentInstallFile {
                                        replace_old: None,
                                        path: bridge::install::ContentInstallPath::Automatic,
                                        download: ContentDownload::Curseforge {
                                            project_id: dep.mod_id,
                                            install_dependencies: true,
                                        },
                                        content_source: ContentSource::CurseforgeProject { project_id: dep.mod_id },
                                    })
                                }
                            }

                            let sha1 = selected_file.hashes.iter()
                                .find(|hash| hash.algo == 1).map(|hash| hash.value.clone());

                            let Some(sha1) = sha1 else {
                                window.push_notification((NotificationType::Error, ts!("instance.content.install.missing_sha1_hash")), cx);
                                return;
                            };

                            let Some(download_url) = selected_file.download_url.clone() else {
                                window.push_notification((NotificationType::Error, ts!("instance.content.install.no_third_party_downloads")), cx);
                                return;
                            };

                            files.push(ContentInstallFile {
                                replace_old: None,
                                path: bridge::install::ContentInstallPath::Safe(path),
                                download: ContentDownload::Url {
                                    url: download_url,
                                    sha1: sha1,
                                    size: selected_file.file_length as usize,
                                },
                                content_source: ContentSource::CurseforgeProject {
                                    project_id: this.project_id
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
    file: CurseforgeFile,
}

impl SelectItem for ModVersionItem {
    type Value = CurseforgeFile;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.file
    }
}

#[derive(Clone)]
struct CurseforgeDepNode {
    key: u32,
    label: SharedString,
    installed: bool,
    children: Vec<CurseforgeDepNode>,
}

#[derive(Clone)]
struct CurseforgeFlatDep {
    key: u32,
    label: SharedString,
}

fn required_curseforge_dependencies(version: &CurseforgeFile) -> Vec<CurseforgeFileDependency> {
    let required = version.dependencies
        .iter()
        .filter(|dep| {
            dep.relation_type == CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY
        })
        .cloned()
        .collect::<Vec<_>>();

    required
}

fn curseforge_installed_projects(
    target: &InstallTarget,
    data: &DataEntities,
    cx: &App,
) -> FxHashSet<u32> {
    let mut existing_projects = FxHashSet::default();
    if let InstallTarget::Instance(instance_id) = *target
        && let Some(instance) = data.instances.read(cx).entries.get(&instance_id)
    {
        let existing_mods = instance.read(cx).mods.read(cx);
        for summary in existing_mods.iter() {
            let ContentSource::CurseforgeProject { project_id: project } = &summary.content_source else {
                continue;
            };
            existing_projects.insert(project.clone());
        }
    }
    existing_projects
}

fn loader_for_curseforge_selection(selected_loader: Option<&SharedString>) -> Loader {
    if let Some(selected_loader) = selected_loader {
        let curseforge_loader = CurseforgeModLoaderType::from_name(selected_loader.as_str());
        match curseforge_loader {
            CurseforgeModLoaderType::Fabric => Loader::Fabric,
            CurseforgeModLoaderType::Forge => Loader::Forge,
            CurseforgeModLoaderType::NeoForge => Loader::NeoForge,
            _ => Loader::Unknown,
        }
    } else {
        Loader::Unknown
    }
}

fn build_curseforge_dep_tree(
    deps: &[CurseforgeFileDependency],
    data: &DataEntities,
    dep_mod_files: &mut FxHashMap<(u32, Ustr, Option<u32>), Entity<FrontendMetadataState>>,
    installed_projects: &FxHashSet<u32>,
    cx: &mut App,
    loader_hint: Loader,
    minecraft_version: &str,
) -> Vec<CurseforgeDepNode> {
    let mut visited = FxHashSet::default();
    build_curseforge_dep_nodes(
        deps,
        data,
        dep_mod_files,
        installed_projects,
        cx,
        loader_hint,
        minecraft_version,
        &mut visited,
    )
}

fn build_curseforge_dep_nodes(
    deps: &[CurseforgeFileDependency],
    data: &DataEntities,
    dep_mod_files: &mut FxHashMap<(u32, Ustr, Option<u32>), Entity<FrontendMetadataState>>,
    installed_projects: &FxHashSet<u32>,
    cx: &mut App,
    loader_hint: Loader,
    minecraft_version: &str,
    visited: &mut FxHashSet<u32>,
) -> Vec<CurseforgeDepNode> {
    let mut nodes = Vec::new();
    for dep in deps.iter() {
        let installed = installed_projects.contains(&dep.mod_id);
        let label = if installed {
            format!("Mod ID {} (already installed)", dep.mod_id).into()
        } else {
            format!("Mod ID {}", dep.mod_id).into()
        };

        let mut children = Vec::new();
        if visited.insert(dep.mod_id) {
            let mod_loader_type = if loader_hint != Loader::Vanilla {
                Some(loader_hint.as_curseforge_loader() as u32)
            } else {
                None
            };
            let key = (dep.mod_id, Ustr::from(minecraft_version), mod_loader_type);
            let request = dep_mod_files.entry(key).or_insert_with(|| {
                FrontendMetadata::request(
                    &data.metadata,
                    MetadataRequest::CurseforgeGetModFiles(CurseforgeGetModFilesRequest {
                        mod_id: dep.mod_id,
                        game_version: Some(Ustr::from(minecraft_version)),
                        mod_loader_type,
                        page_size: None,
                    }),
                    cx,
                )
            });

            let result: FrontendMetadataResult<CurseforgeGetModFilesResult> = request.read(cx).result();
            if let FrontendMetadataResult::Loaded(result) = result {
                if let Some(file) = select_latest_curseforge_file(&result.data) {
                    let required = required_curseforge_dependencies(&file);
                    children = build_curseforge_dep_nodes(
                        &required,
                        data,
                        dep_mod_files,
                        installed_projects,
                        cx,
                        loader_hint,
                        minecraft_version,
                        visited,
                    );
                }
            }
        }

        nodes.push(CurseforgeDepNode {
            key: dep.mod_id,
            label,
            installed,
            children,
        });
    }
    nodes
}

fn flatten_curseforge_deps(nodes: &[CurseforgeDepNode]) -> Vec<CurseforgeFlatDep> {
    let mut deps = Vec::new();
    flatten_curseforge_deps_inner(nodes, &mut deps);
    deps
}

fn flatten_curseforge_deps_inner(nodes: &[CurseforgeDepNode], deps: &mut Vec<CurseforgeFlatDep>) {
    for node in nodes {
        if !node.installed {
            deps.push(CurseforgeFlatDep {
                key: node.key,
                label: node.label.clone(),
            });
        }
        if !node.children.is_empty() {
            flatten_curseforge_deps_inner(&node.children, deps);
        }
    }
}

trait CurseforgeDepSelection {
    fn set_selected(&mut self, key: u32, selected: bool);
    fn set_all_selected(&mut self, keys: &[u32], selected: bool);
    fn toggle_dependency_list(&mut self);
}

impl CurseforgeDepSelection for InstallDialog {
    fn set_selected(&mut self, key: u32, selected: bool) {
        if selected {
            self.dep_selection.insert(key);
        } else {
            self.dep_selection.remove(&key);
        }
    }

    fn set_all_selected(&mut self, keys: &[u32], selected: bool) {
        if selected {
            self.dep_selection = keys.iter().copied().collect();
        } else {
            self.dep_selection.clear();
        }
    }

    fn toggle_dependency_list(&mut self) {
        self.show_dependency_list = !self.show_dependency_list;
    }
}


struct CurseforgeDepUiState {
    selectable_keys: Vec<u32>,
    total_selectable: usize,
    selected_count: usize,
    label: SharedString,
}

fn curseforge_dep_ui_state(flat_deps: &[CurseforgeFlatDep], selection: &FxHashSet<u32>) -> CurseforgeDepUiState {
    let selectable_keys: Vec<u32> = flat_deps.iter().map(|dep| dep.key).collect();
    let total_selectable = selectable_keys.len();
    let selected_count = selection.len();
    let label = if selected_count == 1 {
        ts!("instance.content.install.install_dependency")
    } else {
        ts!("instance.content.install.install_dependencies", num = selected_count)
    };

    CurseforgeDepUiState {
        selectable_keys,
        total_selectable,
        selected_count,
        label,
    }
}

fn build_curseforge_dep_list<T: CurseforgeDepSelection + 'static>(
    flat_deps: &[CurseforgeFlatDep],
    selection: &FxHashSet<u32>,
    id_prefix: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut dep_elements = Vec::new();
    for (index, dep) in flat_deps.iter().enumerate() {
        let key = dep.key;
        let label = dep.label.clone();
        dep_elements.push(
            Checkbox::new(format!("{id_prefix}_{index}"))
                .checked(selection.contains(&key))
                .label(label)
                .on_click(cx.listener(move |dialog, value, _, _| {
                    dialog.set_selected(key, *value);
                }))
                .into_any_element()
        );
    }

    v_flex().gap_1().pl_3().children(dep_elements).into_any_element()
}

fn build_curseforge_dep_header<T: CurseforgeDepSelection + 'static>(
    label: SharedString,
    selectable_keys: Vec<u32>,
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

fn select_latest_curseforge_file(files: &[CurseforgeFile]) -> Option<CurseforgeFile> {
    if files.is_empty() {
        return None;
    }

    let mut highest_release = None;
    let mut highest_beta = None;
    let mut highest_alpha = None;

    for (index, version) in files.iter().enumerate() {
        match CurseforgeReleaseType::from_u32(version.release_type) {
            CurseforgeReleaseType::Release => {
                highest_release = Some(index);
                break;
            },
            CurseforgeReleaseType::Beta => {
                if highest_beta.is_none() {
                    highest_beta = Some(index);
                }
            },
            CurseforgeReleaseType::Alpha => {
                if highest_alpha.is_none() {
                    highest_alpha = Some(index);
                }
            },
            _ => {},
        }
    }

    let highest = highest_release.or(highest_beta).or(highest_alpha);
    highest.map(|index| files[index].clone())
}

fn start_curseforge_install(
    data: &DataEntities,
    project_type: CurseforgeClassId,
    project_id: u32,
    selected_file: &CurseforgeFile,
    target: InstallTarget,
    loader_hint: Loader,
    minecraft_version: &str,
    selected_dependencies: Vec<CurseforgeFileDependency>,
    window: &mut Window,
    cx: &mut App,
) {
    let path = match project_type {
        CurseforgeClassId::Mod => RelativePath::new("mods").join(&*selected_file.file_name),
        CurseforgeClassId::Modpack => RelativePath::new("mods").join(&*selected_file.file_name),
        CurseforgeClassId::Resourcepack => RelativePath::new("resourcepacks").join(&*selected_file.file_name),
        CurseforgeClassId::Shader => RelativePath::new("shaderpacks").join(&*selected_file.file_name),
        _ => {
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
            download: ContentDownload::Curseforge {
                project_id: dep.mod_id,
                install_dependencies: true,
            },
            content_source: ContentSource::CurseforgeProject { project_id: dep.mod_id },
        })
    }

    let sha1 = selected_file.hashes.iter()
        .find(|hash| hash.algo == 1).map(|hash| hash.value.clone());

    let Some(sha1) = sha1 else {
        window.push_notification((NotificationType::Error, ts!("instance.content.install.missing_sha1_hash")), cx);
        return;
    };

    let Some(download_url) = selected_file.download_url.clone() else {
        window.push_notification((NotificationType::Error, ts!("instance.content.install.no_third_party_downloads")), cx);
        return;
    };

    files.push(ContentInstallFile {
        replace_old: None,
        path: bridge::install::ContentInstallPath::Safe(path),
        download: ContentDownload::Url {
            url: download_url,
            sha1,
            size: selected_file.file_length as usize,
        },
        content_source: ContentSource::CurseforgeProject {
            project_id
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
