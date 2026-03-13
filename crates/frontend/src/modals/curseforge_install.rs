use std::{cmp::Ordering, sync::Arc};

use bridge::{install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::InstanceID, meta::MetadataRequest, safe_path::SafePath};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, dialog::Dialog, h_flex, notification::NotificationType, select::{SearchableVec, Select, SelectItem, SelectState}, v_flex, IndexPath, WindowExt
};
use relative_path::RelativePath;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{
    content::ContentSource, curseforge::{CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY, CurseforgeClassId, CurseforgeFile, CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeHit, CurseforgeModLoaderType, CurseforgeReleaseType}, loader::Loader
};
use ustr::Ustr;

use crate::{
    component::instance_dropdown::InstanceDropdown,
    entity::{
        DataEntities, instance::InstanceEntry, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}
    },
    root, ts,
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
            mod_version_not_loaded_message: None,
            mod_version_select_state: None,
            last_selected_loader: None,
        };
        install_dialog.show(window, cx);
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

        let required_dependencies = selected_file.as_ref().map(|version| {
            let mut required = version.dependencies
                .iter()
                .filter(|dep| {
                    dep.relation_type == CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY
                })
                .cloned()
                .collect::<Vec<_>>();

            // Ignore projects that are already installed
            if !required.is_empty()
                && let Some(InstallTarget::Instance(instance_id)) = self.target
                && let Some(instance) = self.data.instances.read(cx).entries.get(&instance_id)
            {
                let mut existing_projects = FxHashSet::default();
                let existing_mods = instance.read(cx).mods.read(cx);
                for summary in existing_mods.iter() {
                    let ContentSource::CurseforgeProject { project_id: project } = &summary.content_source else {
                        continue;
                    };
                    existing_projects.insert(project.clone());
                }
                required.retain(|dep| !existing_projects.contains(&dep.mod_id));
            }

            required
        }).unwrap_or_default();

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
                    .when(!required_dependencies.is_empty(), |modal| {
                        modal.child(Checkbox::new("install_deps").checked(self.install_dependencies).label(if required_dependencies.len() == 1 {
                            ts!("instance.content.install.install_dependency")
                        } else {
                            ts!("instance.content.install.install_dependencies", num = required_dependencies.len())
                        }).on_click(cx.listener(|dialog, value, _, _| {
                            dialog.install_dependencies = *value;
                        })))
                    })
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
