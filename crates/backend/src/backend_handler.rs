use std::{borrow::Cow, io::{BufRead, Read}, sync::{Arc, atomic::Ordering}, time::{Duration, Instant, SystemTime}};

use auth::{credentials::AccountCredentials, models::MinecraftAccessToken, secret::PlatformSecretStorage};
use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, instance::{ContentFolder, ContentSummary, ContentType, InstanceID}, keep_alive::KeepAlive, message::{AccountCapesResult, AccountSkinResult, BackendConfigWithPassword, EmbeddedOrRaw, LogFiles, MessageToBackend, MessageToFrontend, QuickPlayLaunch}, meta::MetadataResult, modal_action::{ModalAction, ModalActionVisitUrl, ProgressTracker, ProgressTrackerFinishType}, serial::AtomicOptionSerial
};
use futures::TryFutureExt;
use schema::{auxiliary::AuxiliaryContentMeta, content::{ContentInstallReason, ContentSource}, curseforge::{CurseforgeGetModFilesRequest, CurseforgeModLoaderType}, loader::Loader, minecraft_profile::{MinecraftProfileResponse, SkinVariant}, modrinth::ModrinthLoader, version::{LaunchArgument, LaunchArgumentValue}};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tokio::{io::AsyncBufReadExt, sync::{Semaphore, TryAcquireError}};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    BackendState, CachedMinecraftProfile, FolderChanges, LoginError, account::BackendAccount, arcfactory::ArcStrFactory, instance::Instance, launch::{ArgumentExpansionKey, LaunchError}, log_reader, metadata::{items::{AssetsIndexMetadataItem, CurseforgeGetModFilesMetadataItem, CurseforgeSearchMetadataItem, FabricLoaderManifestMetadataItem, ForgeInstallerMavenMetadataItem, MinecraftVersionManifestMetadataItem, MinecraftVersionMetadataItem, ModrinthProjectMetadataItem, ModrinthProjectVersionsMetadataItem, ModrinthSearchMetadataItem, ModrinthV3VersionUpdateMetadataItem, ModrinthVersionUpdateMetadataItem, MojangJavaRuntimeComponentMetadataItem, MojangJavaRuntimesMetadataItem, NeoforgeInstallerMavenMetadataItem, VersionUpdateParameters, VersionV3LoaderFields, VersionV3UpdateParameters}, manager::MetaLoadError}, mod_metadata::{ContentUpdateAction, ContentUpdateKey}, skin_manager::SkinManager
};

