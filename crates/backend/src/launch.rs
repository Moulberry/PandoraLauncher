use std::{
    borrow::Cow, cmp::Ordering, collections::{BTreeSet, HashMap, HashSet}, ffi::{OsStr, OsString}, fs::File, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, process::{Child, Stdio}, sync::{Arc, OnceLock, atomic::AtomicBool}
};

use bridge::{
    handle::FrontendHandle, message::{MessageToFrontend, QuickPlayLaunch}, modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType, ProgressTrackers}, safe_path::SafePath
};
use futures::{FutureExt, TryFutureExt};
use rand::seq::SliceRandom;
use rc_zip_sync::{ArchiveHandle, ReadZip};
use regex::Regex;
use rustc_hash::FxHashMap;
use schema::{
    assets_index::AssetsIndex, fabric_launch::FabricLaunch, forge::{ForgeInstallProfile, ForgeInstallProfileLegacy, ForgeSide, VersionFragment}, instance::InstanceConfiguration, java_runtime_component::{JavaRuntimeComponentFile, JavaRuntimeComponentManifest}, loader::Loader, maven::{MavenCoordinate, MavenMetadataXml}, version::{
        GameLibrary, GameLibraryArtifact, GameLibraryDownloads, GameLibraryExtractOptions, GameLogging, LaunchArgument, LaunchArgumentValue, MinecraftVersion, OsArch, OsName, PartialMinecraftVersion, Rule, RuleAction
    }, version_manifest::MinecraftVersionManifest
};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use ustr::Ustr;

use crate::{
    account::MinecraftLoginInfo, directories::LauncherDirectories, launch_wrapper, metadata::{items::{AssetsIndexMetadataItem, FabricLaunchMetadataItem, FabricLoaderManifestMetadataItem, ForgeInstallerMavenMetadataItem, MinecraftVersionManifestMetadataItem, MinecraftVersionMetadataItem, MojangJavaRuntimeComponentMetadataItem, MojangJavaRuntimesMetadataItem, NeoforgeInstallerMavenMetadataItem}, manager::{
        MetaLoadError, MetadataManager,
    }}
};

#[derive(Clone)]
pub struct Launcher {
    meta: Arc<MetadataManager>,
    directories: Arc<LauncherDirectories>,
    launch_wrapper: Arc<Path>,
    sender: FrontendHandle,
    pub global_memory_max: Option<u32>,
}

