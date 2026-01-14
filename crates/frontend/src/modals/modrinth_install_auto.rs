use std::sync::Arc;

use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget},
    instance::InstanceID,
    message::MessageToBackend,
    meta::MetadataRequest,
    modal_action::ModalAction,
    safe_path::SafePath,
};
use gpui::{prelude::*, *};
use gpui_component::{WindowExt, h_flex, notification::Notification, spinner::Spinner};
use relative_path::RelativePath;
use schema::{
    content::ContentSource,
    modrinth::{
        ModrinthDependencyType, ModrinthProjectType, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult,
        ModrinthVersionType,
    },
};
use uuid::Uuid;

use crate::entity::{
    DataEntities,
    metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState},
};

// struct VersionMatrixLoaders {
//     loaders: EnumSet<ModrinthLoader>,
//     same_loaders_for_all_versions: bool,
// }

// struct InstallNotification {
//     title: SharedString,
//     name: SharedString,

//     project_versions: Arc<[ModrinthProjectVersion]>,
//     data: DataEntities,
//     project_type: ModrinthProjectType,

//     version_matrix: FxHashMap<&'static str, VersionMatrixLoaders>,
//     instances: Option<Entity<SelectState<InstanceDropdown>>>,
//     unsupported_instances: usize,

//     target: Option<InstallTarget>,

//     last_selected_minecraft_version: Option<SharedString>,
//     last_selected_loader: Option<SharedString>,

//     fixed_minecraft_version: Option<&'static str>,
//     minecraft_version_select_state: Option<Entity<SelectState<SearchableVec<SharedString>>>>,

//     fixed_loader: Option<ModrinthLoader>,
//     loader_select_state: Option<Entity<SelectState<Vec<SharedString>>>>,
//     skip_loader_check_for_mod_version: bool,
//     install_dependencies: bool,

//     mod_version_select_state: Option<Entity<SelectState<SearchableVec<ModVersionItem>>>>,
// }

struct AutoInstallNotificationType;

pub fn open(
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
            project_id,
            game_versions: None,
            loaders: None,
        }),
        cx,
    );

    let key = Uuid::new_v4();
    let title = SharedString::new(format!("Install {}", name));

    if handle_project_versions(data, title.clone(), key, project_type, install_for, &project_versions, window, cx) {
        return;
    }

    let _subscription = window.observe(&project_versions, cx, {
        let title = title.clone();
        let data = data.clone();
        move |project_versions, window, cx| {
            handle_project_versions(
                &data,
                title.clone(),
                key,
                project_type,
                install_for,
                &project_versions,
                window,
                cx,
            );
        }
    });

    let notification = Notification::new()
        .id1::<AutoInstallNotificationType>(key)
        .title(title)
        .content(move |_, _, _| {
            _ = &_subscription;

            h_flex()
                .gap_2()
                .child("Loading project versions from Modrinth...")
                .child(Spinner::new())
                .into_any_element()
        })
        .autohide(false);

    window.push_notification(notification, cx);
}