impl BackendState {
    pub async fn handle_message(self: &Arc<Self>, message: MessageToBackend) {
        match message {
            MessageToBackend::RequestMetadata { request, force_reload } => {
                let meta = self.meta.clone();
                let send = self.send.clone();
                tokio::task::spawn(async move {
                    let (result, keep_alive_handle) = match request {
                        bridge::meta::MetadataRequest::MinecraftVersionManifest => {
                            let (result, handle) = meta.fetch_with_keepalive(&MinecraftVersionManifestMetadataItem, force_reload).await;
                            (result.map(MetadataResult::MinecraftVersionManifest), handle)
                        },
                        bridge::meta::MetadataRequest::FabricLoaderManifest => {
                            let (result, handle) = meta.fetch_with_keepalive(&FabricLoaderManifestMetadataItem, force_reload).await;
                            (result.map(MetadataResult::FabricLoaderManifest), handle)
                        },
                        bridge::meta::MetadataRequest::ForgeMavenManifest => {
                            let (result, handle) = meta.fetch_with_keepalive(&ForgeInstallerMavenMetadataItem, force_reload).await;
                            (result.map(MetadataResult::ForgeMavenManifest), handle)
                        },
                        bridge::meta::MetadataRequest::NeoforgeMavenManifest => {
                            let (result, handle) = meta.fetch_with_keepalive(&NeoforgeInstallerMavenMetadataItem, force_reload).await;
                            (result.map(MetadataResult::NeoforgeMavenManifest), handle)
                        },
                        bridge::meta::MetadataRequest::ModrinthSearch(ref search) => {
                            let (result, handle) = meta.fetch_with_keepalive(&ModrinthSearchMetadataItem(search), force_reload).await;
                            (result.map(MetadataResult::ModrinthSearchResult), handle)
                        },
                        bridge::meta::MetadataRequest::ModrinthProjectVersions(ref project_versions) => {
                            let (result, handle) = meta.fetch_with_keepalive(&ModrinthProjectVersionsMetadataItem(project_versions), force_reload).await;
                            (result.map(MetadataResult::ModrinthProjectVersionsResult), handle)
                        },
                        bridge::meta::MetadataRequest::ModrinthProject(ref project) => {
                            let (result, handle) = meta.fetch_with_keepalive(&ModrinthProjectMetadataItem(project), force_reload).await;
                            (result.map(MetadataResult::ModrinthProjectResult), handle)
                        },
                        bridge::meta::MetadataRequest::CurseforgeSearch(ref search) => {
                            let (result, handle) = meta.fetch_with_keepalive(&CurseforgeSearchMetadataItem(search), force_reload).await;
                            (result.map(MetadataResult::CurseforgeSearchResult), handle)
                        },
                        bridge::meta::MetadataRequest::CurseforgeGetModFiles(ref request) => {
                            let (result, handle) = meta.fetch_with_keepalive(&CurseforgeGetModFilesMetadataItem(request), force_reload).await;
                            (result.map(MetadataResult::CurseforgeGetModFilesResult), handle)
                        },
                    };
                    let result = result.map_err(|err| format!("{}", err).into());
                    send.send(MessageToFrontend::MetadataResult {
                        request,
                        result,
                        keep_alive_handle
                    });
                });
            },
            MessageToBackend::RequestLoadWorlds { id } => {
                tokio::task::spawn(Instance::load_worlds(self.clone(), id));
            },
            MessageToBackend::RequestLoadServers { id } => {
                tokio::task::spawn(Instance::load_servers(self.clone(), id));
            },
            MessageToBackend::ReorderServers { id, from_index, to_index } => {
                tokio::task::spawn(Instance::reorder_servers(self.clone(), id, from_index, to_index));
            },
            MessageToBackend::RequestLoadContentFolder { id, content_folder } => {
                tokio::task::spawn(Instance::load_content(self.clone(), id, content_folder));
            },
            MessageToBackend::CreateInstance { name, version, loader, icon } => {
                self.create_instance(&name, &version, loader, icon).await;
            },
            MessageToBackend::DeleteInstance { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let result = std::fs::remove_dir_all(&instance.root_path);
                    if let Err(err) = result {
                        self.send.send_error(format!("Unable to delete instance folder: {}", err));
                    }
                }
            },
            MessageToBackend::ExportInstance { id, format, options, output, modal_action } => {
                let backend = self.clone();
                tokio::task::spawn(async move {
                    crate::export::export_instance(backend, id, format, options, output, modal_action).await;
                });
            },
            MessageToBackend::RenameInstance { id, name } => {
                self.rename_instance(id, &name).await;
            },
            MessageToBackend::SetInstanceMinecraftVersion { id, version } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.minecraft_version = version;
                    });
                }
            },
            MessageToBackend::SetInstanceLoader { id, loader } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.loader = loader;
                        configuration.preferred_loader_version = None;
                    });
                }
            },
            MessageToBackend::SetInstancePreferredAccount { id, account } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                       instance.configuration.modify(|configuration| {
                           configuration.preferred_account = account;
                      });
                 }
            }
            MessageToBackend::SetInstancePreferredLoaderVersion { id, loader_version } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.preferred_loader_version = loader_version.map(Ustr::from);
                    });
                }
            },
            MessageToBackend::SetInstanceDisableFileSyncing { id, disable_file_syncing } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.disable_file_syncing = disable_file_syncing;
                    });
                }
                self.apply_syncing_to_instance(id);
            },
            MessageToBackend::SetInstanceSandboxing { id, sandbox } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.sandbox = sandbox;
                    });
                }
            },
            MessageToBackend::SetInstanceMemory { id, memory } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.memory = Some(memory);
                    });
                }
            },
            MessageToBackend::SetInstanceWrapperCommand { id, wrapper_command } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.wrapper_command = Some(wrapper_command);
                    });
                }
            },
            MessageToBackend::SetInstanceJvmFlags { id, jvm_flags } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.jvm_flags = Some(jvm_flags);
                    });
                }
            },
            MessageToBackend::SetInstanceJvmBinary { id, jvm_binary } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.jvm_binary = Some(jvm_binary);
                    });
                }
            },
            MessageToBackend::SetInstanceLinuxWrapper { id, linux_wrapper } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.linux_wrapper = Some(linux_wrapper);
                    });
                }
            },
            MessageToBackend::SetInstanceSystemLibraries { id, system_libraries } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.configuration.modify(|configuration| {
                        configuration.system_libraries = Some(system_libraries);
                    });
                }
            },
            MessageToBackend::SetInstanceIcon { id, icon } => {
                let root_path = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let root_path = instance.root_path.clone();
                    instance.configuration.modify(|configuration| {
                        configuration.instance_fallback_icon = None;
                        if let Some(EmbeddedOrRaw::Embedded(ref e)) = icon {
                            configuration.instance_fallback_icon = Some(Ustr::from(e));
                        }
                    });
                    root_path
                } else {
                    return;
                };

                match icon {
                    Some(EmbeddedOrRaw::Raw(image_bytes)) => {
                        if let Ok(format) = image::guess_format(&*image_bytes) {
                            if format == image::ImageFormat::Png {
                                let icon_path = root_path.join("icon.png");
                                if let Err(err) = crate::write_safe(&icon_path, &*image_bytes) {
                                    log::error!("Unable to save instance icon: {:?}", err);
                                    self.send.send_error("Unable to save instance icon");
                                    return;
                                }
                                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                                    instance.icon = Some(image_bytes);
                                    self.send.send(instance.create_modify_message());
                                }
                            } else {
                                self.send.send_error("Unable to apply icon: only pngs are supported");
                            }
                        } else {
                            self.send.send_error("Unable to apply icon: unknown format");
                        }
                    },
                    Some(EmbeddedOrRaw::Embedded(_)) => {
                        let icon_path = root_path.join("icon.png");
                        if icon_path.exists() {
                            let _ = std::fs::remove_file(&icon_path);
                        }
                        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                            instance.icon = None;
                            self.send.send(instance.create_modify_message());
                        }
                    },
                    None => {
                        let icon_path = root_path.join("icon.png");
                        if icon_path.exists() {
                            let _ = std::fs::remove_file(&icon_path);
                        }
                        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                            instance.icon = None;
                            self.send.send(instance.create_modify_message());
                        }
                    },
                }
            },
            MessageToBackend::KillInstance { id } => {
                let mut instance_state = self.instance_state.write();
                let Some(instance) = instance_state.instances.get_mut(id) else {
                    self.send.send_error("Can't kill instance, unknown id");
                    return;
                };

                if instance.processes.is_empty() && instance.closing_processes.is_empty() {
                    self.send.send_error("Can't kill instance, instance wasn't running");
                    return;
                }

                for (process, _) in instance.closing_processes.drain(..) {
                    let result = process.kill();

                    if let Err(err) = result {
                        self.send.send_error("Failed to kill instance");
                        log::error!("Failed to kill instance: {err:?}");
                    }
                }

                let now = Instant::now();
                for mut process in instance.processes.drain(..) {
                    let mut result = process.close();
                    if result.is_err() {
                        result = process.kill();
                    } else {
                        instance.closing_processes.push((process, now + Duration::from_secs(3)));
                    }

                    if let Err(err) = result {
                        self.send.send_error("Failed to kill instance");
                        log::error!("Failed to kill instance: {err:?}");
                    }
                }

                instance.update_session();
                self.send.send(instance.create_modify_message());
                self.restore_mods_folder_if_stopped(instance);
            },
            MessageToBackend::StartInstanceByName { name, quick_play } => {
                let mut id = None;

                for instance in self.instance_state.read().instances.iter() {
                    if instance.name == &name {
                        id = Some(instance.id);
                        break;
                    } else if instance.name.eq_ignore_ascii_case(&name) {
                        id = Some(instance.id);
                    }
                }

                if let Some(id) = id {
                    self.start_instance(id, quick_play, Default::default()).await
                }
            },
            MessageToBackend::StartInstance {
                id,
                quick_play,
                modal_action,
            } => {
                self.start_instance(id, quick_play, modal_action).await
            },
            MessageToBackend::SetContentEnabled { id, content_ids: mod_ids, enabled } => {
                let mut instance_state = self.instance_state.write();
                let Some(instance) = instance_state.instances.get_mut(id) else {
                    return;
                };

                let mut cannot_modify_while_running = false;

                for mod_id in mod_ids {
                    if let Some((instance_mod, folder)) = instance.try_get_content(mod_id) {
                        if instance_mod.enabled == enabled {
                            continue;
                        }

                        if folder == ContentFolder::Mods && !instance.processes.is_empty() {
                            cannot_modify_while_running = true;
                            continue;
                        }

                        let mut new_path = instance_mod.path.to_path_buf();
                        if instance_mod.enabled {
                            new_path.add_extension("disabled");
                        } else {
                            new_path.set_extension("");
                        };

                        let _ = std::fs::rename(&instance_mod.path, new_path);
                    }
                }

                if cannot_modify_while_running {
                    self.send.send_warning("Cannot modify mods folder while instance is running");
                }
            },
            MessageToBackend::SetContentChildEnabled { id, content_id: mod_id, child_id, child_name, child_filename, disabled_default, enabled } => {
                let mut instance_state = self.instance_state.write();
                if let Some(instance) = instance_state.instances.get_mut(id)
                    && let Some((instance_mod, folder)) = instance.try_get_content(mod_id)
                {
                    let Some(aux_path) = crate::pandora_aux_path_for_content(instance_mod) else {
                        return;
                    };

                    if folder == ContentFolder::Mods && !instance.processes.is_empty() {
                        self.send.send_warning("Cannot modify mods folder while instance is running");
                        return;
                    }

                    let mut aux: AuxiliaryContentMeta = crate::read_json(&aux_path).unwrap_or_default();

                    let mut changed = false;

                    if disabled_default {
                        if enabled {
                            if let Some(child_id) = child_id {
                                changed |= aux.disabled_children.enabled_ids.insert(child_id);
                            } else if let Some(child_name) = child_name {
                                changed |= aux.disabled_children.enabled_names.insert(child_name);
                            } else {
                                changed |= aux.disabled_children.enabled_filenames.insert(child_filename);
                            }
                        } else {
                            if let Some(child_id) = child_id {
                                changed |= aux.disabled_children.enabled_ids.remove(&child_id);
                            }
                            if let Some(child_name) = child_name {
                                changed |= aux.disabled_children.enabled_names.remove(&child_name);
                            }
                            changed |= aux.disabled_children.enabled_filenames.remove(&child_filename);
                        }
                    } else {
                        if enabled {
                            if let Some(child_id) = child_id {
                                changed |= aux.disabled_children.disabled_ids.remove(&child_id);
                            }
                            if let Some(child_name) = child_name {
                                changed |= aux.disabled_children.disabled_names.remove(&child_name);
                            }
                            changed |= aux.disabled_children.disabled_filenames.remove(&child_filename);
                        } else {
                            if let Some(child_id) = child_id {
                                changed |= aux.disabled_children.disabled_ids.insert(child_id);
                            } else if let Some(child_name) = child_name {
                                changed |= aux.disabled_children.disabled_names.insert(child_name);
                            } else {
                                changed |= aux.disabled_children.disabled_filenames.insert(child_filename);
                            }
                        }
                    }

                    if changed {
                        let bytes = match serde_json::to_vec(&aux) {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                log::error!("Unable to serialize AuxiliaryContentMeta: {err:?}");
                                self.send.send_error("Unable to serialize AuxiliaryContentMeta");
                                return;
                            },
                        };
                        if let Err(err) = crate::write_safe(&aux_path, &bytes) {
                            log::error!("Unable to save aux meta: {err:?}");
                            self.send.send_error("Unable to save aux meta");
                        }
                    }
                }
            },
            MessageToBackend::DownloadContentChildren { id, content_id, modal_action } => {
                let (summary, loader, minecraft_version) = {
                    let mut instance_state = self.instance_state.write();
                    let Some(instance) = instance_state.instances.get_mut(id) else {
                        return;
                    };
                    let Some((summary, _)) = instance.try_get_content(content_id) else {
                        return;
                    };
                    let summary = summary.clone();
                    let configuration = instance.configuration.get();
                    (summary, configuration.loader, configuration.minecraft_version)
                };

                self.download_modpack_children(&summary, loader, minecraft_version, &modal_action).await;

                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let mut changes = FolderChanges::no_changes();
                    changes.dirty_path(summary.path);
                    instance.mark_content_dirty(self, ContentFolder::Mods, changes, true);
                }
            },
            MessageToBackend::DownloadAllMetadata => {
                self.download_all_metadata().await;
            },
            MessageToBackend::InstallContent { content, modal_action } => {
                let this = self.clone();

                tokio::spawn(async move {
                    this.install_content(content, modal_action.clone()).await;
                    modal_action.set_finished();
                    this.send.send(MessageToFrontend::Refresh);
                });
            },
            MessageToBackend::CreateInstanceFromFile { file, modal_action } => {
                let summary = self.mod_metadata_manager.get_path(&file);

                // right now only .mrpack importing is used
                let ContentType::ModrinthModpack { dependencies, .. } = &summary.extra else {
                    modal_action.set_error_message("Not a .mrpack file".into());
                    modal_action.set_finished();
                    return;
                };

                let Some(name) = summary.name.clone() else {
                    modal_action.set_error_message("Unable to determine name from modpack".into());
                    modal_action.set_finished();
                    return;
                };

                let mut minecraft_version = None;
                let mut loader = Loader::Vanilla;
                for (key, value) in dependencies {
                    match &**key {
                        "forge" => loader = Loader::Forge,
                        "neoforge" => loader = Loader::NeoForge,
                        "fabric-loader" => loader = Loader::Fabric,
                        "minecraft" => minecraft_version = Some(value.clone()),
                        _ => {}
                    }
                }

                let Some(minecraft_version) = minecraft_version else {
                    modal_action.set_error_message("Unable to determine minecraft version from modpack".into());
                    modal_action.set_finished();
                    return;
                };

                let content_install = ContentInstall {
                    target: InstallTarget::NewInstance { name: Some(name) },
                    loader,
                    minecraft_version: minecraft_version.into(),
                    files: Arc::from([
                        ContentInstallFile {
                            replace_old: None,
                            path: ContentInstallPath::Automatic,
                            download: ContentDownload::File { path: file },
                            content_source: ContentSource::Manual,
                            reason: ContentInstallReason::Standalone,
                        }
                    ]),
                };

                let this = self.clone();

                tokio::spawn(async move {
                    this.install_content(content_install, modal_action.clone()).await;
                    modal_action.set_finished();
                    this.send.send(MessageToFrontend::Refresh);
                });
            },
            MessageToBackend::DeleteContent { id, content_ids: mod_ids } => {
                let mut instance_state = self.instance_state.write();
                let Some(instance) = instance_state.instances.get_mut(id) else {
                    self.send.send_error("Unable to find instance, unknown id");
                    return;
                };

                let mut cannot_modify_while_running = false;

                for mod_id in mod_ids {
                    let Some((instance_mod, folder)) = instance.try_get_content(mod_id) else {
                        self.send.send_error("Unable to delete mod, invalid id");
                        continue;
                    };

                    if folder == ContentFolder::Mods && !instance.processes.is_empty() {
                        cannot_modify_while_running = true;
                        continue;
                    }

                    _ = std::fs::remove_file(&instance_mod.path);

                    if let Some(aux_path) = crate::pandora_aux_path_for_content(&instance_mod) {
                        _ = std::fs::remove_file(aux_path);
                    }
                }

                if cannot_modify_while_running {
                    self.send.send_warning("Cannot modify mods folder while instance is running");
                }
            },
            MessageToBackend::UpdateCheck { instance: id, modal_action } => {
                let (loader, version) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let configuration = instance.configuration.get();
                    (configuration.loader, configuration.minecraft_version)
                } else {
                    self.send.send_error("Can't update instance, unknown id");
                    modal_action.set_error_message("Can't update instance, unknown id".into());
                    modal_action.set_finished();
                    return;
                };

                let mut content = Vec::new();
                for folder in ContentFolder::iter() {
                    let Some(summaries) = Instance::load_content(self.clone(), id, folder).await else {
                        modal_action.set_finished();
                        return;
                    };
                    content.extend_from_slice(&*summaries);
                }

                let modrinth_loader = loader.as_modrinth_loader();
                if modrinth_loader == ModrinthLoader::Unknown {
                    modal_action.set_error_message("Unable to update instance, unsupported loader".into());
                    modal_action.set_finished();
                    return;
                }

                let tracker = ProgressTracker::new("Checking content".into(), self.send.clone());
                tracker.set_total(content.len());
                modal_action.trackers.push(tracker.clone());

                let semaphore = Semaphore::new(8);

                let mod_params = &VersionUpdateParameters {
                    loaders: [modrinth_loader].into(),
                    game_versions: [version].into(),
                };

                let fabric_mod_params = &VersionUpdateParameters {
                    loaders: [ModrinthLoader::Fabric].into(),
                    game_versions: [version].into(),
                };

                let forge_mod_params = &VersionUpdateParameters {
                    loaders: [ModrinthLoader::Forge].into(),
                    game_versions: [version].into(),
                };

                let neoforge_mod_params = &VersionUpdateParameters {
                    loaders: [ModrinthLoader::NeoForge].into(),
                    game_versions: [version].into(),
                };

                let resourcepack_params = &VersionUpdateParameters {
                    loaders: [ModrinthLoader::Minecraft].into(),
                    game_versions: [version].into(),
                };

                let shaderpack_params = &VersionUpdateParameters {
                    loaders: [ModrinthLoader::Iris, ModrinthLoader::Optifine, ModrinthLoader::Canvas].into(),
                    game_versions: [version].into(),
                };

                let modrinth_modpack_params = &VersionV3UpdateParameters {
                    loaders: ["mrpack".into()].into(),
                    loader_fields: VersionV3LoaderFields {
                        mrpack_loaders: [modrinth_loader].into(),
                        game_versions: [version].into(),
                    },
                };

                let meta = self.meta.clone();

                let mut futures = Vec::new();

                struct UpdateResult {
                    mod_summary: Arc<ContentSummary>,
                    action: ContentUpdateAction,
                }

                { // Scope is needed so await doesn't complain about the non-send RwLockReadGuard
                    let sources = self.mod_metadata_manager.read_content_sources();
                    for summary in content.iter() {
                        let source = sources.get(&summary.content_summary.hash).unwrap_or(ContentSource::Manual);
                        let semaphore = &semaphore;
                        let meta = &meta;
                        let tracker = &tracker;
                        futures.push(async move {
                            match source {
                                ContentSource::Manual => {
                                    tracker.add_count(1);
                                    tracker.notify();
                                    Ok(ContentUpdateAction::ManualInstall)
                                },
                                ContentSource::ModrinthUnknown | ContentSource::ModrinthProject { .. } => {
                                    let permit = semaphore.acquire().await.unwrap();
                                    let result = match summary.content_summary.extra {
                                        ContentType::Fabric => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: fabric_mod_params.clone()
                                            }).await
                                        },
                                        ContentType::Forge | ContentType::LegacyForge => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: forge_mod_params.clone()
                                            }).await
                                        },
                                        ContentType::NeoForge => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: neoforge_mod_params.clone()
                                            }).await
                                        },
                                        ContentType::JavaModule | ContentType::CurseforgeModpack { .. } | ContentType::Unknown => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: mod_params.clone()
                                            }).await
                                        },
                                        ContentType::ModrinthModpack { .. } => {
                                            meta.fetch(&ModrinthV3VersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: modrinth_modpack_params.clone()
                                            }).await
                                        },
                                        ContentType::ResourcePack => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: resourcepack_params.clone()
                                            }).await
                                        },
                                        ContentType::ShaderPack => {
                                            meta.fetch(&ModrinthVersionUpdateMetadataItem {
                                                sha1: hex::encode(summary.content_summary.hash).into(),
                                                params: shaderpack_params.clone()
                                            }).await
                                        },
                                    };
                                    drop(permit);

                                    tracker.add_count(1);
                                    tracker.notify();

                                    if let Err(MetaLoadError::NonOK(404)) = result {
                                        return Ok(ContentUpdateAction::ErrorNotFound);
                                    }

                                    let result = result?;

                                    if let ContentSource::ModrinthProject { ref project_id } = source {
                                        if &result.0.project_id != project_id {
                                            log::error!("Refusing to update {:?}, mismatched project ids: expected {}, got {}",
                                                summary.content_summary.hash, project_id, &result.0.project_id);
                                            return Ok(ContentUpdateAction::ErrorNotFound);
                                        }
                                    }

                                    let install_file = result
                                        .0
                                        .files
                                        .iter()
                                        .find(|file| file.primary)
                                        .unwrap_or(result.0.files.first().unwrap());

                                    let mut latest_hash = [0u8; 20];
                                    let Ok(_) = hex::decode_to_slice(&*install_file.hashes.sha1, &mut latest_hash) else {
                                        return Ok(ContentUpdateAction::ErrorInvalidHash);
                                    };

                                    if latest_hash == summary.content_summary.hash {
                                        Ok(ContentUpdateAction::AlreadyUpToDate)
                                    } else {
                                        Ok(ContentUpdateAction::Modrinth {
                                            file: install_file.clone(),
                                            project_id: result.0.project_id.clone(),
                                        })
                                    }
                                },
                                ContentSource::CurseforgeProject { project_id } => {
                                    let permit = semaphore.acquire().await.unwrap();

                                    let mod_loader_type = match summary.content_summary.extra {
                                        ContentType::Fabric => {
                                            Some(CurseforgeModLoaderType::Fabric as u32)
                                        },
                                        ContentType::Forge | ContentType::LegacyForge => {
                                            Some(CurseforgeModLoaderType::Forge as u32)
                                        },
                                        ContentType::NeoForge => {
                                            Some(CurseforgeModLoaderType::NeoForge as u32)
                                        },
                                        _ => None
                                    };

                                    let result = self.meta.fetch(&CurseforgeGetModFilesMetadataItem(&CurseforgeGetModFilesRequest {
                                        mod_id: project_id,
                                        game_version: Some(version),
                                        mod_loader_type,
                                        page_size: Some(1)
                                    })).await;

                                    drop(permit);

                                    tracker.add_count(1);
                                    tracker.notify();

                                    if let Err(MetaLoadError::NonOK(404)) = result {
                                        return Ok(ContentUpdateAction::ErrorNotFound);
                                    }

                                    let result = result?;

                                    let Some(file) = result.data.first() else {
                                        return Ok(ContentUpdateAction::ErrorNotFound);
                                    };

                                    if file.mod_id != project_id {
                                        log::error!("Refusing to update {:?}, mismatched project ids: expected {}, got {}",
                                            summary.content_summary.hash, project_id, file.mod_id);
                                        return Ok(ContentUpdateAction::ErrorNotFound);
                                    }

                                    let sha1 = file.hashes.iter()
                                        .find(|hash| hash.algo == 1).map(|hash| &hash.value);
                                    let Some(sha1) = sha1 else {
                                        return Ok(ContentUpdateAction::ErrorInvalidHash);
                                    };

                                    let mut latest_hash = [0u8; 20];
                                    let Ok(_) = hex::decode_to_slice(&**sha1, &mut latest_hash) else {
                                        return Ok(ContentUpdateAction::ErrorInvalidHash);
                                    };

                                    if latest_hash == summary.content_summary.hash {
                                        Ok(ContentUpdateAction::AlreadyUpToDate)
                                    } else {
                                        Ok(ContentUpdateAction::Curseforge {
                                            file: file.clone(),
                                            project_id,
                                        })
                                    }
                                }
                            }
                        }.map_ok(|action| UpdateResult {
                            mod_summary: summary.content_summary.clone(),
                            action,
                        }));
                    }
                }

                let results: Result<Vec<UpdateResult>, MetaLoadError> = futures::future::try_join_all(futures).await;

                match results {
                    Ok(updates) => {
                        let mut meta_updates = self.mod_metadata_manager.updates.write();

                        for update in updates {
                            meta_updates.insert(ContentUpdateKey {
                                hash: update.mod_summary.hash,
                                loader,
                                version,
                            }, update.action);
                        }

                        drop(meta_updates);

                        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                            for content_folder in ContentFolder::iter() {
                                instance.mark_content_dirty(self, content_folder, FolderChanges::all_dirty(), true);
                            }
                        }
                    },
                    Err(error) => {
                        tracker.set_finished(ProgressTrackerFinishType::Error);
                        modal_action.set_error_message(format!("Error checking for updates: {}", error).into());
                        modal_action.set_finished();
                        return;
                    },
                }

                tracker.set_finished(ProgressTrackerFinishType::Normal);
                modal_action.set_finished();
            },
            MessageToBackend::UpdateContent { instance: id, content_id: mod_id, modal_action } => {
                let content_install = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let configuration = instance.configuration.get();
                    let (loader, minecraft_version) = (configuration.loader, configuration.minecraft_version);
                    let Some((mod_summary, _)) = instance.try_get_content(mod_id) else {
                        self.send.send_error("Can't update mod in instance, unknown mod id");
                        modal_action.set_finished();
                        return;
                    };

                    let Some(update_info) = self.mod_metadata_manager.updates.read().get(&ContentUpdateKey {
                        hash: mod_summary.content_summary.hash,
                        loader: loader,
                        version: minecraft_version
                    }).cloned() else {
                        self.send.send_error("Can't update mod in instance, missing update action");
                        modal_action.set_finished();
                        return;
                    };

                    match update_info {
                        ContentUpdateAction::ErrorNotFound => {
                            self.send.send_error("Can't update mod in instance, 404 not found");
                            modal_action.set_finished();
                            return;
                        },
                        ContentUpdateAction::ErrorInvalidHash => {
                            self.send.send_error("Can't update mod in instance, returned invalid hash");
                            modal_action.set_finished();
                            return;
                        },
                        ContentUpdateAction::AlreadyUpToDate => {
                            self.send.send_error("Can't update mod in instance, already up-to-date");
                            modal_action.set_finished();
                            return;
                        },
                        ContentUpdateAction::ManualInstall => {
                            self.send.send_error("Can't update mod in instance, mod was manually installed");
                            modal_action.set_finished();
                            return;
                        },
                        ContentUpdateAction::Modrinth { file, project_id } => {
                            let mut path = mod_summary.path.with_file_name(&*file.filename);
                            if !mod_summary.enabled {
                                path.add_extension("disabled");
                            }

                            let mut hash = [0u8; 20];
                            let Ok(_) = hex::decode_to_slice(&*file.hashes.sha1, &mut hash) else {
                                log::warn!("File {} has invalid sha1: {}", file.filename, file.hashes.sha1);
                                return;
                            };

                            debug_assert!(path.is_absolute());
                            ContentInstall {
                                target: InstallTarget::Instance(id),
                                loader,
                                minecraft_version,
                                files: [ContentInstallFile {
                                    replace_old: Some(mod_summary.path.clone()),
                                    path: bridge::install::ContentInstallPath::Raw(path.into()),
                                    download: ContentDownload::Url {
                                        url: file.url.clone(),
                                        sha1: hash,
                                        size: file.size,
                                    },
                                    content_source: ContentSource::ModrinthProject { project_id },
                                    reason: ContentInstallReason::Update,
                                }].into(),
                            }
                        },
                        ContentUpdateAction::Curseforge { file, project_id } => {
                            let mut path = mod_summary.path.with_file_name(&*file.file_name);
                            if !mod_summary.enabled {
                                path.add_extension("disabled");
                            }
                            debug_assert!(path.is_absolute());

                            let sha1 = file.hashes.iter()
                                .find(|hash| hash.algo == 1).map(|hash| &hash.value);
                            let Some(sha1) = sha1 else {
                                self.send.send_error("Can't update mod in instance, missing sha1 hash");
                                modal_action.set_finished();
                                return;
                            };

                            let mut hash = [0u8; 20];
                            let Ok(_) = hex::decode_to_slice(&**sha1, &mut hash) else {
                                log::warn!("File {} has invalid sha1: {}", file.file_name, sha1);
                                return;
                            };

                            let Some(url) = file.download_url.clone() else {
                                self.send.send_error("Can't update mod in instance, author has blocked third party downloads");
                                modal_action.set_finished();
                                return;
                            };

                            ContentInstall {
                                target: InstallTarget::Instance(id),
                                loader,
                                minecraft_version,
                                files: [ContentInstallFile {
                                    replace_old: Some(mod_summary.path.clone()),
                                    path: bridge::install::ContentInstallPath::Raw(path.into()),
                                    download: ContentDownload::Url {
                                        url,
                                        sha1: hash,
                                        size: file.file_length as usize,
                                    },
                                    content_source: ContentSource::CurseforgeProject { project_id },
                                    reason: ContentInstallReason::Update,
                                }].into(),
                            }
                        },
                    }
                } else {
                    self.send.send_error("Can't update mod in instance, unknown instance id");
                    modal_action.set_finished();
                    return;
                };

                self.install_content(content_install, modal_action.clone()).await;
                modal_action.set_finished();
                self.send.send(MessageToFrontend::Refresh);
            },
            MessageToBackend::Sleep5s => {
                tokio::time::sleep(Duration::from_secs(5)).await;
            },
            MessageToBackend::ReadLog { path, send } => {
                let frontend = self.send.clone();
                let serial = AtomicOptionSerial::default();

                let file = match std::fs::File::open(path) {
                    Ok(file) => file,
                    Err(e) => {
                        let error = format!("Unable to read file: {e}");
                        for line in error.split('\n') {
                            let replaced = log_reader::replace(line.trim_ascii_end());
                            if send.send(replaced.into()).await.is_err() {
                                return;
                            }
                        }
                        frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                        return;
                    },
                };

                let mut reader = std::io::BufReader::new(file);
                let Ok(buffer) = reader.fill_buf() else {
                    return;
                };
                if buffer.len() >= 2 && buffer[0] == 0x1F && buffer[1] == 0x8B {
                    let gz_decoder = flate2::bufread::GzDecoder::new(reader);
                    let mut buf_reader = std::io::BufReader::new(gz_decoder);
                    tokio::task::spawn_blocking(move || {
                        let mut line = String::new();
                        let mut factory = ArcStrFactory::default();
                        loop {
                            match buf_reader.read_line(&mut line) {
                                Ok(0) => return,
                                Ok(_) => {
                                    let replaced = log_reader::replace(line.trim_ascii_end());
                                    if send.blocking_send(factory.create(&replaced)).is_err() {
                                        return;
                                    }
                                    line.clear();
                                    frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                                },
                                Err(e) => {
                                    let error = format!("Error while reading file: {e}");
                                    for line in error.split('\n') {
                                        let replaced = log_reader::replace(line.trim_ascii_end());
                                        if send.blocking_send(factory.create(&replaced)).is_err() {
                                            return;
                                        }
                                    }
                                    frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                                    return;
                                },
                            }
                        }
                    });
                    return;
                }

                let initial_data: Vec<u8> = buffer.into();
                let file = reader.into_inner();
                let mut reader = tokio::io::BufReader::new(tokio::fs::File::from_std(file));

                tokio::task::spawn(async move {
                    let mut factory = ArcStrFactory::default();
                    let mut remaining = initial_data.as_slice();
                    while let Some(index) = memchr::memchr(b'\n', remaining) {
                        let line = &remaining[..index+1];
                        remaining = &remaining[index+1..];

                        if send_log_line(&line, &send, &mut factory).await.is_err() {
                            return;
                        }
                        frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                    }

                    let remaining = remaining.trim_ascii_end();
                    let remaining_len = remaining.len();
                    let mut line = initial_data;
                    if remaining_len == 0 {
                        line.clear();
                    } else {
                        let from = line.len() - remaining_len;
                        line.copy_within(from.., 0);
                        line.truncate(remaining_len);
                    };

                    loop {
                        tokio::select! {
                            _ = send.closed() => {
                                return;
                            },
                            read = reader.read_until('\n' as u8, &mut line) => match read {
                                Ok(0) => {
                                    // EOF reached. If this file is being actively written to (e.g. latest.log),
                                    // then there could be more data
                                    tokio::time::sleep(Duration::from_millis(250)).await;
                                },
                                Ok(_) => {
                                    if line.last() != Some(&b'\n') {
                                        // Didn't read the full line, wait a bit and try again
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        continue;
                                    }

                                    if send_log_line(&line, &send, &mut factory).await.is_err() {
                                        return;
                                    }

                                    frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                                    line.clear();
                                },
                                Err(e) => {
                                    let error = format!("Error while reading file: {e}");
                                    for line in error.split('\n') {
                                        let replaced = log_reader::replace(line.trim_ascii_end());
                                        if send.send(factory.create(&replaced)).await.is_err() {
                                            return;
                                        }
                                    }
                                    frontend.send_with_serial(MessageToFrontend::Refresh, &serial);
                                    return;
                                },
                            }
                        }
                    }
                });
            },
            MessageToBackend::GetLogFiles { instance: id, channel } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let logs = instance.dot_minecraft_path.join("logs");

                    if let Ok(read_dir) = std::fs::read_dir(logs) {
                        let mut paths_with_time = Vec::new();
                        let mut total_gzipped_size = 0;

                        for file in read_dir {
                            let Ok(entry) = file else {
                                continue;
                            };
                            let Ok(metadata) = entry.metadata() else {
                                continue;
                            };
                            let filename = entry.file_name();
                            let Some(filename) = filename.to_str() else {
                                continue;
                            };

                            if filename.ends_with(".log.gz") {
                                total_gzipped_size += metadata.len();
                            } else if !filename.ends_with(".log") {
                                continue;
                            }

                            let created = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);
                            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

                            paths_with_time.push((Arc::from(entry.path()), created.max(modified)));
                        }

                        paths_with_time.sort_by_key(|(_, t)| *t);
                        let paths = paths_with_time.into_iter().map(|(p, _)| p).rev().collect();

                        let _ = channel.send(LogFiles { paths, total_gzipped_size: total_gzipped_size.min(usize::MAX as u64) as usize });
                    }
                }
            },
            MessageToBackend::GetImportFromOtherLauncherJob { channel, launcher, path } => {
                let result = crate::launcher_import::get_import_from_other_launcher_job(launcher, path);
                _ = channel.send(result);
            },
            MessageToBackend::GetSyncState { channel } => {
                let result = crate::syncing::get_sync_state(&self.config.write().get().sync_targets, &mut *self.instance_state.write(), &self.directories);

                match result {
                    Ok(state) => {
                        _ = channel.send(state);
                    },
                    Err(error) => {
                        self.send.send_error(format!("Error while getting sync state: {error}"));
                    },
                }
            },
            MessageToBackend::SetSyncing { target, is_file, value } => {
                let mut write = self.config.write();

                let result = if value {
                    crate::syncing::enable_all(&target, is_file, &mut *self.instance_state.write(), &self.directories)
                } else {
                    crate::syncing::disable_all(&target, is_file, &self.directories).map(|_| true)
                };

                match result {
                    Ok(success) => {
                        if !success {
                            self.send.send_error("Unable to enable syncing");
                            return;
                        }
                    },
                    Err(error) => {
                        self.send.send_error(format!("Error while enabling syncing: {error}"));
                        return;
                    },
                }

                write.modify(|config| {
                    let (set, other_set) = if is_file {
                        (&mut config.sync_targets.files, &mut config.sync_targets.folders)
                    } else {
                        (&mut config.sync_targets.folders, &mut config.sync_targets.files)
                    };

                    other_set.remove(&target);
                    if value {
                        _ = set.insert(target);
                    } else {
                        set.remove(&target);
                    }
                });
            },
            MessageToBackend::GetBackendConfiguration { channel } => {
                let configuration = self.config.write().get().clone();
                let proxy_password = if configuration.proxy.enabled && configuration.proxy.auth_enabled {
                    match PlatformSecretStorage::new().await {
                        Ok(storage) => match storage.read_proxy_password().await {
                            Ok(password) => password,
                            Err(e) => {
                                log::warn!("Failed to read proxy password from keyring: {:?}", e);
                                None
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to create secret storage: {:?}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                _ = channel.send(BackendConfigWithPassword {
                    config: configuration,
                    proxy_password,
                });
            },
            MessageToBackend::CleanupOldLogFiles { instance: id } => {
                let mut deleted = 0;

                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let logs = instance.dot_minecraft_path.join("logs");

                    if let Ok(read_dir) = std::fs::read_dir(logs) {
                        for file in read_dir {
                            let Ok(entry) = file else {
                                continue;
                            };

                            let filename = entry.file_name();
                            let Some(filename) = filename.to_str() else {
                                continue;
                            };

                            if filename.ends_with(".log.gz") {
                                if std::fs::remove_file(entry.path()).is_ok() {
                                    deleted += 1;
                                }
                            }
                        }
                    }
                }

                self.send.send_success(format!("Deleted {} files", deleted));
            },
            MessageToBackend::UploadLogFile { path, modal_action } => {
                let file = match std::fs::File::open(path) {
                    Ok(file) => file,
                    Err(e) => {
                        let error = format!("Unable to read file: {e}");
                        modal_action.set_error_message(log_reader::replace(&error).into());
                        modal_action.set_finished();
                        return;
                    },
                };

                let tracker = ProgressTracker::new("Reading log file".into(), self.send.clone());
                tracker.set_total(4);
                tracker.notify();
                modal_action.trackers.push(tracker.clone());

                let mut reader = std::io::BufReader::new(file);
                let Ok(buffer) = reader.fill_buf() else {
                    tracker.set_finished(ProgressTrackerFinishType::Error);
                    tracker.notify();
                    return;
                };

                let mut content = String::new();

                if buffer.len() >= 2 && buffer[0] == 0x1F && buffer[1] == 0x8B {
                    let mut gz_decoder = flate2::bufread::GzDecoder::new(reader);
                    if let Err(e) = gz_decoder.read_to_string(&mut content) {
                        let error = format!("Error while reading file: {e}");
                        modal_action.set_error_message(log_reader::replace(&error).into());
                        modal_action.set_finished();
                        return;
                    }
                } else {
                    if let Err(e) = reader.read_to_string(&mut content) {
                        let error = format!("Error while reading file: {e}");
                        modal_action.set_error_message(log_reader::replace(&error).into());
                        modal_action.set_finished();
                        return;
                    }
                }

                tracker.set_title("Redacting sensitive information".into());
                tracker.set_count(1);
                tracker.notify();

                // Truncate to 11mb, mclo.gs limit as of right now is ~10.5mb
                if content.len() > 11000000 {
                    for i in 0..4 {
                        if content.is_char_boundary(11000000 - i) {
                            content.truncate(11000000 - i);
                            break;
                        }
                    }
                }

                let replaced = log_reader::replace(&*content);

                tracker.set_title("Uploading to mclo.gs".into());
                tracker.set_count(2);
                tracker.notify();

                if replaced.trim_ascii().is_empty() {
                    modal_action.set_error_message("Log file was empty, didn't upload".into());
                    modal_action.set_finished();
                    return;
                }

                let result = self.http_client.post("https://api.mclo.gs/1/log").form(&[("content", &*replaced)]).send().await;

                let resp = match result {
                    Ok(resp) => resp,
                    Err(e) => {
                        let error = format!("Error while uploading log: {e:?}");
                        modal_action.set_error_message(error.into());
                        modal_action.set_finished();
                        return;
                    },
                };

                tracker.set_count(3);
                tracker.notify();

                let bytes = match resp.bytes().await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        let error = format!("Error while reading mclo.gs response: {e:?}");
                        modal_action.set_error_message(error.into());
                        modal_action.set_finished();
                        return;
                    },
                };

                #[derive(Deserialize)]
                struct McLogsResponse {
                    success: bool,
                    url: Option<String>,
                    error: Option<String>,
                }

                let response: McLogsResponse = match serde_json::from_slice(&bytes) {
                    Ok(response) => response,
                    Err(e) => {
                        let error = format!("Error while deserializing mclo.gs response: {e:?}");
                        modal_action.set_error_message(error.into());
                        modal_action.set_finished();
                        return;
                    },
                };

                if response.success {
                    if let Some(url) = response.url {
                        modal_action.set_visit_url(ModalActionVisitUrl {
                            message: format!("Open {}", url).into(),
                            url: url.into(),
                            prevent_auto_finish: true,
                        });
                        modal_action.set_finished();
                    } else {
                        modal_action.set_error_message("Success returned, but missing url".into());
                        modal_action.set_finished();
                    }
                } else {
                    if let Some(e) = response.error {
                        let error = format!("mclo.gs rejected upload: {e}");
                        modal_action.set_error_message(error.into());
                        modal_action.set_finished();
                    } else {
                        modal_action.set_error_message("Failure returned, but missing error".into());
                        modal_action.set_finished();
                    }
                }

                tracker.set_count(4);
                tracker.set_finished(ProgressTrackerFinishType::Normal);
                tracker.notify();
            },
            MessageToBackend::AddNewAccount { modal_action } => {
                self.login_flow(&modal_action, None).await;
                modal_action.set_finished();
            },
            MessageToBackend::AddOfflineAccount { name, uuid } => {
                let mut account_info = self.account_info.write();
                account_info.modify(|account_info| {
                    account_info.accounts.insert(uuid, BackendAccount {
                        username: name,
                        offline: true,
                        head: None
                    });
                    account_info.selected_account = Some(uuid);
                });
            },
            MessageToBackend::SelectAccount { uuid } => {
                let mut account_info = self.account_info.write();

                let info = account_info.get();
                if info.selected_account == Some(uuid) || !info.accounts.contains_key(&uuid) {
                    return;
                }

                account_info.modify(|account_info| {
                    account_info.selected_account = Some(uuid);
                });
            },
            MessageToBackend::DeleteAccount { uuid } => {
                let mut account_info = self.account_info.write();

                account_info.modify(|account_info| {
                    account_info.accounts.remove(&uuid);
                    if account_info.selected_account == Some(uuid) {
                        account_info.selected_account = None;
                    }
                });
            },
            MessageToBackend::SetOpenGameOutputAfterLaunching { value } => {
                self.config.write().modify(|config| {
                    config.dont_open_game_output_when_launching = !value;
                });
            },
            MessageToBackend::SetProxyConfiguration { config, password } => {
                self.config.write().modify(|backend_config| {
                    backend_config.proxy = config;
                });

                // system keyring (store or delete)
                if let Some(password) = password {
                    match self.secret_storage.get_or_init(PlatformSecretStorage::new).await {
                        Ok(storage) => {
                            if password.is_empty() {
                                if let Err(e) = storage.delete_proxy_password().await {
                                    log::warn!("Failed to delete proxy password from keyring: {:?}", e);
                                }
                            } else if let Err(e) = storage.write_proxy_password(&password).await {
                                log::warn!("Failed to write proxy password to keyring: {:?}", e);
                                self.send.send_error("Failed to save proxy password to system keyring");
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to initialize secret storage: {:?}", e);
                            self.send.send_error("Failed to access system keyring for proxy password");
                        }
                    }
                }

                // Notify user that restart is required for proxy changes to take effect
                self.send.send_info("Proxy settings saved. Restart the launcher to apply changes.");
            },
            MessageToBackend::CreateInstanceShortcut { id, path } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let Ok(current_exe) = std::env::current_exe() else {
                        return;
                    };

                    let args = &[
                        "--run-instance",
                        instance.name.as_str()
                    ];
                    crate::shortcut::create_shortcut(path, &format!("Launch {}", instance.name), &current_exe, args);
                }
            },
            MessageToBackend::RelocateInstance { id, path } => {
                if let Err(err) = std::fs::remove_dir(&path) && err.kind() != std::io::ErrorKind::NotFound {
                    self.send.send_warning(format!("Cannot relocate instance: {err}"));
                    return;
                }

                let mut is_normal_instance_folder = false;

                if let Ok(path) = path.strip_prefix(&self.directories.instances_dir) && crate::is_single_component_path(path) {
                    is_normal_instance_folder = true;

                    let instance_root = if let Some(instance) = self.instance_state.read().instances.get(id) {
                        instance.root_path.clone()
                    } else {
                        return;
                    };

                    #[cfg(unix)]
                    let is_real_folder = !instance_root.is_symlink();
                    #[cfg(windows)]
                    let is_real_folder = !instance_root.is_symlink() && !junction::exists(&instance_root).unwrap_or(false);

                    if is_real_folder && let Some(name) = path.to_str() {
                        self.rename_instance(id, name).await;
                        return;
                    }
                };

                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    if cfg!(windows) {
                        self.file_watching.write().unwatch_subdirectories_of_instance(id);
                        instance.mark_all_dirty(self, false);
                    }

                    #[cfg(windows)]
                    if let Ok(target) = junction::get_target(&instance.root_path) {
                        if let Err(err) = crate::rename_with_fallback_across_devices(&target, &path) {
                            log::error!("Unable to move instance files from {target:?} to {path:?}: {err:?}");
                            self.send.send_error(format!("Unable to move instance files: {err}"));
                            return;
                        }

                        _ = junction::delete(&instance.root_path);

                        if !is_normal_instance_folder {
                            if let Err(err) = junction::create(&path, &instance.root_path) {
                                log::error!("Error while creating junction to moved instance: {err:?}");
                                self.send.send_error(format!("Error while creating junction to moved instance: {err}"));
                                return;
                            }
                        }
                    };

                    if let Ok(target) = std::fs::read_link(&instance.root_path) {
                        if let Err(err) = crate::rename_with_fallback_across_devices(&target, &path) {
                            log::error!("Unable to move instance files from {target:?} to {path:?}: {err:?}");
                            self.send.send_error(format!("Unable to move instance files: {err}"));
                            return;
                        }

                        _ = std::fs::remove_file(&instance.root_path);

                        if !is_normal_instance_folder {
                            #[cfg(unix)]
                            if let Err(err) = std::os::unix::fs::symlink(&path, &instance.root_path) {
                                log::error!("Error while linking to moved instance: {err:?}");
                                self.send.send_error(format!("Error while linking to moved instance: {err}"));
                                return;
                            }
                            #[cfg(windows)]
                            if let Err(err) = std::os::windows::fs::symlink_dir(&path, &instance.root_path) {
                                log::error!("Error while linking to moved instance: {err:?}");
                                self.send.send_error(format!("Error while linking to moved instance: {err}"));
                                return;
                            }
                            #[cfg(not(any(unix, windows)))]
                            compile_error!("Unsupported platform");
                        }

                        return;
                    }

                    if let Err(err) = crate::rename_with_fallback_across_devices(&instance.root_path, &path) {
                        log::error!("Unable to move instance files: {err:?}");
                        self.send.send_error(format!("Unable to move instance files: {err}"));
                        return;
                    }

                    if !is_normal_instance_folder {
                        #[cfg(unix)]
                        if let Err(err) = std::os::unix::fs::symlink(&path, &instance.root_path) {
                            log::error!("Error while linking to moved instance: {err:?}");
                            self.send.send_error(format!("Error while linking to moved instance: {err}"));
                            return;
                        }
                        #[cfg(windows)]
                        if let Err(err) = junction::create(&path, &instance.root_path) {
                            log::error!("Error while creating junction to moved instance: {err:?}");
                            self.send.send_error(format!("Error while creating junction to moved instance: {err}"));
                            return;
                        }
                        #[cfg(not(any(unix, windows)))]
                        compile_error!("Unsupported platform");
                    }

                }
            },
            MessageToBackend::InstallUpdate { update, modal_action } => {
                tokio::task::spawn(crate::update::install_update(self.redirecting_http_client.clone(), self.directories.clone(), self.send.clone(), update, modal_action));
            },
            MessageToBackend::ImportFromOtherLauncher { launcher, import_job, modal_action } => {
                crate::launcher_import::import_from_other_launcher(self, launcher, import_job, modal_action).await;
            },
            MessageToBackend::GetAccountSkin { account, result } => {
                let backend = self.clone();
                tokio::task::spawn(async move {
                    let Some(account) = backend.get_minecraft_profile(account).await else {
                        _ = result.send(AccountSkinResult::NeedsLogin);
                        return;
                    };

                    if let Some(skin) = account.active_skin() {
                        SkinManager::frontend_request(&backend, skin.url.clone(), skin.variant, result);
                    } else {
                        _ = result.send(AccountSkinResult::Success { skin: None, variant: SkinVariant::Classic });
                    }
                });
            },
            MessageToBackend::SetAccountSkin { account, skin, variant } => {
                let Some((_, access_token)) = self.noninteractive_login_flow(account).await else {
                    self.send.send_error("Unable to get access token");
                    return;
                };

                let variant_str = match variant {
                    SkinVariant::Slim => "slim",
                    _ => "classic",
                };

                let form = reqwest::multipart::Form::new()
                    .text("variant", variant_str)
                    .part("file", reqwest::multipart::Part::bytes(skin.to_vec())
                        .file_name("file.png")
                        .mime_str("image/png").unwrap());

                let response = self.http_client
                    .post("https://api.minecraftservices.com/minecraft/profile/skins")
                    .multipart(form)
                    .bearer_auth(access_token.secret())
                    .send()
                    .await;

                let response = match response {
                    Ok(response) => response,
                    Err(err) => {
                        log::error!("Error while making skin change request: {:?}", err);
                        self.send.send_error("Error while making skin change request");
                        return;
                    },
                };

                let status = response.status();
                if status != reqwest::StatusCode::OK {
                    #[derive(Deserialize)]
                    struct MojangApiResponse {
                        #[serde(rename = "errorMessage")]
                        error_message: String
                    }
                    if let Ok(response) = response.json::<MojangApiResponse>().await {
                        log::error!("Skin change failed: {}", &response.error_message);
                        self.send.send_error(format!("Skin change failed: {}", &response.error_message));
                    } else {
                        log::error!("Skin change failed with non-200 status code: {}", status);
                        self.send.send_error(format!("Skin change failed with non-200 status code: {}", status));
                    }
                    return;
                } else if let Ok(profile) = response.json().await {
                    self.cached_minecraft_profiles.write().insert(account, CachedMinecraftProfile::new(profile));
                }
            },
            MessageToBackend::GetAccountCapes { account, result } => {
                let backend = self.clone();
                tokio::task::spawn(async move {
                    let Some(account) = backend.get_minecraft_profile(account).await else {
                        _ = result.send(AccountCapesResult::NeedsLogin);
                        return;
                    };

                    _ = result.send(AccountCapesResult::Success {
                        capes: account.capes
                    });
                });
            },
            MessageToBackend::SetAccountCape { account, cape } => {
                let Some((_, access_token)) = self.noninteractive_login_flow(account).await else {
                    self.send.send_error("Unable to get access token");
                    return;
                };

                let request = if let Some(cape) = cape {
                    #[derive(Serialize)]
                    struct PutActiveCape {
                        #[serde(rename = "capeId")]
                        cape_id: Uuid
                    }

                    self.http_client.put("https://api.minecraftservices.com/minecraft/profile/capes/active").json(&PutActiveCape {
                        cape_id: cape
                    })
                } else {
                    self.http_client.delete("https://api.minecraftservices.com/minecraft/profile/capes/active")
                };

                let response = request
                    .bearer_auth(access_token.secret())
                    .send()
                    .await;

                let response = match response {
                    Ok(response) => response,
                    Err(err) => {
                        log::error!("Error while making cape change request: {:?}", err);
                        self.send.send_error("Error while making cape change request");
                        return;
                    },
                };

                let status = response.status();
                if status != reqwest::StatusCode::OK {
                    #[derive(Deserialize)]
                    struct MojangApiResponse {
                        #[serde(rename = "errorMessage")]
                        error_message: String
                    }
                    if let Ok(response) = response.json::<MojangApiResponse>().await {
                        log::error!("Cape change failed: {}", &response.error_message);
                        self.send.send_error(format!("Cape change failed: {}", &response.error_message));
                    } else {
                        log::error!("Cape change failed with non-200 status code: {}", status);
                        self.send.send_error(format!("Cape change failed with non-200 status code: {}", status));
                    }
                    return;
                } else if let Ok(profile) = response.json().await {
                    self.cached_minecraft_profiles.write().insert(account, CachedMinecraftProfile::new(profile));
                }
            },
            MessageToBackend::RequestSkinLibrary => {
                SkinManager::load_skin_library(&self);
            },
            MessageToBackend::RemoveFromSkinLibrary { skin } => {
                SkinManager::remove_skin(&self, skin);
            },
            MessageToBackend::AddToSkinLibrary { source } => {
                let (bytes, filename) = match source {
                    bridge::message::UrlOrFile::Url { url } => {
                        let url = match url::Url::parse(&*url) {
                            Ok(url) => url,
                            Err(err) => {
                                log::error!("Invalid URL: {}", err);
                                self.send.send_error(format!("Invalid URL: {}", err));
                                return;
                            },
                        };

                        let filename = url.path_segments()
                            .and_then(|s| s.last())
                            .unwrap_or("skin.png")
                            .to_owned();

                        let response = self.redirecting_http_client.get(url).send().await;

                        let response = match response {
                            Ok(response) => response,
                            Err(err) => {
                                log::error!("Error while requesting skin: {:?}", err);
                                self.send.send_error("Error while requesting skin, see logs for more details");
                                return;
                            },
                        };

                        let bytes = match response.bytes().await {
                            Ok(bytes) => bytes.to_vec(),
                            Err(err) => {
                                log::error!("Error while downloading skin: {:?}", err);
                                self.send.send_error("Error while downloading skin, see logs for more details");
                                return;
                            },
                        };

                        (bytes, filename)
                    },
                    bridge::message::UrlOrFile::File { path } => {
                        let bytes = match std::fs::read(&path) {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                log::error!("Error while reading skin file: {:?}", err);
                                self.send.send_error("Error while reading skin file, see logs for more details");
                                return;
                            },
                        };

                        let filename = path.file_name()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or(Cow::Borrowed("skin.png"))
                            .into_owned();

                        (bytes, filename)
                    },
                };

                let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png);
                let image = match image {
                    Ok(image) => image,
                    Err(err) => {
                        if let image::ImageError::Decoding(_) = err {
                            self.send.send_error("Skin is not a valid PNG image");
                        } else {
                            log::error!("An error occurred while loading the image: {:?}", err);
                            self.send.send_error("An error occurred while loading the image, see logs for more details");
                        }
                        return;
                    },
                };
                if !SkinManager::is_valid_size(&image) {
                    self.send.send_error("Invalid skin file. Must be 64x64 or 64x32.");
                    return;
                }

                let filename = sanitize_filename::sanitize_with_options(filename, sanitize_filename::Options { windows: true, ..Default::default() });

                let mut path = self.directories.skin_library_dir.join(&filename);

                if path.exists() {
                    for i in 1..32 {
                        let new_filename = format!("{filename} ({i})");
                        let new_path = self.directories.skin_library_dir.join(&new_filename);
                        if !new_path.exists() {
                            path = new_path;
                            break;
                        }
                    }
                }

                if let Err(err) = crate::write_safe(&path, &bytes) {
                    log::error!("Error while saving skin: {:?}", err);
                    self.send.send_error("Error while saving skin, see logs for more details");
                }
            },
            MessageToBackend::CopyPlayerSkin { username } => {
                let lookup_url = format!(
                    "https://api.mojang.com/minecraft/profile/lookup/name/{}",
                    username
                );
                let response = match self.http_client.get(&lookup_url).send().await {
                    Ok(r) => r,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to request Mojang API: {:?}", err);
                        self.send.send_error("Failed to request Mojang API");
                        return;
                    }
                };
                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    self.send.send_error(format!("Player '{}' not found", username));
                    return;
                }
                if !response.status().is_success() {
                    log::error!("CopyPlayerSkin: Mojang API returned status {}", response.status());
                    self.send.send_error(format!("Failed to request Mojang API: status {}", response.status()));
                    return;
                }
                let body = match response.text().await {
                    Ok(b) => b,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to read Mojang API response: {:?}", err);
                        self.send.send_error("Failed to read Mojang API response");
                        return;
                    }
                };
                let profile_lookup: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(v) => v,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to deserialize Mojang API response: {:?}", err);
                        self.send.send_error("Failed to deserialize Mojang API response");
                        return;
                    }
                };
                let uuid = match profile_lookup["id"].as_str() {
                    Some(id) => id.to_owned(),
                    None => {
                        log::error!("CopyPlayerSkin: missing 'id' field in Mojang API response");
                        self.send.send_error("Failed to deserialize Mojang API response");
                        return;
                    }
                };

                let session_url = format!(
                    "https://sessionserver.mojang.com/session/minecraft/profile/{}",
                    uuid
                );
                let response = match self.http_client.get(&session_url).send().await {
                    Ok(r) => r,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to request session server: {:?}", err);
                        self.send.send_error("Failed to request Mojang session server");
                        return;
                    }
                };
                if !response.status().is_success() {
                    log::error!("CopyPlayerSkin: session server returned status {}", response.status());
                    self.send.send_error(format!("Failed to request Mojang session server: status {}", response.status()));
                    return;
                }
                let body = match response.text().await {
                    Ok(b) => b,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to read session server response: {:?}", err);
                        self.send.send_error("Failed to read Mojang session server response");
                        return;
                    }
                };

                let skin_url = match Self::extract_skin_url_from_profile(&body) {
                    Some(url) => url,
                    None => {
                        self.send.send_error(format!("Player '{}' has no skin", username));
                        return;
                    }
                };

                let url = match url::Url::parse(&*skin_url) {
                    Ok(url) => url,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to parse skin URL: {}", err);
                        self.send.send_error("Failed to parse skin URL");
                        return;
                    }
                };

                let filename = format!("{}.png", username);

                let response = match self.redirecting_http_client.get(url).send().await {
                    Ok(r) => r,
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to request skin texture: {:?}", err);
                        self.send.send_error("Error while requesting skin, see logs for more details");
                        return;
                    }
                };
                if !response.status().is_success() {
                    log::error!("CopyPlayerSkin: skin texture request returned status {}", response.status());
                    self.send.send_error(format!("Failed to request skin texture: status {}", response.status()));
                    return;
                }
                let bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(err) => {
                        log::error!("CopyPlayerSkin: failed to read skin texture: {:?}", err);
                        self.send.send_error("Error while downloading skin, see logs for more details");
                        return;
                    }
                };

                let image = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
                    Ok(image) => image,
                    Err(_) => {
                        self.send.send_error("Player skin is not a valid PNG image");
                        return;
                    }
                };
                if !SkinManager::is_valid_size(&image) {
                    self.send.send_error("Player skin has invalid dimensions. Must be 64x64 or 64x32.");
                    return;
                }

                let filename = sanitize_filename::sanitize_with_options(filename, sanitize_filename::Options { windows: true, ..Default::default() });

                let mut path = self.directories.skin_library_dir.join(&filename);
                if path.exists() {
                    for i in 1..32 {
                        let new_filename = format!("{filename} ({i})");
                        let new_path = self.directories.skin_library_dir.join(&new_filename);
                        if !new_path.exists() {
                            path = new_path;
                            break;
                        }
                    }
                }

                if let Err(err) = crate::write_safe(&path, &bytes) {
                    log::error!("CopyPlayerSkin: failed to save skin: {:?}", err);
                    self.send.send_error("Error while saving skin, see logs for more details");
                }
            },
            MessageToBackend::Login { account, modal_action } => {
                self.login_flow(&modal_action, Some(account)).await;
                modal_action.set_finished();
            },
            MessageToBackend::Quit => {
                self.should_quit.store(true, Ordering::Relaxed);
            },
        }
    }

    async fn start_instance(self: &Arc<Self>, id: InstanceID, quick_play: Option<QuickPlayLaunch>, modal_action: ModalAction) {
        let keepalive = KeepAlive::new();

        let (dot_minecraft, configuration) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            if let Some(launch_keepalive) = &instance.launch_keepalive && launch_keepalive.is_alive() {
                modal_action.set_error_message("Can't launch instance, already launching".into());
                modal_action.set_finished();
                return;
            }

            instance.launch_keepalive = Some(keepalive.create_handle());

            self.send.send(MessageToFrontend::MoveInstanceToTop {
                id
            });
            self.send.send(instance.create_modify_message());

            (instance.dot_minecraft_path.clone(), instance.configuration.get().clone())
        } else {
            self.send.send_error("Can't launch instance, unknown id");
            modal_action.set_error_message("Can't launch instance, unknown id".into());
            modal_action.set_finished();
            return;
        };

        scopeguard::defer! {
            modal_action.set_finished();
            drop(keepalive);
            if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                if let Some(launch_keepalive) = &instance.launch_keepalive && !launch_keepalive.is_alive() {
                    instance.launch_keepalive = None;
                }
                self.restore_mods_folder_if_stopped(instance);
                self.send.send(instance.create_modify_message());
            }
        }

        let Some(login_info) = self.get_login_info(&modal_action, configuration.preferred_account).await else {
            modal_action.set_error_message("Unable to log in to Minecraft account".into());
            return;
        };

        let prelaunch_result = tokio::select! {
            result = self.prelaunch(id, &modal_action) => result,
            _ = modal_action.request_cancel.cancelled() => {
                self.send.send(MessageToFrontend::CloseModal);
                return;
            }
        };

        if modal_action.error.read().is_some() {
            self.send.send(MessageToFrontend::Refresh);
            return;
        }
        if let Err(err) = prelaunch_result {
            modal_action.set_error_message(format!("Unable to run prelaunch: {}", err).into());
            log::error!("Unable to run prelaunch: {err:?}");
            self.send.send(MessageToFrontend::Refresh);
            return;
        }

        let game_output = !self.config.write().get().dont_open_game_output_when_launching;

        let launch_tracker = ProgressTracker::new(Arc::from("Launching"), self.send.clone());
        modal_action.trackers.push(launch_tracker.clone());
        let result = self.launcher.launch(&self.redirecting_http_client, dot_minecraft, configuration, quick_play, login_info, game_output, &launch_tracker, &modal_action).await;

        if matches!(result, Err(LaunchError::CancelledByUser)) {
            self.send.send(MessageToFrontend::CloseModal);
            return;
        }

        let is_err = result.is_err();
        match result {
            Ok(mut child) => {
                if game_output {
                    if let Some(stdout) = child.stdout.take() {
                        log_reader::start_game_output(stdout, child.stderr.take(), self.send.clone());
                    }
                }

                // Close handles if unused
                child.stderr.take();
                child.stdin.take();
                child.stdout.take();

                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.processes.push(child.process);
                    instance.update_session();
                    self.quit_coordinator.set_can_quit(false);
                }
            },
            Err(ref err) => {
                log::error!("Failed to launch due to error: {:?}", &err);
                modal_action.set_error_message(format!("{}", &err).into());
            },
        }

        launch_tracker.set_finished(ProgressTrackerFinishType::from_err(is_err));
        launch_tracker.notify();
    }

    fn extract_skin_url_from_profile(profile_json: &str) -> Option<Arc<str>> {
        use base64::Engine;
        let parsed: serde_json::Value = serde_json::from_str(profile_json).ok()?;
        for prop in parsed["properties"].as_array()? {
            if prop["name"].as_str() == Some("textures") {
                let encoded = prop["value"].as_str()?;
                let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
                let textures: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
                let url = textures["textures"]["SKIN"]["url"].as_str()?;
                return Some(url.into());
            }
        }
        None
    }

    pub async fn get_minecraft_profile(&self, account: Uuid) -> Option<MinecraftProfileResponse> {
        if let Some(cached_profile) = self.cached_minecraft_profiles.read().get(&account) {
            if cached_profile.is_valid(Instant::now()) {
                return Some(cached_profile.profile.clone());
            }
        }

        let try_permit = self.login_semaphore.try_acquire();
        let mut _await_permit = None;
        if matches!(try_permit, Err(TryAcquireError::NoPermits)) {
            _await_permit = Some(self.login_semaphore.acquire().await);

            if let Some(cached_profile) = self.cached_minecraft_profiles.read().get(&account) {
                if cached_profile.is_valid(Instant::now()) {
                    return Some(cached_profile.profile.clone());
                }
            }
        }

        let secret_storage = self.get_secret_storage(None).await?;
        let credentials = secret_storage.read_credentials(account).await.ok().flatten().unwrap_or_default();

        Some(self.noninteractive_login_flow_inner(account, credentials).await?.0)
    }

    pub async fn noninteractive_login_flow(&self, account: Uuid) -> Option<(MinecraftProfileResponse, MinecraftAccessToken)> {
        let _permit = self.login_semaphore.acquire().await;

        let secret_storage = self.get_secret_storage(None).await?;
        let credentials = secret_storage.read_credentials(account).await.ok().flatten().unwrap_or_default();

        if let Some(access_token) = credentials.access_token()
            && let Some(cached_profile) = self.cached_minecraft_profiles.read().get(&account)
            && cached_profile.is_valid(Instant::now())
        {
            return Some((cached_profile.profile.clone(), access_token));
        }

        self.noninteractive_login_flow_inner(account, credentials).await
    }

    pub async fn noninteractive_login_flow_inner(&self, account: Uuid, mut credentials: AccountCredentials) -> Option<(MinecraftProfileResponse, MinecraftAccessToken)> {
        log::info!("Doing non-interactive login flow for {account}");
        let login_result = self.login(&mut credentials, None, None).await;

        if let Err(LoginError::NeedsUserInteraction) | Err(LoginError::CancelledByUser) = login_result {
            return None;
        }

        let secret_storage = self.get_secret_storage(None).await?;

        let (profile, access_token) = match login_result {
            Ok(login_result) => login_result,
            Err(ref err) => {
                log::error!("Error logging in: {err}");
                let _ = secret_storage.delete_credentials(account).await;
                return None;
            },
        };

        self.cached_minecraft_profiles.write().insert(profile.id, CachedMinecraftProfile::new(profile.clone()));

        if profile.id != account {
            let _ = secret_storage.delete_credentials(account).await;
        }

        self.update_account_info_with_profile(&profile, false);

        if let Err(error) = secret_storage.write_credentials(profile.id, &credentials).await {
            log::warn!("Unable to write credentials to keychain: {error}");
        }

        Some((profile, access_token))
    }

    pub async fn get_secret_storage(&self, modal_action: Option<&ModalAction>) -> Option<&PlatformSecretStorage> {
        match self.secret_storage.get_or_init(PlatformSecretStorage::new).await {
            Ok(secret_storage) => Some(secret_storage),
            Err(error) => {
                log::error!("Error initializing secret storage: {error}");
                if let Some(modal_action) = modal_action {
                    modal_action.set_error_message(format!("Error initializing secret storage: {error}").into());
                    modal_action.set_finished();
                }
                return None;
            }
        }
    }

    pub async fn login_flow(&self, modal_action: &ModalAction, selected_account: Option<Uuid>) -> Option<(MinecraftProfileResponse, MinecraftAccessToken)> {
        let _permit = self.login_semaphore.acquire().await;

        let mut credentials = if let Some(selected_account) = selected_account {
            let secret_storage = self.get_secret_storage(Some(modal_action)).await?;

            match secret_storage.read_credentials(selected_account).await {
                Ok(credentials) => credentials.unwrap_or_default(),
                Err(error) => {
                    log::warn!("Unable to read credentials from keychain: {error}");
                    self.send.send_warning(
                        "Unable to read credentials from keychain. You will need to log in again",
                    );
                    AccountCredentials::default()
                },
            }
        } else {
            AccountCredentials::default()
        };

        if let Some(selected_account) = selected_account
            && let Some(access_token) = credentials.access_token()
            && let Some(cached_profile) = self.cached_minecraft_profiles.read().get(&selected_account)
        {
            let now = Instant::now();
            if now >= cached_profile.not_before && now < cached_profile.not_after {
                return Some((cached_profile.profile.clone(), access_token));
            }
        }

        let login_tracker = ProgressTracker::new(Arc::from("Logging in"), self.send.clone());
        modal_action.trackers.push(login_tracker.clone());

        let login_result = self.login(&mut credentials, Some(&login_tracker), Some(&modal_action)).await;

        if matches!(login_result, Err(LoginError::CancelledByUser)) {
            self.send.send(MessageToFrontend::CloseModal);
            return None;
        }

        let secret_storage = self.get_secret_storage(Some(modal_action)).await?;

        let (profile, access_token) = match login_result {
            Ok(login_result) => {
                login_tracker.set_finished(ProgressTrackerFinishType::Normal);
                login_tracker.notify();
                login_result
            },
            Err(ref err) => {
                log::error!("Error logging in: {err}");

                if let Some(selected_account) = selected_account {
                    let _ = secret_storage.delete_credentials(selected_account).await;
                }

                modal_action.set_error_message(format!("Error logging in: {}", &err).into());
                login_tracker.set_finished(ProgressTrackerFinishType::Error);
                login_tracker.notify();
                modal_action.set_finished();
                return None;
            },
        };

        self.cached_minecraft_profiles.write().insert(profile.id, CachedMinecraftProfile::new(profile.clone()));

        if let Some(selected_account) = selected_account
            && profile.id != selected_account
        {
            let _ = secret_storage.delete_credentials(selected_account).await;
        }

        self.update_account_info_with_profile(&profile, true);

        if let Err(error) = secret_storage.write_credentials(profile.id, &credentials).await {
            log::warn!("Unable to write credentials to keychain: {error}");
            self.send.send_warning("Unable to write credentials to keychain. You might need to fully log in again next time");
        }

        Some((profile, access_token))
    }

    pub fn update_account_info_with_profile(&self, profile: &MinecraftProfileResponse, select: bool) {
        let mut account_info = self.account_info.write();

        let info = account_info.get();
        if info.accounts.contains_key(&profile.id) && (!select || info.selected_account == Some(profile.id)) {
            drop(account_info);
            if let Some(skin) = profile.active_skin().cloned() {
                SkinManager::update_account(self, profile.id, skin.url);
            }
            return;
        }

        account_info.modify(|info| {
            if !info.accounts.contains_key(&profile.id) {
                let account = BackendAccount::new_from_profile(profile);
                info.accounts.insert(profile.id, account);
            }

            if select {
                info.selected_account = Some(profile.id);
            }
        });

        drop(account_info);
        if let Some(skin) = profile.active_skin().cloned() {
            SkinManager::update_account(self, profile.id, skin.url);
        }
    }

    pub async fn download_all_metadata(&self) {
        let Ok(versions) = self.meta.fetch(&MinecraftVersionManifestMetadataItem).await else {
            panic!("Unable to get Minecraft version manifest");
        };

        for link in &versions.versions {
            let Ok(version_info) = self.meta.fetch(&MinecraftVersionMetadataItem(link)).await else {
                panic!("Unable to get load version: {:?}", link.id);
            };

            let asset_index = format!("{}", version_info.assets);

            let Ok(_) = self.meta.fetch(&AssetsIndexMetadataItem {
                url: version_info.asset_index.url,
                cache: self.directories.assets_index_dir.join(format!("{}.json", &asset_index)).into(),
                hash: version_info.asset_index.sha1,
            }).await else {
                panic!("Can't get assets index {:?}", version_info.asset_index.url);
            };

            if let Some(arguments) = &version_info.arguments {
                for argument in arguments.game.iter() {
                    let value = match argument {
                        LaunchArgument::Single(launch_argument_value) => launch_argument_value,
                        LaunchArgument::Ruled(launch_argument_ruled) => &launch_argument_ruled.value,
                    };
                    match value {
                        LaunchArgumentValue::Single(shared_string) => {
                            check_argument_expansions(shared_string.as_str());
                        },
                        LaunchArgumentValue::Multiple(shared_strings) => {
                            for shared_string in shared_strings.iter() {
                                check_argument_expansions(shared_string.as_str());
                            }
                        },
                    }
                }
            } else if let Some(legacy_arguments) = &version_info.minecraft_arguments {
                for argument in legacy_arguments.split_ascii_whitespace() {
                    check_argument_expansions(argument);
                }
            }
        }

        let Ok(runtimes) = self.meta.fetch(&MojangJavaRuntimesMetadataItem).await else {
            panic!("Unable to get java runtimes manifest");
        };

        for (platform_name, platform) in &runtimes.platforms {
            for (jre_component, components) in &platform.components {
                if components.is_empty() {
                    continue;
                }

                let runtime_component_dir = self.directories.runtime_base_dir.join(jre_component).join(platform_name.as_str());
                let _ = std::fs::create_dir_all(&runtime_component_dir);
                let Ok(runtime_component_dir) = runtime_component_dir.canonicalize() else {
                    panic!("Unable to create runtime component dir");
                };

                for runtime_component in components {
                    let Ok(manifest) = self.meta.fetch(&MojangJavaRuntimeComponentMetadataItem {
                        url: runtime_component.manifest.url,
                        cache: runtime_component_dir.join("manifest.json").into(),
                        hash: runtime_component.manifest.sha1,
                    }).await else {
                        panic!("Unable to get java runtime component manifest");
                    };

                    let keys: &[Arc<std::path::Path>] = &[
                        std::path::Path::new("bin/java").into(),
                        std::path::Path::new("bin/javaw.exe").into(),
                        std::path::Path::new("jre.bundle/Contents/Home/bin/java").into(),
                        std::path::Path::new("MinecraftJava.exe").into(),
                    ];

                    let mut known_executable_path = false;
                    for key in keys {
                        if manifest.files.contains_key(key) {
                            known_executable_path = true;
                            break;
                        }
                    }

                    if !known_executable_path {
                        panic!("{}/{} doesn't contain known java executable", jre_component, platform_name);
                    }
                }
            }
        }

        println!("Done downloading all metadata");
    }
}

fn check_argument_expansions(argument: &str) {
    let mut dollar_last = false;
    for (i, character) in argument.char_indices() {
        if character == '$' {
            dollar_last = true;
        } else if dollar_last && character == '{' {
            let remaining = &argument[i..];
            if let Some(end) = remaining.find('}') {
                let to_expand = &argument[i+1..i+end];
                if ArgumentExpansionKey::from_str(to_expand).is_none() {
                    panic!("Unsupported argument: {:?}", to_expand);
                }
            }
        } else {
            dollar_last = false;
        }
    }
}

async fn send_log_line(line: &[u8], send: &tokio::sync::mpsc::Sender<Arc<str>>, factory: &mut ArcStrFactory) -> Result<(), tokio::sync::mpsc::error::SendError<Arc<str>>> {
    match str::from_utf8(&*line) {
        Ok(utf8) => {
            let replaced = log_reader::replace(utf8.trim_ascii_end());
            send.send(factory.create(&replaced)).await?;
        },
        Err(e) => {
            let error = format!("Invalid UTF8: {e}");
            for line in error.split('\n') {
                let replaced = log_reader::replace(line.trim_ascii_end());
                send.send(factory.create(&replaced)).await?;
            }
        },
    }
    Ok(())
}