#[derive(thiserror::Error, Debug)]
pub enum LaunchError {
    #[error("Failed to deserialize data:\n{0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Failed to perform I/O operation:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to load java runtime:\n{0}")]
    LoadJavaRuntimeError(#[from] LoadJavaRuntimeError),
    #[error("Failed to load game assets:\n{0}")]
    LoadAssetObjectsError(#[from] LoadAssetObjectsError),
    #[error("Failed to load game libraries:\n{0}")]
    LoadLibrariesError(#[from] LoadLibrariesError),
    #[error("Failed to load metadata:\n{0}")]
    MetaLoadError(#[from] MetaLoadError),
    #[error("Failed read zip:\n{0}")]
    ZipError(#[from] rc_zip_sync::rc_zip::Error),
    #[error("Missing file in zip: {0}")]
    MissingFileInZipError(Cow<'static, str>),
    #[error("Failed to find version: {0}")]
    CantFindVersion(&'static str),
    #[error("Invalid instance name: {0}")]
    InvalidInstanceName(&'static str),
    #[error("Error running forge post processor")]
    ForgePostProcessorError,
    #[error("Cancelled by user")]
    CancelledByUser,
    #[error("Loader supports the wrong version of Minecraft: {0}")]
    MismatchedLoaderVersions(Arc<str>),
}

#[derive(PartialEq, Eq)]
pub enum AddVanillaJar {
    Yes,
    No,
}

impl Launcher {
    pub fn new(meta: Arc<MetadataManager>, directories: Arc<LauncherDirectories>, sender: FrontendHandle) -> Self {
        let launch_wrapper = launch_wrapper::create_wrapper(&directories.temp_dir).into();
        Self {
            meta,
            directories,
            launch_wrapper,
            sender,
            global_memory_max: None,
        }
    }

    pub async fn launch(
        &self,
        http_client: &reqwest::Client,
        dot_minecraft_path: Arc<Path>,
        instance_info: InstanceConfiguration,
        quick_play: Option<QuickPlayLaunch>,
        login_info: MinecraftLoginInfo,
        add_mods: Vec<PathBuf>,
        launch_tracker: &ProgressTracker,
        modal_action: &ModalAction,
    ) -> Result<Child, LaunchError> {
        log::info!("Launching {:?}", dot_minecraft_path);

        launch_tracker.set_total(6);

        log::debug!("Creating launch version");

        let (version_info, add_vanilla_jar) = tokio::select! {
            result = self.create_launch_version(http_client, &modal_action.trackers, launch_tracker, &instance_info) => result?,
            _ = modal_action.request_cancel.cancelled() => {
                self.sender.send(MessageToFrontend::CloseModal);
                return Err(LaunchError::CancelledByUser);
            }
        };

        launch_tracker.add_count(1);
        launch_tracker.notify();

        let _ = std::fs::create_dir_all(&dot_minecraft_path);

        let launch_rule_context = LaunchRuleContext {
            is_demo_user: false,
            custom_resolution: None,
            quick_play,
        };

        let mut artifacts = Vec::new();
        let mut natives_to_extract = HashMap::new();
        launch_rule_context.collect_libraries(&version_info.libraries, &mut artifacts, &mut natives_to_extract);

        // Compute natives path based on combined hash of all libraries
        let natives_dir = self.directories.temp_natives_base_dir.join(calculate_natives_dirname(&artifacts));
        let _ = std::fs::create_dir_all(&natives_dir);

        if add_vanilla_jar == AddVanillaJar::Yes {
            let client_download = &version_info.downloads.client;
            artifacts.push(GameLibraryArtifact {
                path: format!("net/minecraft/{0}/minecraft-client-{0}.jar", instance_info.minecraft_version).into(),
                sha1: Some(client_download.sha1),
                size: Some(client_download.size),
                url: client_download.url,
            });
        }

        let mojang_java_binary_future = self.load_mojang_java_binary(
            &self.meta,
            http_client,
            &instance_info,
            &version_info,
            &modal_action.trackers,
            launch_tracker,
        );
        let load_assets_future =
            self.load_assets(&self.meta, http_client, &dot_minecraft_path, &version_info, &modal_action.trackers, launch_tracker);
        let load_libraries_future =
            self.load_libraries(http_client, &artifacts, &modal_action.trackers, launch_tracker);
        let load_log_configuration = self.load_log_configuration(http_client, version_info.logging.as_ref());

        log::debug!("Loading java, assets, libraries and log configuration");

        let joined = futures::future::try_join4(
            mojang_java_binary_future.map_err(LaunchError::from),
            load_assets_future.map_err(LaunchError::from),
            load_libraries_future.map_err(LaunchError::from),
            load_log_configuration.map(Ok),
        );

        let (java_path, assets_index_name, library_paths, log_configuration) = tokio::select! {
            result = joined => result?,
            _ = modal_action.request_cancel.cancelled() => {
                self.sender.send(MessageToFrontend::CloseModal);
                return Err(LaunchError::CancelledByUser);
            }
        };

        launch_tracker.add_count(1);
        launch_tracker.notify();

        let mut classpath = Vec::new();
        for (raw_path, library_path) in library_paths {
            if let Some(extract_options) = natives_to_extract.get(&raw_path) {
                let Ok(file) = std::fs::File::open(library_path) else {
                    continue;
                };
                let Ok(archive) = file.read_zip() else {
                    continue;
                };
                for file in archive.entries() {
                    let Some(path) = SafePath::new(&file.name) else {
                        continue;
                    };

                    if let Some(exclude) = &extract_options.exclude {
                        let mut skip = false;
                        for to_exclude in exclude.iter() {
                            if path.starts_with(to_exclude) {
                                skip = true;
                                break;
                            }
                        }
                        if skip {
                            continue;
                        }
                    }

                    let output_path = path.to_path(&natives_dir);
                    match file.kind() {
                        rc_zip_sync::rc_zip::EntryKind::Directory => {
                            let _ = std::fs::create_dir(output_path);
                        },
                        rc_zip_sync::rc_zip::EntryKind::File => {
                            let Ok(mut outfile) = std::fs::File::create(&output_path) else {
                                continue;
                            };
                            let _ = std::io::copy(&mut file.reader(), &mut outfile);
                        },
                        rc_zip_sync::rc_zip::EntryKind::Symlink => {},
                    }
                }
            } else {
                classpath.push(library_path.into_os_string());
            }
        }

        let launch_context = LaunchContext {
            launch_wrapper_path: self.launch_wrapper.clone(),
            java_path,
            natives_dir,
            libraries_dir: self.directories.libraries_dir.clone(),
            game_dir: dot_minecraft_path,
            configuration: instance_info,
            assets_root: self.directories.assets_root_dir.clone(),
            temp_dir: self.directories.temp_dir.clone(),
            assets_index_name,
            classpath,
            log_configuration,
            rule_context: launch_rule_context,
            login_info,
            add_mods,
            global_memory_max: self.global_memory_max,
        };

        if modal_action.has_requested_cancel() {
            self.sender.send(MessageToFrontend::CloseModal);
            return Err(LaunchError::CancelledByUser);
        }

        log::info!("Launching game process");
        let child = launch_context.launch(&version_info)?;

        launch_tracker.add_count(1);

        Ok(child)
    }

    async fn create_launch_version(
        &self,
        http_client: &reqwest::Client,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
        instance_info: &InstanceConfiguration,
    ) -> Result<(Arc<MinecraftVersion>, AddVanillaJar), LaunchError> {
        match instance_info.loader {
            Loader::Vanilla => {
                launch_tracker.add_total(1);
                launch_tracker.notify();

                let versions = self.meta.fetch(&MinecraftVersionManifestMetadataItem).await?;

                launch_tracker.add_count(1);
                launch_tracker.notify();

                let Some(version) = versions.versions.iter().find(|v| v.id == instance_info.minecraft_version) else {
                    return Err(LaunchError::CantFindVersion(instance_info.minecraft_version.as_str()));
                };

                Ok((self.meta.fetch(&MinecraftVersionMetadataItem(version)).await?, AddVanillaJar::Yes))
            },
            Loader::Fabric => {
                let versions = self.meta.fetch(&MinecraftVersionManifestMetadataItem).map_err(LaunchError::from);

                let fabric_loader_version = async move {
                    if let Some(preferred_version) = instance_info.preferred_loader_version {
                        Ok(preferred_version)
                    } else {
                        let manifest = self.meta.fetch(&FabricLoaderManifestMetadataItem).map_err(LaunchError::from).await?;

                        let mut latest_loader_version = manifest.0.iter().find(|v| v.stable);
                        if latest_loader_version.is_none() {
                            latest_loader_version = manifest.0.first();
                        }
                        Ok(latest_loader_version.unwrap().version)
                    }
                };

                launch_tracker.add_total(4);
                launch_tracker.notify();

                let launch_tracker2 = launch_tracker.clone();
                let meta2 = Arc::clone(&self.meta);
                let minecraft_version = instance_info.minecraft_version;
                let fabric_launch = fabric_loader_version.and_then(async move |loader_version| {
                    launch_tracker2.add_count(1);
                    launch_tracker2.notify();

                    let value = meta2.fetch(&FabricLaunchMetadataItem {
                        minecraft_version,
                        loader_version,
                    }).await?;

                    launch_tracker2.add_count(1);
                    launch_tracker2.notify();

                    Ok(value)
                });

                let launch_tracker3 = launch_tracker.clone();
                let meta3 = Arc::clone(&self.meta);
                let instance_version = instance_info.minecraft_version;
                let version = versions.and_then(async move |versions| {
                    launch_tracker3.add_count(1);
                    launch_tracker3.notify();

                    let Some(version) = versions.versions.iter().find(|v| v.id == instance_version) else {
                        return Err(LaunchError::CantFindVersion(instance_version.as_str()));
                    };

                    let value = meta3.fetch(&MinecraftVersionMetadataItem(version)).await?;

                    launch_tracker3.add_count(1);
                    launch_tracker3.notify();

                    Ok(value)
                });

                let (version, fabric_launch): (Arc<MinecraftVersion>, Arc<FabricLaunch>) =
                    futures::future::try_join(version, fabric_launch).await?;

                let mut version: MinecraftVersion = (*version).clone();

                if let Some(loader) = &fabric_launch.loader {
                    let loader_coordinate = MavenCoordinate::create(&loader.maven);
                    let artifact_path = loader_coordinate.artifact_path();
                    version.libraries.push(GameLibrary {
                        downloads: GameLibraryDownloads {
                            artifact: Some(GameLibraryArtifact {
                                url: format!("https://maven.fabricmc.net/{}", &artifact_path).into(),
                                path: artifact_path.into(),
                                sha1: None,
                                size: None,
                            }),
                            classifiers: None,
                        },
                        name: loader.maven,
                        rules: None,
                        natives: None,
                        extract: None,
                    });
                }

                if let Some(intermediary) = &fabric_launch.intermediary {
                    let intermediary_coordinate = MavenCoordinate::create(&intermediary.maven);
                    let artifact_path = intermediary_coordinate.artifact_path();
                    version.libraries.push(GameLibrary {
                        downloads: GameLibraryDownloads {
                            artifact: Some(GameLibraryArtifact {
                                url: format!("https://maven.fabricmc.net/{}", &artifact_path).into(),
                                path: artifact_path.into(),
                                sha1: None,
                                size: None,
                            }),
                            classifiers: None,
                        },
                        name: intermediary.maven,
                        rules: None,
                        natives: None,
                        extract: None,
                    });
                }

                let libraries = &fabric_launch.launcher_meta.libraries;
                for library in libraries.common.iter().chain(libraries.client.iter()) {
                    let library_coordinate = MavenCoordinate::create(&library.name);
                    let artifact_path = library_coordinate.artifact_path();
                    version.libraries.push(GameLibrary {
                        downloads: GameLibraryDownloads {
                            artifact: Some(GameLibraryArtifact {
                                url: format!("{}{}", &library.url, &artifact_path).into(),
                                path: artifact_path.into(),
                                sha1: Some(library.sha1),
                                size: Some(library.size),
                            }),
                            classifiers: None,
                        },
                        name: library.name,
                        rules: None,
                        natives: None,
                        extract: None,
                    });
                }

                version.main_class = fabric_launch.launcher_meta.main_class.client;

                Ok((Arc::new(version), AddVanillaJar::Yes))
            },
            Loader::Forge => {
                launch_tracker.add_total(7);
                launch_tracker.notify();

                // Download Minecraft manifest and neoforge installer maven
                let (minecraft_versions, loader_versions) = futures::future::try_join(
                    self.meta.fetch(&MinecraftVersionManifestMetadataItem),
                    self.meta.fetch(&ForgeInstallerMavenMetadataItem)
                ).await?;

                self.create_forgelike_launch_version(http_client, progress_trackers, launch_tracker, instance_info,
                    minecraft_versions,
                    &loader_versions.0,
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{0}/forge-{0}-installer.jar.sha1",
                    "net/minecraftforge/forge/{0}/forge-{0}-installer.jar",
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{0}/forge-{0}-installer.jar",
                    true,
                    false
                ).await
            },
            Loader::NeoForge => {
                launch_tracker.add_total(7);
                launch_tracker.notify();

                // Download Minecraft manifest and neoforge installer maven
                let (minecraft_versions, loader_versions) = futures::future::try_join(
                    self.meta.fetch(&MinecraftVersionManifestMetadataItem),
                    self.meta.fetch(&NeoforgeInstallerMavenMetadataItem)
                ).await?;

                self.create_forgelike_launch_version(http_client, progress_trackers, launch_tracker, instance_info,
                    minecraft_versions,
                    &loader_versions.0,
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar.sha1",
                    "net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar",
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar",
                    false,
                    true
                ).await
            },
            Loader::Unknown => todo!(),
        }
    }

    async fn create_forgelike_launch_version(
        &self,
        http_client: &reqwest::Client,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
        instance_info: &InstanceConfiguration,
        minecraft_versions: Arc<MinecraftVersionManifest>,
        loader_versions: &[Ustr],
        installer_hash_url: &'static str,
        installer_path: &'static str,
        installer_url: &'static str,
        check_mirrors: bool,
        neoforge_versioning: bool,
    ) -> Result<(Arc<MinecraftVersion>, AddVanillaJar), LaunchError> {
        launch_tracker.add_count(1);
        launch_tracker.notify();

        let Some(version_link) = minecraft_versions.versions.iter().find(|v| v.id == instance_info.minecraft_version) else {
            return Err(LaunchError::CantFindVersion(instance_info.minecraft_version.as_str()));
        };

        let loader_version = if let Some(preferred_loader_version) = instance_info.preferred_loader_version {
            preferred_loader_version
        } else {
            let mut minecraft_version_parts = VersionFragment::string_to_parts(instance_info.minecraft_version.as_str());
            if neoforge_versioning {
                // 1.21.5 -> 21.5
                // 25w14craftmine -> 0.25w14craftmine
                // 1.21 -> 21.0
                // 26.1 -> 26.1.0
                if minecraft_version_parts[0] == VersionFragment::String("25w14craftmine".into()) {
                    minecraft_version_parts.insert(0, VersionFragment::Number(0))
                } else {
                    if minecraft_version_parts.len() < 3 {
                        minecraft_version_parts.push(VersionFragment::Number(0))
                    }
                    if minecraft_version_parts[0] == VersionFragment::Number(1) {
                        minecraft_version_parts.remove(0);
                    }
                }
            }

            let mut latest_loader_version = None;
            let mut latest_loader_version_parts = Vec::new();
            for version in loader_versions.iter() {
                let parts = VersionFragment::string_to_parts(version);

                if parts.starts_with(&minecraft_version_parts) {
                    if parts > latest_loader_version_parts {
                        latest_loader_version_parts = parts;
                        latest_loader_version = Some(version.clone());
                    }
                }
            }
            let Some(latest_loader_version) = latest_loader_version else {
                return Err(LaunchError::CantFindVersion(instance_info.minecraft_version.as_str()));
            };

            latest_loader_version
        };

        // Download base Minecraft version and neoforge installer hash
        let installer_hash_url = installer_hash_url.replace("{0}", &loader_version);
        let (base_version, installer_sha1) = futures::future::join(
            self.meta.fetch(&MinecraftVersionMetadataItem(version_link)),
            Self::download_sha1(http_client, &installer_hash_url)
        ).await;
        let base_version = base_version?;

        launch_tracker.add_count(1);
        launch_tracker.notify();

        // Download installer jar as an artifact
        let artifacts = &[
            GameLibraryArtifact {
                path: installer_path.replace("{0}", &loader_version).into(),
                sha1: installer_sha1,
                size: None,
                url: installer_url.replace("{0}", &loader_version).into(),
            },
            GameLibraryArtifact {
                path: format!("net/minecraft/{0}/minecraft-client-{0}.jar", instance_info.minecraft_version).into(),
                sha1: Some(base_version.downloads.client.sha1),
                size: Some(base_version.downloads.client.size),
                url: base_version.downloads.client.url,
            },
        ];

        let mojang_java_binary_future = self.load_mojang_java_binary(
            &self.meta,
            http_client,
            instance_info,
            &base_version,
            progress_trackers,
            launch_tracker,
        );
        let load_installer_library_future = self.load_libraries(http_client, artifacts, progress_trackers, launch_tracker);

        let (artifact_load_result, java_load_result) = futures::future::try_join(
            load_installer_library_future.map_err(LaunchError::from),
            mojang_java_binary_future.map_err(LaunchError::from),
        ).await?;
        let installer_path = &artifact_load_result[0].1;
        let minecraft_jar_path = &artifact_load_result[1].1;

        let installer_file = std::fs::File::open(installer_path)?;

        let installer_zip = installer_file.read_zip()?;

        // Read install profile
        let Some(install_profile_file) = installer_zip.by_name("install_profile.json") else {
            return Err(LaunchError::MissingFileInZipError(Cow::Borrowed("install_profile.json")));
        };

        let install_profile_bytes = install_profile_file.bytes()?;

        let install_profile = serde_json::from_slice(&install_profile_bytes);

        if install_profile.is_err() {
            if let Ok(install_profile_legacy) = serde_json::from_slice(&install_profile_bytes) {
                launch_tracker.add_count(1);
                let ret = self.create_forgelike_install_version_legacy(install_profile_legacy, installer_zip,
                    base_version, http_client, progress_trackers, launch_tracker, instance_info, check_mirrors).await;
                return ret;
            }
        }

        self.create_forgelike_install_version_modern(install_profile?, installer_zip,
            installer_path, minecraft_jar_path, &java_load_result, base_version, http_client,
            progress_trackers, launch_tracker, instance_info, check_mirrors).await
    }

    async fn create_forgelike_install_version_modern(
        &self,
        install_profile: ForgeInstallProfile,
        installer_zip: ArchiveHandle<'_, File>,
        installer_path: &PathBuf,
        minecraft_jar_path: &PathBuf,
        java_path: &PathBuf,
        base_version: Arc<MinecraftVersion>,
        http_client: &reqwest::Client,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
        instance_info: &InstanceConfiguration,
        check_mirrors: bool,
    ) -> Result<(Arc<MinecraftVersion>, AddVanillaJar), LaunchError> {
        if &*install_profile.minecraft != instance_info.minecraft_version.as_str() {
            return Err(LaunchError::MismatchedLoaderVersions(install_profile.minecraft.clone()));
        }

        // Read partial minecraft version
        let mut version_file_name = &*install_profile.json;
        if version_file_name.starts_with('/') {
            version_file_name = &version_file_name[1..];
        }
        let Some(version_file) = installer_zip.by_name(&version_file_name) else {
            return Err(LaunchError::MissingFileInZipError(Cow::Owned(version_file_name.to_string())));
        };
        let version: PartialMinecraftVersion = serde_json::from_slice(&version_file.bytes()?)?;

        // Download mirror list
        let mirror = if check_mirrors {
            Self::download_random_mirror(http_client, &install_profile.mirror_list).await
        } else {
            None
        };

        launch_tracker.add_count(1);
        launch_tracker.notify();

        // Download libraries
        let libraries = install_profile.libraries.iter().filter_map(|library| {
            let mut artifact = library.downloads.artifact.clone()?;
            if let Some(mirror) = &mirror {
                if artifact.url.starts_with("http") && !artifact.url.starts_with("https://libraries.minecraft.net/") && artifact.url.ends_with(artifact.path.as_str()) {
                    artifact.url = format!("{}{}", mirror, artifact.path).into();
                }
            }
            Some(artifact)
        }).collect::<Vec<_>>();

        self.load_libraries(http_client, &libraries, progress_trackers, launch_tracker).await?;

        let forge_temp = self.directories.temp_dir.join("forge_installer");

        let mut data = FxHashMap::default();

        for (key, sided_data) in install_profile.data {
            let value = sided_data.client;
            if value.is_empty() {
                continue;
            }

            if value.starts_with('[') && value.ends_with(']') {
                let artifact = MavenCoordinate::create(&value[1..value.len()-1]);
                let artifact_path = artifact.artifact_path();
                if let Some(target) = SafePath::new(&artifact_path) {
                    let target = target.to_path(&self.directories.libraries_dir);
                    data.insert(key, target.into_os_string());
                } else {
                    log::error!("Artifact generated invalid path: {}", artifact_path);
                }
            } else if value.starts_with('\'') && value.ends_with('\'') {
                data.insert(key, OsString::from(&value[1..value.len()-1]));
            } else {
                let mut file_name = &*value;
                if file_name.starts_with('/') {
                    file_name = &file_name[1..];
                }
                let Some(file) = installer_zip.by_name(&file_name) else {
                    return Err(LaunchError::MissingFileInZipError(Cow::Owned(file_name.to_string())));
                };
                if let Some(target) = SafePath::new(file_name) {
                    let target = target.to_path(&forge_temp);
                    crate::write_safe(&target, &file.bytes()?)?;
                    data.insert(key, target.into_os_string());
                } else {
                    log::error!("Unable to extract {}", file_name);
                }
            }
        }

        drop(installer_zip);

        data.insert("SIDE".into(), "client".into());
        data.insert("MINECRAFT_JAR".into(), minecraft_jar_path.as_os_str().to_os_string());
        data.insert("MINECRAFT_VERSION".into(), OsString::from(&*install_profile.minecraft));
        // ROOT is omitted
        data.insert("INSTALLER".into(), installer_path.as_os_str().to_os_string());
        data.insert("LIBRARY_DIR".into(), self.directories.libraries_dir.as_os_str().to_os_string());

        let processor_tracker = ProgressTracker::new("Forge Post Processors".into(), self.sender.clone());
        progress_trackers.push(processor_tracker.clone());

        processor_tracker.set_total(install_profile.processors.len());
        processor_tracker.notify();

        for processor in install_profile.processors.iter() {
            if let Some(sides) = &processor.sides {
                if !sides.iter().any(|side| *side == ForgeSide::Client) {
                    processor_tracker.add_count(1);
                    processor_tracker.notify();

                    continue;
                }
            }

            let jar = MavenCoordinate::create(&processor.jar);

            // Check if the output already exists and the step can be skipped
            let skip = self.can_skip_forge_processor(&jar, processor, &data);
            if skip {
                processor_tracker.add_count(1);
                processor_tracker.notify();
                continue;
            }

            let relative_jar_path = jar.artifact_path();

            let Some(safe_jar_path) = SafePath::new(&relative_jar_path) else {
                log::error!("Unable to run processor, invalid path: {}", relative_jar_path);
                processor_tracker.add_count(1);
                processor_tracker.notify();
                continue;
            };

            let jar_path = safe_jar_path.to_path(&self.directories.libraries_dir);
            let jar_file = std::fs::File::open(&jar_path)?;
            let jar_zip = jar_file.read_zip()?;

            let Some(manifest_file) = jar_zip.by_name("META-INF/MANIFEST.MF") else {
                return Err(LaunchError::MissingFileInZipError(Cow::Borrowed("META-INF/MANIFEST.MF")));
            };

            let manifest_bytes = manifest_file.bytes()?;

            drop(jar_zip);
            drop(jar_file);

            let Ok(manifest_str) = str::from_utf8(&manifest_bytes) else {
                log::error!("Unable to run processor, MANIFEST.MF is not utf8 encoded");
                processor_tracker.add_count(1);
                processor_tracker.notify();
                continue;
            };

            let manifest_map = crate::java_manifest::parse_java_manifest(manifest_str);

            let Some(main_class) = manifest_map.get("Main-Class") else {
                log::error!("Unable to run processor, can't find Main-Class in MANIFEST.MF");
                processor_tracker.add_count(1);
                processor_tracker.notify();
                continue;
            };

            let mut command = std::process::Command::new(java_path);

            command.current_dir(&forge_temp);
            command.stdin(Stdio::inherit());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());

            command.arg("-cp");
            command.arg(std::env::join_paths(processor.classpath.iter().map(|f| {
                let artifact = MavenCoordinate::create(&**f);
                self.directories.libraries_dir.join(artifact.artifact_path()).into_os_string()
            }).chain(std::iter::once(jar_path.into_os_string()))).unwrap());

            command.arg(main_class);

            for arg in processor.args.iter() {
                let expanded = if arg.starts_with('[') && arg.ends_with(']') {
                    let artifact = MavenCoordinate::create(&arg[1..arg.len()-1]);
                    let artifact_path = artifact.artifact_path();
                    if let Some(target) = SafePath::new(&artifact_path) {
                        let target = target.to_path(&self.directories.libraries_dir);
                        Cow::Owned(target.into_os_string())
                    } else {
                        log::error!("Artifact generated invalid path: {}", artifact_path);
                        continue;
                    }
                } else if &**arg == "{ROOT}/libraries/" {
                    Cow::Borrowed(self.directories.libraries_dir.as_os_str())
                } else {
                    expand_forge_argument(&arg, &data)
                };
                command.arg(expanded);
            }

            let mut child = command.spawn()?;
            let exit_code = child.wait()?;

            if !exit_code.success() {
                return Err(LaunchError::ForgePostProcessorError);
            }

            processor_tracker.add_count(1);
            processor_tracker.notify();
        }

        processor_tracker.set_finished(ProgressTrackerFinishType::Normal);

        launch_tracker.add_count(1);
        launch_tracker.notify();

        Ok((Arc::new(version.apply_to(&base_version)), AddVanillaJar::No))
    }

    async fn create_forgelike_install_version_legacy(
        &self,
        install_profile: ForgeInstallProfileLegacy,
        installer_zip: ArchiveHandle<'_, File>,
        base_version: Arc<MinecraftVersion>,
        http_client: &reqwest::Client,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
        instance_info: &InstanceConfiguration,
        check_mirrors: bool,
    ) -> Result<(Arc<MinecraftVersion>, AddVanillaJar), LaunchError> {
        if &*install_profile.install.minecraft != instance_info.minecraft_version.as_str() {
            return Err(LaunchError::MismatchedLoaderVersions(install_profile.install.minecraft.clone()));
        }

        // Extract forge jar
        let Some(file) = installer_zip.by_name(&install_profile.install.file_path) else {
            return Err(LaunchError::MissingFileInZipError(Cow::Owned(install_profile.install.file_path.to_string())));
        };
        let forge_path = MavenCoordinate::create(&install_profile.install.path).artifact_path();
        if !path_is_normal(forge_path.as_str()) {
            return Err(LoadLibrariesError::IllegalLibraryPath(forge_path.into()).into());
        }
        let forge_artifact_path = self.directories.libraries_dir.join(forge_path.as_str());
        crate::write_safe(&forge_artifact_path, &file.bytes()?)?;

        // Read partial minecraft version
        let version: PartialMinecraftVersion = install_profile.version_info.into_partial_version(ForgeSide::Client);

        // Download mirror list
        let mirror = if check_mirrors {
            Self::download_random_mirror(http_client, &install_profile.install.mirror_list).await
        } else {
            None
        };

        launch_tracker.add_count(1);
        launch_tracker.notify();

        // Download libraries with mirror
        if let Some(libraries) = &version.libraries {
            let libraries = libraries.iter().filter_map(|library| {
                let mut artifact = library.downloads.artifact.clone()?;
                if let Some(mirror) = &mirror {
                    if artifact.url.starts_with("http") && !artifact.url.starts_with("https://libraries.minecraft.net/") && artifact.url.ends_with(artifact.path.as_str()) {
                        artifact.url = format!("{}{}", mirror, artifact.path).into();
                    }
                }
                Some(artifact)
            }).collect::<Vec<_>>();

            self.load_libraries(http_client, &libraries, progress_trackers, launch_tracker).await?;
        }

        Ok((Arc::new(version.apply_to(&base_version)), AddVanillaJar::Yes))
    }

    async fn download_sha1(http_client: &reqwest::Client, url: &str) -> Option<Ustr> {
        let response = http_client
            .get(url)
            .send().await.ok()?;

        let bytes = response.bytes().await.ok()?;

        if bytes.len() != 40 {
            return None;
        }

        Some(str::from_utf8(&bytes).ok()?.into())
    }

    async fn download_random_mirror(http_client: &reqwest::Client, url: &str) -> Option<Arc<str>> {
        let response = http_client
            .get(url)
            .send().await.ok()?;

        let bytes = response.bytes().await.ok()?;

        #[derive(Deserialize)]
        struct Mirror {
            url: Arc<str>,
        }

        let mirrors: Vec<Mirror> = serde_json::from_slice(&bytes).ok()?;

        let mirror = mirrors.choose(&mut rand::thread_rng())?;

        Some(mirror.url.clone())
    }

    async fn load_mojang_java_binary(
        &self,
        meta: &MetadataManager,
        http_client: &reqwest::Client,
        configuration: &InstanceConfiguration,
        version_info: &MinecraftVersion,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
    ) -> Result<PathBuf, LoadJavaRuntimeError> {
        if let Some(jvm_binary) = &configuration.jvm_binary {
            if jvm_binary.enabled && let Some(path) = &jvm_binary.path {
                if let Some(binary) = Self::search_for_java_binary(&path) {
                    return Ok(binary);
                }
            }
        }

        if let Some(force_external_java) = std::env::var_os("FORCE_EXTERNAL_JAVA") {
            let paths = std::env::split_paths(&force_external_java);

            let mut found_versions = BTreeSet::new();

            let needed_version = if let Some(java_version) = &version_info.java_version {
                java_version.major_version
            } else {
                8
            };

            for path in paths {
                let Some(binary) = Self::search_for_java_binary(&path) else {
                    continue;
                };

                let Some(major_version) = self.get_major_java_version(&binary) else {
                    continue;
                };

                if major_version == needed_version {
                    return Ok(binary);
                } else {
                    found_versions.insert(major_version);
                }
            }

            return Err(LoadJavaRuntimeError::UnableToFindExternalBinary(needed_version, found_versions.into_iter().collect()));
        }

        let mut platform: Ustr = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => "linux".into(),
            ("linux", "x86") => "linux-i386".into(),
            ("macos", "x86_64") => "mac-os".into(),
            ("macos", "aarch64") => "mac-os-arm64".into(),
            ("windows", "aarch64") => "windows-arm64".into(),
            ("windows", "x86_64") => "windows-x64".into(),
            ("windows", "x86") => "windows-x86".into(),
            ("macos", b) => format!("mac-os-{b}").into(),
            (a, b) => format!("{a}-{b}").into(),
        };

        let jre_component = if let Some(java_version) = &version_info.java_version {
            java_version.component
        } else {
            "jre-legacy".into()
        };

        let runtimes = meta.fetch(&MojangJavaRuntimesMetadataItem).await?;

        let mut runtime_platform = runtimes.platforms.get(&platform).ok_or(LoadJavaRuntimeError::UnknownPlatform)?;
        let mut runtime_components = runtime_platform.components.get(&jre_component);

        // Fall back to x86 runtime on mac-os, since Rosetta exists
        let missing_runtime_component = runtime_components.map(Vec::is_empty).unwrap_or(true);
        if missing_runtime_component && platform == "mac-os-arm64" {
            platform = "mac-os".into();
            runtime_platform = runtimes.platforms.get(&platform).ok_or(LoadJavaRuntimeError::UnknownPlatform)?;
            runtime_components = runtime_platform.components.get(&jre_component);
        }

        let Some(runtime_components) = runtime_components else {
            return Err(LoadJavaRuntimeError::UnknownComponentForPlatform);
        };
        let runtime_component = runtime_components.first().ok_or(LoadJavaRuntimeError::UnknownComponentForPlatform)?;

        if !crate::is_single_component_path(jre_component.as_str()) {
            return Err(LoadJavaRuntimeError::InvalidComponentPath);
        }
        if !crate::is_single_component_path(&platform) {
            return Err(LoadJavaRuntimeError::InvalidComponentPath);
        }

        let runtime_component_dir = self.directories.runtime_base_dir.join(jre_component).join(platform);
        let _ = std::fs::create_dir_all(&runtime_component_dir);
        let Ok(runtime_component_dir) = runtime_component_dir.canonicalize() else {
            return Err(LoadJavaRuntimeError::InvalidComponentPath);
        };

        let fresh_install = !runtime_component_dir.exists();

        let runtime = meta.fetch(&MojangJavaRuntimeComponentMetadataItem {
            url: runtime_component.manifest.url,
            cache: runtime_component_dir.join("manifest.json").into(),
            hash: runtime_component.manifest.sha1,
        }).await?;

        let initial_title = if fresh_install {
            "Downloading Java Runtime"
        } else {
            "Verifying integrity of Java Runtime"
        };

        let java_runtime_tracker = ProgressTracker::new(initial_title.into(), self.sender.clone());
        progress_trackers.push(java_runtime_tracker.clone());
        java_runtime_tracker.notify();

        let result = do_java_runtime_load(http_client, runtime_component_dir, fresh_install, runtime, &java_runtime_tracker).await;

        java_runtime_tracker.set_finished(ProgressTrackerFinishType::from_err(result.is_err()));
        java_runtime_tracker.notify();

        launch_tracker.add_count(1);
        launch_tracker.notify();

        result
    }

    async fn load_assets(
        &self,
        meta: &MetadataManager,
        http_client: &reqwest::Client,
        game_dir: &Arc<Path>,
        version_info: &MinecraftVersion,
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
    ) -> Result<String, LoadAssetObjectsError> {
        let asset_index = format!("{}", version_info.assets);

        let assets_index = meta.fetch(&AssetsIndexMetadataItem {
            url: version_info.asset_index.url,
            cache: self.directories.assets_index_dir.join(format!("{}.json", &asset_index)).into(),
            hash: version_info.asset_index.sha1,
        }).await?;

        let initial_title = Arc::from("Verifying integrity of game assets");
        let assets_tracker = ProgressTracker::new(initial_title, self.sender.clone());
        progress_trackers.push(assets_tracker.clone());
        assets_tracker.notify();

        let assets_dir = if assets_index.map_to_resources == Some(true) {
            game_dir.join("resources").into()
        } else if assets_index.r#virtual == Some(true) {
            self.directories.assets_root_dir.join("virtual").join("legacy").into()
        } else {
            self.directories.assets_objects_dir.clone()
        };

        let result = do_asset_objects_load(http_client, assets_index, assets_dir, &assets_tracker).await;

        assets_tracker.set_finished(ProgressTrackerFinishType::from_err(result.is_err()));
        assets_tracker.notify();

        launch_tracker.add_count(1);
        launch_tracker.notify();

        result?;

        Ok(asset_index)
    }

    async fn load_libraries(
        &self,
        http_client: &reqwest::Client,
        artifacts: &[GameLibraryArtifact],
        progress_trackers: &ProgressTrackers,
        launch_tracker: &ProgressTracker,
    ) -> Result<Vec<(Ustr, PathBuf)>, LoadLibrariesError> {
        let initial_title = Arc::from("Verifying integrity of game libraries");
        let libraries_tracker = ProgressTracker::new(initial_title, self.sender.clone());
        progress_trackers.push(libraries_tracker.clone());
        libraries_tracker.notify();

        let result =
            do_libraries_load(http_client, artifacts, self.directories.libraries_dir.clone(), &libraries_tracker).await;

        libraries_tracker.set_finished(ProgressTrackerFinishType::from_err(result.is_err()));
        libraries_tracker.notify();

        launch_tracker.add_count(1);
        launch_tracker.notify();

        result
    }

    async fn load_log_configuration(
        &self,
        http_client: &reqwest::Client,
        logging: Option<&GameLogging>,
    ) -> Option<OsString> {
        let Some(logging) = logging else {
            return None;
        };
        let Some(client) = &logging.client else {
            return None;
        };
        let id = client.file.id.as_str();
        if !path_is_normal(id) {
            log::error!("Log configuration has invalid path: {}", id);
            return None;
        }
        let path = self.directories.log_configs_dir.join(id);

        let _ = std::fs::create_dir(&self.directories.log_configs_dir);

        let mut expected_hash = [0u8; 20];
        let Ok(_) = hex::decode_to_slice(client.file.sha1.as_str(), &mut expected_hash) else {
            log::error!("Log configuration has invalid sha1: {}", client.file.sha1.as_str());
            return None;
        };

        let valid_hash_on_disk = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                crate::check_sha1_hash(&path, expected_hash).unwrap_or(false)
            }).await.unwrap()
        };

        if valid_hash_on_disk {
            return Some(expand_logging_argument(client.argument.as_str(), &path));
        }

        let Ok(response) = http_client.get(client.file.url.as_str()).send().await else {
            log::error!("Failed to make request to download log configuration");
            return None;
        };
        let Ok(bytes) = response.bytes().await else {
            log::error!("Failed to download log configuration");
            return None;
        };
        let bytes = Arc::new(bytes);

        if bytes.len() != client.file.size as usize {
            log::error!("Rejecting log configuration because invalid size");
            return None;
        }

        let correct_hash = {
            let bytes = Arc::clone(&bytes);

            tokio::task::spawn_blocking(move || {
                let mut hasher = Sha1::new();
                hasher.update(&*bytes);
                let actual_hash = hasher.finalize();

                expected_hash == *actual_hash
            }).await.unwrap()
        };

        if !correct_hash {
            log::error!("Log configuration has incorrect hash");
            return None;
        }

        let Ok(_) = tokio::fs::write(path.clone(), &*bytes).await else {
            log::error!("Failed to write log configuration to disk");
            return None;
        };

        Some(expand_logging_argument(client.argument.as_str(), &path))
    }

    fn can_skip_forge_processor(&self, jar: &MavenCoordinate<'_>, processor: &schema::forge::ForgeInstallProcessor, data: &FxHashMap<String, OsString>) -> bool {
        if let Some(outputs) = &processor.outputs {
            if outputs.is_empty() {
                return false;
            }
            for (key, value) in outputs {
                let key = expand_forge_argument(key, data);
                let value = expand_forge_argument(value, data);

                let Some(value) = value.to_str() else {
                    return false;
                };

                let mut expected_hash = [0u8; 20];
                let Ok(_) = hex::decode_to_slice(value, &mut expected_hash) else {
                    return false;
                };

                if !crate::check_sha1_hash(Path::new(&key), expected_hash).unwrap_or(false) {
                    return false;
                }
            }
            true
        } else {
            let mut argmap = FxHashMap::default();

            let mut last_key = None;
            for arg in processor.args.iter() {
                if arg.starts_with("--") {
                    last_key = Some(arg.as_str());
                } else if let Some(key) = last_key.take() {
                    argmap.insert(key, arg.as_str());
                }
            }

            match jar.artifact_id {
                "installertools" => {
                    match argmap.get("--task") {
                        Some(&"PROCESS_MINECRAFT_JAR") => {
                            if argmap.get("--output") == Some(&"{PATCHED}") {
                                if let Some(path) = data.get("PATCHED") {
                                    return Path::new(path).exists();
                                }
                            }
                        },
                        Some(&"MCP_DATA") => {
                            if argmap.get("--output") == Some(&"{MAPPINGS}") {
                                if let Some(path) = data.get("MAPPINGS") {
                                    return Path::new(path).exists();
                                }
                            }
                        },
                        Some(&"DOWNLOAD_MOJMAPS") => {
                            if argmap.get("--output") == Some(&"{MOJMAPS}") {
                                if let Some(path) = data.get("MOJMAPS") {
                                    return Path::new(path).exists();
                                }
                            }
                        },
                        Some(&"MERGE_MAPPING") => {
                            if argmap.get("--output") == Some(&"{MERGED_MAPPINGS}") {
                                if let Some(path) = data.get("MERGED_MAPPINGS") {
                                    return Path::new(path).exists();
                                }
                            }
                        },
                        _ => {}
                    }
                },
                "jarsplitter" => {
                    if argmap.get("--input") != Some(&"{MINECRAFT_JAR}") {
                        return false;
                    }
                    if argmap.get("--slim") != Some(&"{MC_SLIM}") {
                        return false;
                    }
                    if argmap.get("--extra") != Some(&"{MC_EXTRA}") {
                        return false;
                    }
                    let Some(slim_path) = data.get("MC_SLIM") else {
                        return false;
                    };
                    let Some(extra_path) = data.get("MC_EXTRA") else {
                        return false;
                    };
                    return Path::new(slim_path).exists() && Path::new(extra_path).exists();
                },
                "AutoRenamingTool" => {
                    if argmap.get("--input") != Some(&"{MC_SLIM}") {
                        return false;
                    }
                    if argmap.get("--output") != Some(&"{MC_SRG}") {
                        return false;
                    }
                    if let Some(path) = data.get("MC_SRG") {
                        return Path::new(path).exists();
                    }
                },
                "binarypatcher" => {
                    if argmap.get("--clean") != Some(&"{MC_SRG}") {
                        return false;
                    }
                    if argmap.get("--output") != Some(&"{PATCHED}") {
                        return false;
                    }
                    if let Some(path) = data.get("PATCHED") {
                        return Path::new(path).exists();
                    }
                },
                _ => {}
            }
            false
        }
    }

    fn search_for_java_binary(path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }

        let paths: &[&'static str] = if std::env::consts::OS == "linux" {
            &["bin", "java"]
        } else if std::env::consts::OS == "macos" {
            &["jre.bundle", "Contents", "Home", "bin", "java"]
        } else if std::env::consts::OS == "windows" {
            &["bin", "javaw.exe"]
        } else {
            return None;
        };

        for start in (0..paths.len()).rev() {
            let mut new_path = path.to_path_buf();
            for fragment_index in start..paths.len() {
                let fragment = paths[fragment_index];
                new_path.push(fragment);
                if fragment_index == paths.len()-1 {
                    if new_path.is_file() {
                        return Some(new_path);
                    } else {
                        break;
                    }
                } else if !new_path.is_dir() {
                    break;
                }
            }
        }

        None
    }

    fn get_major_java_version(&self, binary: &Path) -> Option<u32> {
        let mut command = std::process::Command::new(binary);
        command.arg("-jar");
        command.arg(self.launch_wrapper.as_os_str().to_os_string());
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());

        let mut process = command.spawn().ok()?;

        let mut stdin = process.stdin.take().unwrap();
        stdin.write_all(b"printproperty\njava.specification.version\nexit\n").ok()?;
        stdin.flush().ok()?;

        let output = process.wait_with_output().ok()?;

        if !output.status.success() {
            return None;
        }

        let output = output.stdout.trim_ascii();
        let mut output = str::from_utf8(output).ok()?;
        if output.starts_with("1.") {
            output = &output[2..];
        }
        output.parse().ok()
    }
}

