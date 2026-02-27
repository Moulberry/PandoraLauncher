use std::sync::Arc;

use bridge::{install::{ContentDownload, ContentInstall, ContentInstallFile, InstallTarget}, instance::InstanceID, message::MessageToBackend, meta::MetadataRequest, modal_action::ModalAction, safe_path::SafePath};
use gpui::{prelude::*, *};
use gpui_component::{
    h_flex, notification::Notification, spinner::Spinner, WindowExt
};
use relative_path::RelativePath;
use rustc_hash::FxHashSet;
use schema::{
    content::ContentSource, modrinth::{
        ModrinthDependencyType, ModrinthProjectType, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthVersionType
    }
};
use uuid::Uuid;

use crate::{
    entity::{
        DataEntities, metadata::{AsMetadataResult, FrontendMetadata, FrontendMetadataResult, FrontendMetadataState}
    },
    ts,
};

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
            project_id: project_id.clone(),
            game_versions: None,
            loaders: None,
        }),
        cx,
    );

    let key = Uuid::new_v4();
    let title = ts!("instance.content.install.title", name = name);

    if handle_project_versions(data, title.clone(), key, project_id.clone(), project_type, install_for, &project_versions, window, cx) {
        return;
    }

    let _subscription = window.observe(&project_versions, cx, {
        let title = title.clone();
        let data = data.clone();
        move |project_versions, window, cx| {
            handle_project_versions(&data, title.clone(), key, project_id.clone(), project_type, install_for, &project_versions, window, cx);
        }
    });

    let notification = Notification::new()
        .id1::<AutoInstallNotificationType>(key)
        .title(title)
        .content(move |_, _, _| {
            _ = &_subscription;

            h_flex()
                .gap_2()
                .child(ts!("instance.content.load.versions_from_modrinth.title"))
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
    project_id: Arc<str>,
    project_type: ModrinthProjectType,
    install_for: InstanceID,
    project_versions: &Entity<FrontendMetadataState>,
    window: &mut Window,
    cx: &mut App
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
            let (configuration, instance_mods) = {
                let instance = instance.read(cx);
                (instance.configuration.clone(), instance.mods.clone())
            };
            let modrinth_loader = configuration.loader.as_modrinth_loader();
            let is_mod = project_type == ModrinthProjectType::Mod || project_type == ModrinthProjectType::Modpack;
            let matching_versions = project_versions.0.iter().filter(|version| {
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
                if is_mod && !loaders.contains(&modrinth_loader) {
                    return false;
                }
                true
            }).collect::<Vec<_>>();

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
                push_error(title.clone(), key, ts!("instance.content.install.no_matching_versions"), window, cx);
                return true;
            };

            let version = matching_versions[highest];

            let install_file = version
                .files
                .iter()
                .find(|file| file.primary)
                .unwrap_or(version.files.first().unwrap());

            let path = match project_type {
                ModrinthProjectType::Mod => RelativePath::new("mods").join(&*install_file.filename),
                ModrinthProjectType::Modpack => RelativePath::new("mods").join(&*install_file.filename),
                ModrinthProjectType::Resourcepack => RelativePath::new("resourcepacks").join(&*install_file.filename),
                ModrinthProjectType::Shader => RelativePath::new("shaderpacks").join(&*install_file.filename),
                ModrinthProjectType::Other => {
                    push_error(title.clone(), key, ts!("instance.content.install.unable_other_type"), window, cx);
                    return true;
                },
            };

            let Some(path) = SafePath::from_relative_path(&path) else {
                push_error(title.clone(), key, ts!("instance.content.install.invalid_filename"), window, cx);
                return true;
            };

            let mut files = Vec::new();

            let required_dependencies = version.dependencies.as_ref().map(|deps| {
                let mut required = deps
                    .iter()
                    .filter(|dep| {
                        dep.project_id.is_some() && dep.dependency_type == ModrinthDependencyType::Required
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                // Ignore projects that are already installed
                if !required.is_empty() {
                    let mut existing_projects = FxHashSet::default();
                    let existing_mods = instance_mods.read(cx);
                    for summary in existing_mods.iter() {
                        let ContentSource::ModrinthProject { project } = &summary.content_source else {
                            continue;
                        };
                        existing_projects.insert(project.clone());
                    }
                    required.retain(|dep| !existing_projects.contains(dep.project_id.as_ref().unwrap()));
                }

                required
            });

            if let Some(required_dependencies) = required_dependencies {
                for dep in required_dependencies.iter() {
                    files.push(ContentInstallFile {
                        replace_old: None,
                        path: bridge::install::ContentInstallPath::Automatic,
                        download: ContentDownload::Modrinth {
                            project_id: dep.project_id.clone().unwrap(),
                            version_id: dep.version_id.clone()
                        },
                        content_source: ContentSource::ModrinthProject {
                            project: dep.project_id.clone().unwrap()
                        },
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
                    project: project_id
                },
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

            crate::modals::generic::show_notification_with_note(window, cx, ts!("instance.content.install.error"), modal_action,
                Notification::new().id1::<AutoInstallNotificationType>(key));

            return true;
        },
        FrontendMetadataResult::Error(error) => {
            push_error(title.clone(), key, ts!("instance.content.load.versions_from_modrinth.error", err = format!("\n{}", error)), window, cx);
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