fn handle_project_versions(
    data: &DataEntities,
    title: SharedString,
    key: Uuid,
    project_type: ModrinthProjectType,
    install_for: InstanceID,
    project_versions: &Entity<FrontendMetadataState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = project_versions.read(cx).result();
    match result {
        FrontendMetadataResult::Loading => {
            return false;
        },
        FrontendMetadataResult::Loaded(project_versions) => {
            let Some(instance) = data.instances.read(cx).entries.get(&install_for) else {
                return true;
            };
            let configuration = instance.read(cx).configuration.clone();
            let modrinth_loader = configuration.loader.as_modrinth_loader();
            let matching_versions = project_versions
                .0
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
                    if !game_versions.contains(&configuration.minecraft_version) {
                        return false;
                    }
                    if !loaders.contains(&modrinth_loader) {
                        return false;
                    }
                    true
                })
                .collect::<Vec<_>>();

            let mut highest_release = None;
            let mut highest_beta = None;
            let mut highest_alpha = None;

            for (index, version) in matching_versions.iter().enumerate() {
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
            let Some(highest) = highest else {
                push_error(title.clone(), key, "Unable to find matching version of project".into(), window, cx);
                return true;
            };

            let version = matching_versions[highest];

            let install_file = version.files.iter().find(|file| file.primary).unwrap_or(version.files.first().unwrap());

            let path = match project_type {
                ModrinthProjectType::Mod => RelativePath::new("mods").join(&*install_file.filename),
                ModrinthProjectType::Modpack => RelativePath::new("mods").join(&*install_file.filename),
                ModrinthProjectType::Resourcepack => RelativePath::new("resourcepacks").join(&*install_file.filename),
                ModrinthProjectType::Shader => RelativePath::new("shaderpacks").join(&*install_file.filename),
                ModrinthProjectType::Other => {
                    push_error(title.clone(), key, "Unable to install 'other' project type".into(), window, cx);
                    return true;
                },
            };

            let Some(path) = SafePath::from_relative_path(&path) else {
                push_error(title.clone(), key, "Invalid/dangerous filename".into(), window, cx);
                return true;
            };

            let mut files = Vec::new();

            let required_dependencies = version.dependencies.as_ref().map(|deps| {
                deps.iter()
                    .filter(|dep| dep.project_id.is_some() && dep.dependency_type == ModrinthDependencyType::Required)
                    .cloned()
                    .collect::<Vec<_>>()
            });

            if let Some(required_dependencies) = required_dependencies {
                for dep in required_dependencies.iter() {
                    files.push(ContentInstallFile {
                        replace_old: None,
                        path: bridge::install::ContentInstallPath::Automatic,
                        download: ContentDownload::Modrinth {
                            project_id: dep.project_id.clone().unwrap(),
                            version_id: dep.version_id.clone(),
                        },
                        content_source: ContentSource::Modrinth,
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
                content_source: ContentSource::Modrinth,
            });

            let content_install = ContentInstall {
                target: InstallTarget::Instance(install_for),
                loader_hint: configuration.loader,
                version_hint: Some(configuration.minecraft_version.into()),
                files: files.into(),
            };
            let modal_action = ModalAction::default();

            data.backend_handle.send(MessageToBackend::InstallContent {
                content: content_install.clone(),
                modal_action: modal_action.clone(),
            });

            crate::modals::generic::show_notification_with_note(
                window,
                cx,
                "Error installing content".into(),
                modal_action,
                Notification::new().id1::<AutoInstallNotificationType>(key),
            );

            return true;
        },
        FrontendMetadataResult::Error(error) => {
            push_error(
                title.clone(),
                key,
                format!("Error loading project versions from Modrinth:\n{error}").into(),
                window,
                cx,
            );
            return true;
        },
    }
}

fn push_error(title: SharedString, key: Uuid, message: SharedString, window: &mut Window, cx: &mut App) {
    let notification = Notification::error(message)
        .id1::<AutoInstallNotificationType>(key)
        .title(title)
        .autohide(false);

    window.push_notification(notification, cx);
}

// fn open_from_entity(
//     name: SharedString,
//     project_versions: Entity<FrontendMetadataState>,
//     project_type: ModrinthProjectType,
//     install_for: Option<InstanceID>,
//     data: DataEntities,
//     window: &mut Window,
//     cx: &mut App,
// ) {
//     let title = SharedString::new(format!("Install {}", name));

//     let result: FrontendMetadataResult<ModrinthProjectVersionsResult> = project_versions.read(cx).result();
//     match result {
//         FrontendMetadataResult::Loading => {
//             let _subscription = window.observe(&project_versions, cx, move |project_versions, window, cx| {
//                 window.close_all_dialogs(cx);
//                 open_from_entity(name.clone(), project_versions, project_type, install_for, data.clone(), window, cx);
//             });
//             window.open_dialog(cx, move |dialog, _, _| {
//                 let _ = &_subscription;
//                 dialog.title(title.clone()).child(h_flex().gap_2().child("Loading mod versions...").child(Spinner::new()))
//             });
//         },
//         FrontendMetadataResult::Loaded(versions) => {
//             let mut valid_project_versions = Vec::with_capacity(versions.0.len());

//             let mut version_matrix: FxHashMap<&'static str, VersionMatrixLoaders> = FxHashMap::default();
//             for version in versions.0.iter() {
//                 let Some(loaders) = version.loaders.clone() else {
//                     continue;
//                 };
//                 let Some(game_versions) = &version.game_versions else {
//                     continue;
//                 };
//                 if version.files.is_empty() {
//                     continue;
//                 }
//                 if let Some(status) = version.status
//                     && !matches!(status, ModrinthVersionStatus::Listed | ModrinthVersionStatus::Archived)
//                 {
//                     continue;
//                 }

//                 let mut loaders = EnumSet::from_iter(loaders.iter().copied());
//                 loaders.remove(ModrinthLoader::Unknown);
//                 if loaders.is_empty() {
//                     continue;
//                 }

//                 valid_project_versions.push(version.clone());

//                 for game_version in game_versions.iter() {
//                     match version_matrix.entry(game_version.as_str()) {
//                         std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
//                             occupied_entry.get_mut().same_loaders_for_all_versions &=
//                                 occupied_entry.get().loaders == loaders;
//                             occupied_entry.get_mut().loaders |= loaders;
//                         },
//                         std::collections::hash_map::Entry::Vacant(vacant_entry) => {
//                             vacant_entry.insert(VersionMatrixLoaders {
//                                 loaders,
//                                 same_loaders_for_all_versions: true,
//                             });
//                         },
//                     }
//                 }
//             }

//             if version_matrix.is_empty() {
//                 open_error_dialog(title.clone(), "No mod versions found".into(), window, cx);
//                 return;
//             }
//             if let Some(install_for) = install_for {
//                 let Some(instance) = data.instances.read(cx).entries.get(&install_for) else {
//                     open_error_dialog(title.clone(), "Unable to find instance".into(), window, cx);
//                     return;
//                 };

//                 let instance = instance.read(cx);

//                 let minecraft_version = instance.configuration.minecraft_version.as_str();
//                 let instance_loader = instance.configuration.loader;

//                 let Some(loaders) = version_matrix.get(minecraft_version) else {
//                     let error_message = SharedString::from(&format!("No mod versions found for {}", minecraft_version));
//                     open_error_dialog(title.clone(), error_message, window, cx);
//                     return;
//                 };

//                 let mut valid_loader = true;
//                 if project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack {
//                     valid_loader = instance_loader == Loader::Vanilla
//                         || loaders.loaders.contains(instance_loader.as_modrinth_loader());
//                 }
//                 if !valid_loader {
//                     let error_message = SharedString::from(&format!("No mod versions found for {} {}",
//                         instance_loader.name(), minecraft_version));
//                     open_error_dialog(title.clone(), error_message, window, cx);
//                     return;
//                 }

//                 let title = title.clone();
//                 let instance_id = instance.id;
//                 let fixed_minecraft_version = Some(minecraft_version);
//                 let fixed_loader = if (project_type == ModrinthProjectType::Mod
//                     || project_type == ModrinthProjectType::Modpack)
//                     && instance_loader != Loader::Vanilla
//                 {
//                     Some(instance_loader.as_modrinth_loader())
//                 } else {
//                     None
//                 };
//                 let install_dialog = InstallNotification {
//                     title,
//                     name: name.into(),
//                     project_versions: valid_project_versions.into(),
//                     data,
//                     project_type,
//                     version_matrix,
//                     instances: None,
//                     unsupported_instances: 0,
//                     target: Some(InstallTarget::Instance(instance_id)),
//                     fixed_minecraft_version,
//                     minecraft_version_select_state: None,
//                     fixed_loader,
//                     loader_select_state: None,
//                     last_selected_minecraft_version: None,
//                     skip_loader_check_for_mod_version: false,
//                     install_dependencies: true,
//                     mod_version_select_state: None,
//                     last_selected_loader: None,
//                 };
//                 install_dialog.show(window, cx);
//             } else {
//                 let instance_entries = data.instances.clone();

//                 let entries: Arc<[InstanceEntry]> = instance_entries
//                     .read(cx)
//                     .entries
//                     .iter()
//                     .filter_map(|(_, instance)| {
//                         let instance = instance.read(cx);

//                         let minecraft_version = instance.configuration.minecraft_version.as_str();
//                         let instance_loader = instance.configuration.loader;

//                         if let Some(loaders) = version_matrix.get(minecraft_version) {
//                             let mut valid_loader = true;
//                             if project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack {
//                                 valid_loader = instance_loader == Loader::Vanilla
//                                     || loaders.loaders.contains(instance_loader.as_modrinth_loader());
//                             }
//                             if valid_loader {
//                                 return Some(instance.clone());
//                             }
//                         }

//                         None
//                     })
//                     .collect();

//                 let unsupported_instances = instance_entries.read(cx).entries.len().saturating_sub(entries.len());
//                 let instances = if !entries.is_empty() {
//                     let dropdown = InstanceDropdown::create(entries, window, cx);
//                     dropdown.update(cx, |dropdown, cx| {
//                         dropdown.set_selected_index(Some(IndexPath::default()), window, cx)
//                     });
//                     Some(dropdown)
//                 } else {
//                     None
//                 };

//                 let install_dialog = InstallNotification {
//                     title,
//                     name: name.into(),
//                     project_versions: valid_project_versions.into(),
//                     data,
//                     project_type,
//                     version_matrix,
//                     instances,
//                     unsupported_instances,
//                     target: None,
//                     fixed_minecraft_version: None,
//                     minecraft_version_select_state: None,
//                     fixed_loader: None,
//                     loader_select_state: None,
//                     last_selected_minecraft_version: None,
//                     skip_loader_check_for_mod_version: false,
//                     install_dependencies: true,
//                     mod_version_select_state: None,
//                     last_selected_loader: None,
//                 };
//                 install_dialog.show(window, cx);
//             }
//         },
//         FrontendMetadataResult::Error(message) => {
//             window.open_dialog(cx, move |modal, _, _| {
//                 modal.title(title.clone()).child(ErrorAlert::new("error", "Error requesting from Modrinth".into(), message.clone()))
//             });
//         },
//     }
// }

// fn open_error_dialog(title: SharedString, text: SharedString, window: &mut Window, cx: &mut App) {
//     window.open_dialog(cx, move |modal, _, _| {
//         modal.title(title.clone()).child(text.clone())
//     });
// }

// impl InstallNotification {
//     fn show(self, window: &mut Window, cx: &mut App) {
//         let install_notification = cx.new(|_| self);
//         window.push_notification(cx, move |modal, window, cx| {
//             install_notification.update(cx, |this, cx| this.render(modal, window, cx))
//         });
//     }

//     fn render(&mut self, modal: Dialog, window: &mut Window, cx: &mut Context<Self>) -> Dialog {
//         let modal = modal.title(self.title.clone());

//         if self.target.is_none() {
//             let create_instance_label = match self.project_type {
//                 ModrinthProjectType::Mod => "Create new instance with this mod",
//                 ModrinthProjectType::Modpack => "Create new instance with this modpack",
//                 ModrinthProjectType::Resourcepack => "Create new instance with this resourcepack",
//                 ModrinthProjectType::Shader => "Create new instance with this shader",
//                 ModrinthProjectType::Other => "Create new instance with this file",
//             };

//             let content = v_flex()
//                 .gap_2()
//                 .text_center()
//                 .when_some(self.instances.as_ref(), |content, instances| {
//                     let read_instances = instances.read(cx);
//                     let selected_instance: Option<InstanceEntry> = read_instances.selected_value().cloned();

//                     let button_and_dropdown = h_flex()
//                         .gap_2()
//                         .child(
//                             v_flex()
//                                 .w_full()
//                                 .gap_0p5()
//                                 .child(
//                                     Select::new(instances).placeholder("Select an instance").title_prefix("Instance: "),
//                                 )
//                                 .when(self.unsupported_instances > 0, |content| {
//                                     content
//                                         .child(format!("({} instances were incompatible)", self.unsupported_instances))
//                                 }),
//                         )
//                         .when_some(selected_instance, |dialog, instance| {
//                             dialog.child(Button::new("instance").success().h_full().label("Add to instance").on_click(
//                                 cx.listener(move |this, _, _, _| {
//                                     this.target = Some(InstallTarget::Instance(instance.id));
//                                     this.fixed_minecraft_version = Some(instance.configuration.minecraft_version.as_str());
//                                     if (this.project_type == ModrinthProjectType::Mod
//                                         || this.project_type == ModrinthProjectType::Modpack)
//                                         && instance.configuration.loader != Loader::Vanilla
//                                     {
//                                         this.fixed_loader = Some(instance.configuration.loader.as_modrinth_loader());
//                                     }
//                                 }),
//                             ))
//                         });

//                     content.child(button_and_dropdown).child("— OR —")
//                 })
//                 .child(Button::new("create").success().label(create_instance_label).on_click(cx.listener(
//                     |this, _, _, _| {
//                         this.target = Some(InstallTarget::NewInstance {
//                             name: "New Instance".into(),
//                         });
//                     },
//                 )));

//             return modal.child(content);
//         }

//         if self.minecraft_version_select_state.is_none() {
//             if let Some(minecraft_version) = self.fixed_minecraft_version.clone() {
//                 self.minecraft_version_select_state = Some(cx.new(|cx| {
//                     let mut select_state =
//                         SelectState::new(SearchableVec::new(vec![SharedString::new_static(minecraft_version)]), None, window, cx)
//                             .searchable(true);
//                     select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                     select_state
//                 }));
//             } else {
//                 let mut keys: Vec<SharedString> =
//                     self.version_matrix.keys().cloned().map(SharedString::new_static).collect();
//                 keys.sort_by(|a, b| {
//                     let a_is_snapshot = a.contains("w") || a.contains("pre") || a.contains("rc");
//                     let b_is_snapshot = b.contains("w") || b.contains("pre") || b.contains("rc");
//                     if a_is_snapshot != b_is_snapshot {
//                         if a_is_snapshot {
//                             Ordering::Greater
//                         } else {
//                             Ordering::Less
//                         }
//                     } else {
//                         lexical_sort::natural_lexical_cmp(a, b).reverse()
//                     }
//                 });
//                 self.minecraft_version_select_state = Some(cx.new(|cx| {
//                     let mut select_state =
//                         SelectState::new(SearchableVec::new(keys), None, window, cx).searchable(true);
//                     select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                     select_state
//                 }));
//             }
//         }

//         let selected_minecraft_version = self
//             .minecraft_version_select_state
//             .as_ref()
//             .and_then(|v| v.read(cx).selected_value())
//             .cloned();
//         let game_version_changed = self.last_selected_minecraft_version != selected_minecraft_version;
//         self.last_selected_minecraft_version = selected_minecraft_version.clone();

//         if self.loader_select_state.is_none() || game_version_changed {
//             self.last_selected_minecraft_version = selected_minecraft_version.clone();
//             self.skip_loader_check_for_mod_version = false;

//             if let Some(loader) = self.fixed_loader {
//                 let loader = SharedString::new_static(loader.pretty_name());
//                 self.loader_select_state = Some(cx.new(|cx| {
//                     let mut select_state = SelectState::new(vec![loader], None, window, cx);
//                     select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                     select_state
//                 }));
//             } else if let Some(selected_minecraft_version) = selected_minecraft_version.clone()
//                 && let Some(loaders) = self.version_matrix.get(selected_minecraft_version.as_str())
//             {
//                 if loaders.same_loaders_for_all_versions {
//                     let single_loader = if loaders.loaders.len() == 1 {
//                         SharedString::new_static(loaders.loaders.iter().next().unwrap().pretty_name())
//                     } else {
//                         let mut string = String::new();
//                         let mut first = true;
//                         for loader in loaders.loaders.iter() {
//                             if first {
//                                 first = false;
//                             } else {
//                                 string.push_str(" / ");
//                             }
//                             string.push_str(loader.pretty_name());
//                         }
//                         SharedString::new(string)
//                     };

//                     self.skip_loader_check_for_mod_version = true;
//                     self.loader_select_state = Some(cx.new(|cx| {
//                         let mut select_state = SelectState::new(vec![single_loader], None, window, cx);
//                         select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                         select_state
//                     }));
//                 } else {
//                     let keys: Vec<SharedString> =
//                         loaders.loaders.iter().map(ModrinthLoader::pretty_name).map(SharedString::new_static).collect();

//                     let previous = self
//                         .loader_select_state
//                         .as_ref()
//                         .and_then(|state| state.read(cx).selected_value().cloned());
//                     self.loader_select_state = Some(cx.new(|cx| {
//                         let mut select_state = SelectState::new(keys, None, window, cx);
//                         if let Some(previous) = previous {
//                             select_state.set_selected_value(&previous, window, cx);
//                         }
//                         if select_state.selected_index(cx).is_none() {
//                             select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                         }
//                         select_state
//                     }));
//                 }
//             }
//             if self.loader_select_state.is_none() {
//                 self.loader_select_state = Some(cx.new(|cx| {
//                     let mut select_state = SelectState::new(Vec::new(), None, window, cx);
//                     select_state.set_selected_index(Some(IndexPath::default()), window, cx);
//                     select_state
//                 }));
//             }
//         }

//         let selected_loader = self.loader_select_state.as_ref().and_then(|v| v.read(cx).selected_value()).cloned();
//         let loader_changed = self.last_selected_loader != selected_loader;
//         self.last_selected_loader = selected_loader.clone();

//         if (self.mod_version_select_state.is_none() || game_version_changed || loader_changed)
//             && let Some(selected_game_version) = selected_minecraft_version.clone()
//             && let Some(selected_loader) = self.last_selected_loader.clone()
//         {
//             let selected_game_version = selected_game_version.as_str();

//             let selected_loader = if self.skip_loader_check_for_mod_version {
//                 None
//             } else {
//                 Some(ModrinthLoader::from_name(selected_loader.as_str()))
//             };

//             let mod_versions: Vec<ModVersionItem> = self
//                 .project_versions
//                 .iter()
//                 .filter_map(|version| {
//                     let Some(game_versions) = &version.game_versions else {
//                         return None;
//                     };
//                     let Some(loaders) = &version.loaders else {
//                         return None;
//                     };
//                     if version.files.is_empty() {
//                         return None;
//                     }
//                     let matches_game_version = game_versions.iter().any(|v| v.as_str() == selected_game_version);
//                     let matches_loader = if let Some(selected_loader) = selected_loader {
//                         loaders.contains(&selected_loader)
//                     } else {
//                         true
//                     };
//                     if matches_game_version && matches_loader {
//                         let name = version
//                             .version_number
//                             .clone()
//                             .unwrap_or(version.name.clone().unwrap_or(version.id.clone()));
//                         let mut name = SharedString::new(name);

//                         match version.version_type {
//                             Some(ModrinthVersionType::Beta) => name = format!("{} (Beta)", name).into(),
//                             Some(ModrinthVersionType::Alpha) => name = format!("{} (Alpha)", name).into(),
//                             _ => {},
//                         }

//                         Some(ModVersionItem {
//                             name,
//                             version: version.clone(),
//                         })
//                     } else {
//                         None
//                     }
//                 })
//                 .collect();

//             let mut highest_release = None;
//             let mut highest_beta = None;
//             let mut highest_alpha = None;

//             for (index, version) in mod_versions.iter().enumerate() {
//                 match version.version.version_type {
//                     Some(ModrinthVersionType::Release) => {
//                         highest_release = Some(index);
//                         break;
//                     },
//                     Some(ModrinthVersionType::Beta) => {
//                         if highest_beta.is_none() {
//                             highest_beta = Some(index);
//                         }
//                     },
//                     Some(ModrinthVersionType::Alpha) => {
//                         if highest_alpha.is_none() {
//                             highest_alpha = Some(index);
//                         }
//                     },
//                     _ => {},
//                 }
//             }

//             let highest = highest_release.or(highest_beta).or(highest_alpha);

//             self.mod_version_select_state = Some(cx.new(|cx| {
//                 let mut select_state =
//                     SelectState::new(SearchableVec::new(mod_versions), None, window, cx).searchable(true);
//                 if let Some(index) = highest {
//                     select_state.set_selected_index(Some(IndexPath::default().row(index)), window, cx);
//                 }
//                 select_state
//             }));
//         }

//         let selected_mod_version = self
//             .mod_version_select_state
//             .as_ref()
//             .and_then(|state| state.read(cx).selected_value())
//             .cloned();

//         let mod_version_prefix = match self.project_type {
//             ModrinthProjectType::Mod => "Mod Version: ",
//             ModrinthProjectType::Modpack => "Modpack version: ",
//             ModrinthProjectType::Resourcepack => "Pack version: ",
//             ModrinthProjectType::Shader => "Shader version: ",
//             ModrinthProjectType::Other => "File version: ",
//         };

//         let required_dependencies = selected_mod_version.as_ref().and_then(|version| {
//             version.dependencies.as_ref().map(|deps| {
//                 deps
//                     .iter()
//                     .filter(|dep| {
//                         dep.project_id.is_some() && dep.dependency_type == ModrinthDependencyType::Required
//                     })
//                     .cloned()
//                     .collect::<Arc<[_]>>()
//             })
//         }).unwrap_or_default();

//         let content = v_flex()
//             .gap_2()
//             .child(
//                 Select::new(self.minecraft_version_select_state.as_ref().unwrap())
//                     .disabled(self.fixed_minecraft_version.is_some())
//                     .title_prefix("Game Version: "),
//             )
//             .child(
//                 Select::new(self.loader_select_state.as_ref().unwrap())
//                     .disabled(self.fixed_loader.is_some() || self.skip_loader_check_for_mod_version)
//                     .title_prefix("Loader: "),
//             )
//             .when_some(self.mod_version_select_state.as_ref(), |modal, mod_versions| {
//                 modal
//                     .child(Select::new(mod_versions).title_prefix(mod_version_prefix))
//                     .when(!required_dependencies.is_empty(), |modal| {
//                         modal.child(Checkbox::new("install_deps").checked(self.install_dependencies).label(if required_dependencies.len() == 1 {
//                             SharedString::new_static("Install 1 dependency")
//                         } else {
//                             SharedString::new(format!("Install {} dependencies", required_dependencies.len()))
//                         }).on_click(cx.listener(|dialog, value, _, _| {
//                             dialog.install_dependencies = *value;
//                         })))
//                     })
//                     .child(Button::new("install").success().label("Install").on_click(cx.listener(
//                         move |this, _, window, cx| {
//
//                         },
//                     )),
//                 )
//             });

//         modal.child(content)
//     }
// }

// #[derive(Clone)]
// struct ModVersionItem {
//     name: SharedString,
//     version: ModrinthProjectVersion,
// }

// impl SelectItem for ModVersionItem {
//     type Value = ModrinthProjectVersion;

//     fn title(&self) -> SharedString {
//         self.name.clone()
//     }

//     fn value(&self) -> &Self::Value {
//         &self.version
//     }
// }