fn expand_logging_argument(argument: &str, path: &Path) -> OsString {
    let mut dollar_last = false;
    let mut builder = OsString::new();
    let mut copied_to_builder = 0;
    for (i, character) in argument.char_indices() {
        if character == '$' {
            dollar_last = true;
        } else if dollar_last && character == '{' {
            let remaining = &argument[i..];
            if let Some(end) = remaining.find('}') {
                let to_expand = &argument[i+1..i+end];
                if to_expand == "path" {
                    builder.push(&argument[copied_to_builder..i-1]);
                    builder.push(path.as_os_str());
                    copied_to_builder = i+end+1;
                } else {
                    panic!("Unsupported argument: {:?}", to_expand);
                }
            }
        } else {
            dollar_last = false;
        }
    }
    builder.push(&argument[copied_to_builder..]);
    builder
}

fn calculate_natives_dirname(artifacts: &[GameLibraryArtifact]) -> String {
    let mut hashes = HashSet::new();

    for artifact in artifacts {
        let mut hash = [0_u8; 20];
        let Some(sha1) = &artifact.sha1 else {
            continue;
        };
        if hex::decode_to_slice(sha1.as_str(), &mut hash).is_ok() {
            hashes.insert(hash);
        }
    }

    let mut combined = [0_u8; 20];
    for hash in hashes {
        for i in 0..20 {
            combined[i] ^= hash[i];
        }
    }
    hex::encode(combined)
}

#[derive(thiserror::Error, Debug)]
pub enum LoadJavaRuntimeError {
    #[error("Failed to load remote content:\n{0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Failed to perform I/O operation:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to load metadata:\n{0}")]
    MetaLoadError(#[from] MetaLoadError),
    #[error("Hash isn't a valid sha1 hash:\n{0}")]
    InvalidHash(Ustr),
    #[error("Unknown platform")]
    UnknownPlatform,
    #[error("Unknown component for platform")]
    UnknownComponentForPlatform,
    #[error("Mojang runtime path is invalid")]
    InvalidComponentPath,
    #[error("Downloaded file had wrong response size. Expected {0}, got {1}")]
    WrongResponseSize(usize, usize),
    #[error("Downloaded file had wrong raw size")]
    WrongRawSize,
    #[error("Failed to decompress file")]
    Lzma(#[from] lzma_rs::error::Error),
    #[error("Downloaded file had the wrong hash")]
    WrongHash,
    #[error("Unable to find binary")]
    UnableToFindBinary,
    #[error("Unable to find external binary, needed Java {0}, got Java {1:?}")]
    UnableToFindExternalBinary(u32, Vec<u32>),
}

async fn do_java_runtime_load(
    http_client: &reqwest::Client,
    runtime_component_dir: PathBuf,
    fresh_install: bool,
    runtime: Arc<JavaRuntimeComponentManifest>,
    java_runtime_tracker: &ProgressTracker,
) -> Result<PathBuf, LoadJavaRuntimeError> {
    let mut links = HashMap::new();

    // Limit max concurrent connections to 8 to avoid ratelimiting issues
    let download_semaphore = tokio::sync::Semaphore::new(8);
    let disk_semaphore = tokio::sync::Semaphore::new(32);
    let started_downloading = AtomicBool::new(fresh_install);

    let mut tasks = Vec::new();

    let mut total_size = 0;

    for (filename, contents) in &runtime.files {
        if !path_is_normal(filename) {
            continue;
        }

        let path = runtime_component_dir.join(filename);

        match contents {
            JavaRuntimeComponentFile::Directory => {
                let _ = std::fs::create_dir(path);
            },
            JavaRuntimeComponentFile::File { executable, downloads } => {
                let mut expected_hash = [0u8; 20];
                let Ok(_) = hex::decode_to_slice(downloads.raw.sha1.as_str(), &mut expected_hash) else {
                    return Err(LoadJavaRuntimeError::InvalidHash(downloads.raw.sha1));
                };

                total_size += downloads.raw.size;

                let started_downloading = &started_downloading;
                let download_semaphore = &download_semaphore;
                let disk_semaphore = &disk_semaphore;

                let task = async move {
                    let valid_hash_on_disk = {
                        let path = path.clone();
                        let permit = disk_semaphore.acquire().await.unwrap();
                        let result = tokio::task::spawn_blocking(move || {
                            crate::check_sha1_hash(&path, expected_hash).unwrap_or(false)
                        }).await.unwrap();
                        drop(permit);
                        result
                    };

                    if valid_hash_on_disk {
                        java_runtime_tracker.add_count(downloads.raw.size as usize);
                        java_runtime_tracker.notify();
                        return Ok(());
                    }

                    let was_downloading = started_downloading.swap(true, std::sync::atomic::Ordering::Relaxed);
                    if !was_downloading {
                        java_runtime_tracker.set_title(Arc::from("Downloading Java Runtime"));
                    }

                    let (lzma, size, download) = if let Some(lzma) = &downloads.lzma {
                        (true, lzma.size as usize, lzma)
                    } else {
                        (false, downloads.raw.size as usize, &downloads.raw)
                    };

                    let permit = download_semaphore.acquire().await.unwrap();
                    let response = http_client.get(download.url.as_str()).send().await?;
                    let bytes = response.bytes().await?;
                    drop(permit);

                    if bytes.len() != size {
                        return Err(LoadJavaRuntimeError::WrongResponseSize(size, bytes.len()));
                    }

                    let decompressed_or_raw = if lzma {
                        let result = tokio::task::spawn_blocking(move || {
                            let mut output = Vec::new();
                            lzma_rs::lzma_decompress(&mut std::io::Cursor::new(bytes), &mut output)?;
                            Ok(output)
                        }).await.unwrap();

                        match result {
                            Ok(decompressed) => Ok(decompressed),
                            Err(lzma_error) => {
                                return Err(LoadJavaRuntimeError::Lzma(lzma_error));
                            },
                        }
                    } else {
                        Err(bytes)
                    };

                    let decompressed_or_raw = Arc::new(decompressed_or_raw);

                    let bytes = match &*decompressed_or_raw {
                        Ok(vec) => vec.as_slice(),
                        Err(bytes) => bytes,
                    };

                    if bytes.len() != downloads.raw.size as usize {
                        return Err(LoadJavaRuntimeError::WrongRawSize);
                    }

                    let valid_hash = {
                        let decompressed_or_raw = Arc::clone(&decompressed_or_raw);
                        tokio::task::spawn_blocking(move || {
                            let bytes = match &*decompressed_or_raw {
                                Ok(vec) => vec.as_slice(),
                                Err(bytes) => bytes,
                            };

                            let mut hasher = Sha1::new();
                            hasher.update(bytes);
                            let actual_hash = hasher.finalize();

                            expected_hash == *actual_hash
                        }).await.unwrap()
                    };

                    if !valid_hash {
                        return Err(LoadJavaRuntimeError::WrongHash);
                    }

                    if let Some(parent) = path.parent() {
                        _ = std::fs::create_dir_all(parent);
                    }
                    tokio::fs::write(&path, bytes).await?;

                    #[cfg(unix)]
                    if *executable {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).await;
                    }

                    java_runtime_tracker.add_count(downloads.raw.size as usize);
                    java_runtime_tracker.notify();
                    Ok(())
                };
                tasks.push(task);
            },
            JavaRuntimeComponentFile::Link { target } => {
                links.insert(path, target.clone());
            },
        }
    }
    java_runtime_tracker.set_total(total_size as usize);
    java_runtime_tracker.notify();

    futures::future::try_join_all(tasks).await?;

    for (path, target) in links {
        if let Some(parent) = path.parent()
            && let Ok(absolute_target) = parent.join(target).canonicalize()
            && absolute_target.starts_with(&runtime_component_dir)
        {
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(absolute_target, path);

            #[cfg(windows)]
            if absolute_target.is_dir() {
                let _ = std::os::windows::fs::symlink_dir(absolute_target, path);
            } else {
                let _ = std::os::windows::fs::symlink_file(absolute_target, path);
            }
        }
    }

    let bin_java = runtime_component_dir.join("bin/java");
    if let Ok(bin_java) = bin_java.canonicalize() {
        return Ok(bin_java);
    }

    let bin_javaw = runtime_component_dir.join("bin/javaw.exe");
    if let Ok(bin_javaw) = bin_javaw.canonicalize() {
        return Ok(bin_javaw);
    }

    let jre_bundle_path = runtime_component_dir.join("jre.bundle/Contents/Home/bin/java");
    if let Ok(jre_bundle_path) = jre_bundle_path.canonicalize() {
        return Ok(jre_bundle_path);
    }

    let legacy_exe = runtime_component_dir.join("MinecraftJava.exe");
    if let Ok(legacy_exe) = legacy_exe.canonicalize() {
        return Ok(legacy_exe);
    }

    Err(LoadJavaRuntimeError::UnableToFindBinary)
}

#[derive(thiserror::Error, Debug)]
pub enum LoadAssetObjectsError {
    #[error("Failed to load remote content")]
    Reqwest(#[from] reqwest::Error),
    #[error("Failed to perform I/O operation")]
    IoError(#[from] std::io::Error),
    #[error("Hash isn't a valid sha1 hash\n{0}")]
    InvalidHash(Ustr),
    #[error("Downloaded file had wrong response size. Expected {0}, got {1}")]
    WrongResponseSize(usize, usize),
    #[error("Downloaded file had the wrong hash")]
    WrongHash,
    #[error("Failed to load metadata:\n{0}")]
    MetaLoadError(#[from] MetaLoadError),
}

async fn do_asset_objects_load(
    http_client: &reqwest::Client,
    assets_index: Arc<AssetsIndex>,
    assets_objects_dir: Arc<Path>,
    assets_tracker: &ProgressTracker,
) -> Result<(), LoadAssetObjectsError> {
    // Limit max concurrent connections to 8 to avoid ratelimiting issues
    let download_semaphore = tokio::sync::Semaphore::new(8);
    let disk_semaphore = tokio::sync::Semaphore::new(32);
    let started_downloading = AtomicBool::new(false);

    let mut total_size = 0;

    let mut tasks = Vec::new();

    let _ = std::fs::create_dir_all(&assets_objects_dir);

    for (_, asset) in &assets_index.objects {
        let mut expected_hash = [0u8; 20];
        let Ok(_) = hex::decode_to_slice(asset.hash.as_str(), &mut expected_hash) else {
            return Err(LoadAssetObjectsError::InvalidHash(asset.hash));
        };

        let mut path = assets_objects_dir.join(&asset.hash[..2]);
        let _ = std::fs::create_dir(&path);
        path.push(asset.hash.as_str());

        total_size += asset.size;

        let started_downloading = &started_downloading;
        let download_semaphore = &download_semaphore;
        let disk_semaphore = &disk_semaphore;

        let url = format!("https://resources.download.minecraft.net/{}/{}", &asset.hash[..2], &asset.hash);

        let task = async move {
            let valid_hash_on_disk = {
                let path = path.clone();
                let permit = disk_semaphore.acquire().await.unwrap();
                let result = tokio::task::spawn_blocking(move || {
                    crate::check_sha1_hash(&path, expected_hash).unwrap_or(false)
                }).await.unwrap();
                drop(permit);
                result
            };

            if valid_hash_on_disk {
                assets_tracker.add_count(asset.size as usize);
                assets_tracker.notify();
                return Ok(());
            }

            let was_downloading = started_downloading.swap(true, std::sync::atomic::Ordering::Relaxed);
            if !was_downloading {
                assets_tracker.set_title(Arc::from("Downloading game assets"));
            }

            let permit = download_semaphore.acquire().await.unwrap();
            let response = http_client.get(&url).send().await?;
            let bytes = Arc::new(response.bytes().await?);
            drop(permit);

            if bytes.len() != asset.size as usize {
                return Err(LoadAssetObjectsError::WrongResponseSize(asset.size as usize, bytes.len()));
            }

            let correct_hash = {
                let bytes = Arc::clone(&bytes);

                tokio::task::spawn_blocking(move || {
                    let mut hasher = Sha1::new();
                    hasher.update(&*bytes);
                    let actual_hash = hasher.finalize();

                    expected_hash == *actual_hash
                }).await.unwrap()
            };

            if !correct_hash {
                return Err(LoadAssetObjectsError::WrongHash);
            }

            tokio::fs::write(path.clone(), &*bytes).await?;
            assets_tracker.add_count(asset.size as usize);
            assets_tracker.notify();
            Ok(())
        };
        tasks.push(task);
    }

    assets_tracker.set_total(total_size as usize);
    assets_tracker.notify();

    futures::future::try_join_all(tasks).await?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum LoadLibrariesError {
    #[error("Failed to load remote content")]
    Reqwest(#[from] reqwest::Error),
    #[error("Failed to perform I/O operation")]
    IoError(#[from] std::io::Error),
    #[error("Hash isn't a valid sha1 hash\n{0}")]
    InvalidHash(Ustr),
    #[error("Downloaded file had wrong response size. Expected {0}, got {1}")]
    WrongResponseSize(usize, usize),
    #[error("Downloaded file had the wrong hash")]
    WrongHash,
    #[error("Illegal library path {0}, directory traversal?")]
    IllegalLibraryPath(Ustr),
}

async fn do_libraries_load(
    http_client: &reqwest::Client,
    artifacts: &[GameLibraryArtifact],
    libraries_dir: Arc<Path>,
    libraries_tracker: &ProgressTracker,
) -> Result<Vec<(Ustr, PathBuf)>, LoadLibrariesError> {
    // Limit max concurrent connections to 8 to avoid ratelimiting issues
    let download_semaphore = tokio::sync::Semaphore::new(8);
    let disk_semaphore = tokio::sync::Semaphore::new(32);
    let started_downloading = AtomicBool::new(false);

    let mut total_size = 0;

    let mut tasks = Vec::new();

    let _ = std::fs::create_dir_all(&libraries_dir);

    for artifact in artifacts {
        let expected_hash = if let Some(sha1) = &artifact.sha1 {
            let mut expected_hash = [0u8; 20];
            let Ok(_) = hex::decode_to_slice(sha1.as_str(), &mut expected_hash) else {
                return Err(LoadLibrariesError::InvalidHash(*sha1));
            };
            Some(expected_hash)
        } else {
            None
        };

        if !path_is_normal(artifact.path.as_str()) {
            return Err(LoadLibrariesError::IllegalLibraryPath(artifact.path));
        }

        let artifact_path = libraries_dir.join(artifact.path.as_str());
        let Some(artifact_path_parent) = artifact_path.parent() else {
            return Err(LoadLibrariesError::IllegalLibraryPath(artifact.path));
        };
        let _ = std::fs::create_dir_all(artifact_path_parent);

        let tracker_size = artifact.size.unwrap_or(1000000);
        total_size += tracker_size;

        let started_downloading = &started_downloading;
        let download_semaphore = &download_semaphore;
        let disk_semaphore = &disk_semaphore;

        let task = async move {
            let valid_hash_on_disk = if let Some(expected_hash) = expected_hash {
                let artifact_path = artifact_path.clone();
                let permit = disk_semaphore.acquire().await.unwrap();
                let result = tokio::task::spawn_blocking(move || {
                    crate::check_sha1_hash(&artifact_path, expected_hash).unwrap_or(false)
                }).await.unwrap();
                drop(permit);
                result
            } else {
                artifact_path.exists()
            };

            if valid_hash_on_disk {
                libraries_tracker.add_count(tracker_size as usize);
                libraries_tracker.notify();
                return Ok((artifact.path, artifact_path));
            }

            let was_downloading = started_downloading.swap(true, std::sync::atomic::Ordering::Relaxed);
            if !was_downloading {
                libraries_tracker.set_title(Arc::from("Downloading game libraries"));
            }

            let permit = download_semaphore.acquire().await.unwrap();
            let response = http_client.get(artifact.url.as_str()).send().await?;
            let bytes = Arc::new(response.bytes().await?);
            drop(permit);

            if let Some(artifact_size) = artifact.size && bytes.len() != artifact_size as usize {
                return Err(LoadLibrariesError::WrongResponseSize(artifact_size as usize, bytes.len()));
            }

            let correct_hash = {
                if let Some(expected_hash) = expected_hash {
                    let bytes = Arc::clone(&bytes);

                    tokio::task::spawn_blocking(move || {
                        let mut hasher = Sha1::new();
                        hasher.update(&*bytes);
                        let actual_hash = hasher.finalize();

                        expected_hash == *actual_hash
                    }).await.unwrap()
                } else {
                    true
                }
            };

            if !correct_hash {
                return Err(LoadLibrariesError::WrongHash);
            }

            tokio::fs::write(artifact_path.clone(), &*bytes).await?;
            libraries_tracker.add_count(tracker_size as usize);
            libraries_tracker.notify();
            Ok((artifact.path, artifact_path))
        };
        tasks.push(task);
    }

    libraries_tracker.set_total(total_size as usize);
    libraries_tracker.notify();

    futures::future::try_join_all(tasks).await
}

pub enum ArgumentExpansionKey {
    NativesDirectory,
    LibrariesDirectory,
    ClasspathSeparator,
    LauncherName,
    LauncherVersion,
    Classpath,
    AuthPlayerName,
    VersionName,
    GameDirectory,
    AssetsRoot,
    AssetsIndexName,
    AuthUuid,
    AuthAccessToken,
    Clientid,
    AuthXuid,
    VersionType,
    QuickPlayPath,
    UserProperties,
    UserType,
    ResolutionWidth,
    ResolutionHeight,
    QuickPlaySingleplayer,
    QuickPlayMultiplayer,
    QuickPlayRealms,
}

impl ArgumentExpansionKey {
    pub fn from_str(string: &str) -> Option<Self> {
        match string {
            "natives_directory" => Some(Self::NativesDirectory),
            "library_directory" => Some(Self::LibrariesDirectory),
            "classpath_separator" => Some(Self::ClasspathSeparator),
            "launcher_name" => Some(Self::LauncherName),
            "launcher_version" => Some(Self::LauncherVersion),
            "classpath" => Some(Self::Classpath),
            "auth_player_name" => Some(Self::AuthPlayerName),
            "version_name" => Some(Self::VersionName),
            "game_directory" => Some(Self::GameDirectory),
            "assets_root" | "game_assets" => Some(Self::AssetsRoot),
            "assets_index_name" => Some(Self::AssetsIndexName),
            "auth_uuid" => Some(Self::AuthUuid),
            "auth_access_token" | "auth_session" => Some(Self::AuthAccessToken),
            "clientid" => Some(Self::Clientid),
            "auth_xuid" => Some(Self::AuthXuid),
            "version_type" => Some(Self::VersionType),
            "quickPlayPath" => Some(Self::QuickPlayPath),
            "user_properties" => Some(Self::UserProperties),
            "user_type" => Some(Self::UserType),
            "resolution_width" => Some(Self::ResolutionWidth),
            "resolution_height" => Some(Self::ResolutionHeight),
            "quickPlaySingleplayer" => Some(Self::QuickPlaySingleplayer),
            "quickPlayMultiplayer" => Some(Self::QuickPlayMultiplayer),
            "quickPlayRealms" => Some(Self::QuickPlayRealms),
            _ => None,
        }
    }
}

pub struct LaunchRuleContext {
    pub is_demo_user: bool,
    pub custom_resolution: Option<(u32, u32)>,
    pub quick_play: Option<QuickPlayLaunch>,
}

impl LaunchRuleContext {
    pub fn collect_libraries(
        &self,
        libraries: &[GameLibrary],
        artifacts: &mut Vec<GameLibraryArtifact>,
        natives_to_extract: &mut HashMap<Ustr, GameLibraryExtractOptions>,
    ) {
        let os_name = match std::env::consts::OS {
            "linux" => Some(OsName::Linux),
            "macos" => Some(OsName::Osx),
            "windows" => Some(OsName::Windows),
            _ => None,
        };

        // Remove duplicate libraries
        let mut deduplicated_libraries: HashMap<String, (GameLibrary, Vec<isize>)> = HashMap::new();
        for library in libraries {
            if let Some(rules) = &library.rules && !self.check_rules(rules) {
                continue;
            }

            let coordinate = MavenCoordinate::create(&library.name);

            let coordinate_id = if let Some(specifier) = coordinate.specifier {
                format!("{}:{}:{}", coordinate.group_id, coordinate.artifact_id, specifier)
            } else {
                format!("{}:{}", coordinate.group_id, coordinate.artifact_id)
            };

            let version_id = coordinate.version_id();
            if let Some((_, existing_library_version)) = deduplicated_libraries.get(&coordinate_id) {
                let mut ordering = Ordering::Equal;
                for (left, right) in version_id.iter().zip(existing_library_version.iter()) {
                    let cmp = left.cmp(right);
                    if cmp != Ordering::Equal {
                        ordering = cmp;
                        break;
                    }
                }
                if ordering == Ordering::Equal {
                    ordering = version_id.len().cmp(&existing_library_version.len());
                }
                if ordering == Ordering::Less {
                    continue;
                }
            }

            deduplicated_libraries.insert(coordinate_id, (library.clone(), version_id));
        }

        for library in deduplicated_libraries.into_values().map(|v| v.0) {
            if let Some(artifact) = &library.downloads.artifact {
                let empty = if let Some(artifact_size) = artifact.size && artifact_size <= 22 {
                    true
                } else {
                    false
                };
                if !empty {
                    artifacts.push(artifact.clone());
                }
            }

            if let Some(platform_natives) = &library.natives
                && let Some(classifiers) = &library.downloads.classifiers
                && let Some(os_name) = os_name
                && let Some(natives_id) = platform_natives.get(&os_name)
                && let Some(natives) = classifiers.get(natives_id)
            {
                artifacts.push(natives.clone());
                if let Some(extract) = &library.extract {
                    natives_to_extract.insert(natives.path, extract.clone());
                }
            }
        }
    }

    pub fn check_rules(&self, rules: &[Rule]) -> bool {
        let mut allowed = false;
        for rule in rules {
            if self.check_rule(rule) {
                allowed = match rule.action {
                    RuleAction::Allow => true,
                    RuleAction::Disallow => false,
                };
            }
        }
        allowed
    }

    pub fn check_rule(&self, rule: &Rule) -> bool {
        if let Some(features) = &rule.features {
            if features.is_demo_user && !self.is_demo_user {
                return false;
            }
            if features.has_custom_resolution && self.custom_resolution.is_none() {
                return false;
            }
            if features.has_quick_plays_support {
                // We use quick play, but we don't need the quick play file to
                // be generated by the client so we set this to false
                return false;
            }
            if features.is_quick_play_singleplayer && !matches!(self.quick_play, Some(QuickPlayLaunch::Singleplayer(_))) {
                return false;
            }
            if features.is_quick_play_multiplayer && !matches!(self.quick_play, Some(QuickPlayLaunch::Multiplayer(_))) {
                return false;
            }
            if features.is_quick_play_realms && !matches!(self.quick_play, Some(QuickPlayLaunch::Realms(_))) {
                return false;
            }
        }

        if let Some(os) = &rule.os {
            if let Some(name) = &os.name {
                let matches = match name {
                    OsName::Linux => std::env::consts::OS == "linux",
                    OsName::Osx => std::env::consts::OS == "macos",
                    OsName::Windows => std::env::consts::OS == "windows",
                };
                if !matches {
                    return false;
                }
            }
            if let Some(arch) = &os.arch {
                match arch {
                    OsArch::Arm64 => {
                        if std::env::consts::ARCH != "aarch64" {
                            return false;
                        }
                    },
                    OsArch::X86 => {
                        if std::env::consts::ARCH != "x86" {
                            return false;
                        }
                    },
                }
            }
            if let Some(version) = &os.version && let Ok(regex) = Regex::new(version.as_str()) {
                static OS_VERSION: OnceLock<String> = OnceLock::new();
                let os_version = OS_VERSION.get_or_init(|| format!("{}", os_info::get().version()));
                if !regex.is_match(os_version) {
                    return false;
                }
            }
        }

        true
    }
}

pub struct LaunchContext {
    pub launch_wrapper_path: Arc<Path>,
    pub java_path: PathBuf,
    pub natives_dir: PathBuf,
    pub libraries_dir: Arc<Path>,
    pub game_dir: Arc<Path>,
    pub configuration: InstanceConfiguration,
    pub assets_root: Arc<Path>,
    pub temp_dir: Arc<Path>,
    pub assets_index_name: String,
    pub classpath: Vec<OsString>,
    pub log_configuration: Option<OsString>,
    pub rule_context: LaunchRuleContext,
    pub login_info: MinecraftLoginInfo,
    pub add_mods: Vec<PathBuf>,
    pub global_memory_max: Option<u32>,
}

impl LaunchContext {
    pub fn launch(mut self, version_info: &MinecraftVersion) -> std::io::Result<std::process::Child> {
        #[cfg(target_os = "linux")]
        let use_mangohud = self.configuration.linux_wrapper.map(|w| w.use_mangohud).unwrap_or(false);
        #[cfg(target_os = "linux")]
        let use_gamemode = self.configuration.linux_wrapper.map(|w| w.use_gamemode).unwrap_or(false);

        #[cfg(target_os = "linux")]
        let mut command = match (use_mangohud, use_gamemode) {
            (true, true) => {
                let mut cmd = std::process::Command::new("mangohud");
                cmd.arg("gamemoderun");
                cmd.arg(&*self.java_path);
                cmd
            }
            (true, false) => {
                let mut cmd = std::process::Command::new("mangohud");
                cmd.arg(&*self.java_path);
                cmd
            }
            (false, true) => {
                let mut cmd = std::process::Command::new("gamemoderun");
                cmd.arg(&*self.java_path);
                cmd
            }
            (false, false) => {
                std::process::Command::new(&*self.java_path)
            }
        };

        #[cfg(not(target_os = "linux"))]
        let mut command = std::process::Command::new(&*self.java_path);

        command.current_dir(&self.game_dir);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        self.classpath.push(self.launch_wrapper_path.as_os_str().to_os_string());

        if let Some(arguments) = &version_info.arguments {
            self.process_arguments(&arguments.jvm, &mut |arg| {
                command.arg(arg);
            });
        } else {
            let mut java_library_path = OsString::new();
            java_library_path.push("-Djava.library.path=");
            java_library_path.push(self.natives_dir.as_os_str());

            command.arg(java_library_path);
            command.arg("-cp");
            command.arg(std::env::join_paths(&self.classpath).unwrap());
        }

        if let Some(log_configuration) = &self.log_configuration {
            command.arg(log_configuration);
        }

        if let Some(memory) = &self.configuration.memory && memory.enabled {
            command.arg(format!("-Xms{}m", memory.min));
            command.arg(format!("-Xmx{}m", memory.max.max(memory.min).max(128)));
        } else if let Some(global_max) = self.global_memory_max {
             command.arg(format!("-Xmx{}m", global_max.max(128)));
        }
        if let Some(jvm_flags) = &self.configuration.jvm_flags && jvm_flags.enabled {
            if let Ok(split) = shell_words::split(&jvm_flags.flags) {
                command.args(split);
            } else {
                command.args(jvm_flags.flags.split_whitespace());
            }
        }

        command.arg("com.moulberry.pandora.LaunchWrapper");

        let mut child = command.spawn()?;

        let mut stdin = child.stdin.take().expect("stdin present");

        let mut stdin_arguments = String::new();

        if let Some(arguments) = &version_info.arguments {
            self.process_arguments(&arguments.game, &mut |arg| {
                stdin_arguments.push_str("arg\n");
                stdin_arguments.push_str(arg.to_string_lossy().as_ref());
                stdin_arguments.push('\n');
            });
        }
        if let Some(legacy_arguments) = &version_info.minecraft_arguments {
            for argument in legacy_arguments.split_ascii_whitespace() {
                stdin_arguments.push_str("arg\n");
                stdin_arguments.push_str(self.expand_argument(argument).to_string_lossy().as_ref());
                stdin_arguments.push('\n');
            }
        }


        if !self.add_mods.is_empty() {
            match self.configuration.loader {
                Loader::Vanilla => {},
                Loader::Fabric => {
                    let mods = std::env::join_paths(self.add_mods).unwrap();

                    stdin_arguments.push_str("property\n");
                    stdin_arguments.push_str("fabric.addMods\n");
                    stdin_arguments.push_str(&mods.to_string_lossy());
                    stdin_arguments.push('\n');
                },
                _ => {
                    if cfg!(debug_assertions) {
                        panic!("addMods was used for unsupported loader: {:?}", self.configuration.loader);
                    }
                }
            }
        }

        stdin_arguments.push_str("launch\n");
        stdin_arguments.push_str(version_info.main_class.as_str());
        stdin_arguments.push('\n');

        stdin.write_all(stdin_arguments.as_bytes())?;
        stdin.flush()?;

        Ok(child)
    }

    fn process_arguments(&self, arguments: &[LaunchArgument], handler: &mut impl FnMut(&OsStr)) {
        for argument in arguments {
            match argument {
                LaunchArgument::Single(value) => {
                    self.process_argument(value, handler);
                },
                LaunchArgument::Ruled(ruled) => {
                    if self.rule_context.check_rules(&ruled.rules) {
                        self.process_argument(&ruled.value, handler);
                    }
                },
            }
        }
    }

    fn process_argument(&self, value: &LaunchArgumentValue, handler: &mut impl FnMut(&OsStr)) {
        match value {
            LaunchArgumentValue::Single(string) => {
                (handler)(&self.expand_argument(string));
            },
            LaunchArgumentValue::Multiple(strings) => {
                for string in strings.iter() {
                    (handler)(&self.expand_argument(string));
                }
            },
        }
    }

    fn expand_argument<'a>(&self, argument: &'a str) -> Cow<'a, OsStr> {
        let mut dollar_last = false;
        let mut builder = OsString::new();
        let mut copied_to_builder = 0;
        for (i, character) in argument.char_indices() {
            if character == '$' {
                dollar_last = true;
            } else if dollar_last && character == '{' {
                let remaining = &argument[i..];
                if let Some(end) = remaining.find('}') {
                    let to_expand = &argument[i+1..i+end];
                    if let Some(to_expand) = ArgumentExpansionKey::from_str(to_expand) {
                        let expanded = self.resolve_expansion(to_expand);
                        builder.push(&argument[copied_to_builder..i-1]);
                        builder.push(expanded);
                        copied_to_builder = i+end+1;
                    } else {
                        panic!("Unsupported argument: {:?}", to_expand);
                    }
                }
            } else {
                dollar_last = false;
            }
        }
        if !builder.is_empty() {
            builder.push(&argument[copied_to_builder..]);
            return Cow::Owned(builder);
        }
        Cow::Borrowed(OsStr::new(argument))
    }

    fn resolve_expansion(&self, key: ArgumentExpansionKey) -> Cow<'_, OsStr> {
        match key {
            ArgumentExpansionKey::NativesDirectory => self.natives_dir.as_os_str().into(),
            ArgumentExpansionKey::LibrariesDirectory => self.libraries_dir.as_os_str().into(),
            ArgumentExpansionKey::ClasspathSeparator => if cfg!(unix) {
                OsStr::new(":").into()
            } else if cfg!(windows) {
                OsStr::new(";").into()
            } else {
                panic!("Unsupported platform")
            },
            ArgumentExpansionKey::LauncherName => OsStr::new("PandoraLauncher").into(),
            ArgumentExpansionKey::LauncherVersion => OsStr::new("1.0.0").into(),
            ArgumentExpansionKey::Classpath => std::env::join_paths(&self.classpath).unwrap().into(),
            ArgumentExpansionKey::AuthPlayerName => OsStr::new(&*self.login_info.username).into(),
            ArgumentExpansionKey::VersionName => OsStr::new("1.21.10").into(),
            ArgumentExpansionKey::GameDirectory => self.game_dir.as_os_str().into(),
            ArgumentExpansionKey::AssetsRoot => self.assets_root.as_os_str().into(),
            ArgumentExpansionKey::AssetsIndexName => OsStr::new(&self.assets_index_name).into(),
            ArgumentExpansionKey::AuthUuid => OsString::from(self.login_info.uuid.as_hyphenated().to_string()).into(),
            ArgumentExpansionKey::AuthAccessToken => OsStr::new(if let Some(access_token) = &self.login_info.access_token {
                access_token.secret()
            } else {
                "offline"
            }).into(),
            ArgumentExpansionKey::Clientid => OsStr::new("").into(), // These are just used for telemetry
            ArgumentExpansionKey::AuthXuid => OsStr::new("").into(), // These are just used for telemetry
            ArgumentExpansionKey::VersionType => OsStr::new("release").into(),
            ArgumentExpansionKey::QuickPlayPath => OsStr::new("quickPlay/log.json").into(),
            ArgumentExpansionKey::UserProperties => OsStr::new("{}").into(),
            ArgumentExpansionKey::UserType => OsStr::new("msa").into(),
            ArgumentExpansionKey::ResolutionWidth => OsString::from(format!("{}", self.rule_context.custom_resolution.unwrap().0)).into(),
            ArgumentExpansionKey::ResolutionHeight => OsString::from(format!("{}", self.rule_context.custom_resolution.unwrap().1)).into(),
            ArgumentExpansionKey::QuickPlaySingleplayer => {
                if let Some(QuickPlayLaunch::Singleplayer(target)) = &self.rule_context.quick_play {
                    target.into()
                } else {
                    OsStr::new("").into()
                }
            },
            ArgumentExpansionKey::QuickPlayMultiplayer => {
                if let Some(QuickPlayLaunch::Multiplayer(target)) = &self.rule_context.quick_play {
                    target.into()
                } else {
                    OsStr::new("").into()
                }
            },
            ArgumentExpansionKey::QuickPlayRealms => {
                if let Some(QuickPlayLaunch::Realms(target)) = &self.rule_context.quick_play {
                    target.into()
                } else {
                    OsStr::new("").into()
                }
            },
        }
    }
}

fn path_is_normal(path: impl AsRef<Path>) -> bool {
    let components = path.as_ref().components();

    for component in components {
        match component {
            std::path::Component::Prefix(_) => return false,
            std::path::Component::RootDir => return false,
            std::path::Component::CurDir => return false,
            std::path::Component::ParentDir => return false,
            std::path::Component::Normal(_) => {},
        }
    }

    true
}

fn expand_forge_argument<'a>(argument: &'a str, map: &FxHashMap<String, OsString>) -> Cow<'a, OsStr> {
    let mut builder = OsString::new();
    let mut copied_to_builder = 0;
    for (i, character) in argument.char_indices() {
        if character == '{' {
            let remaining = &argument[i..];
            if let Some(end) = remaining.find('}') {
                let to_expand = &argument[i+1..i+end];
                if let Some(expanded) = map.get(to_expand) {
                    builder.push(&argument[copied_to_builder..i]);
                    builder.push(expanded);
                    copied_to_builder = i+end+1;
                } else {
                    panic!("Unsupported argument: {:?}", to_expand);
                }
            }
        }
    }
    if !builder.is_empty() {
        builder.push(&argument[copied_to_builder..]);
        return Cow::Owned(builder);
    }
    Cow::Borrowed(OsStr::new(argument))
}
