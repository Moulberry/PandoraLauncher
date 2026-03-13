use std::{ffi::{OsStr, OsString}, io::Write, path::{Path, PathBuf}, sync::Arc};

use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget}, instance::{ContentSummary, ContentType}, modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType}, safe_path::SafePath
};
use parking_lot::Mutex;
use reqwest::StatusCode;
use rustc_hash::FxHashSet;
use schema::{content::ContentSource, curseforge::{CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY, CurseforgeGetModFilesRequest, CurseforgeModLoaderType}, loader::Loader, modrinth::{ModrinthDependencyType, ModrinthLoader, ModrinthProjectVersionsRequest}};
use sha1::{Digest, Sha1};
use strum::IntoEnumIterator;
use tokio::sync::Semaphore;

use crate::{BackendState, instance::{ContentFolder, Instance}, lockfile::Lockfile, metadata::{items::{CurseforgeGetModFilesMetadataItem, MinecraftVersionManifestMetadataItem, ModrinthProjectVersionsMetadataItem, ModrinthVersionMetadataItem}, manager::MetaLoadError}};

#[derive(thiserror::Error, Debug)]
pub enum ContentInstallError {
    #[error("Unable to find appropriate version")]
    UnableToFindVersion,
    #[error("Unable to determine content type (mod, resourcepack, etc.) for file: {0}")]
    UnableToDetermineContentType(Arc<str>),
    #[error("Invalid filename: {0}")]
    InvalidFilename(Arc<str>),
    #[error("Failed to download remote content")]
    Reqwest(#[from] reqwest::Error),
    #[error("Remote server returned non-200 status code: {0}")]
    NotOK(StatusCode),
    #[error("Downloaded file had the wrong size")]
    WrongFilesize,
    #[error("Downloaded file had the wrong hash")]
    WrongHash,
    #[error("Missing required sha1 hash")]
    MissingHash,
    #[error("Hash isn't a valid sha1 hash:\n{0}")]
    InvalidHash(Arc<str>),
    #[error("Failed to perform I/O operation:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to load metadata:\n{0}")]
    MetaLoadError(#[from] MetaLoadError),
    #[error("Mismatched project id for version {0}, expected {1} got {2}")]
    MismatchedProjectIdForVersion(Arc<str>, Arc<str>, Arc<str>),
    #[error("No filename")]
    NoFilename,
    #[error("Content already installed")]
    ContentAlreadyInstalled,
    #[error("The mod author has blocked downloads from third-party launchers")]
    NoThirdPartyDownloads,
}

struct InstallFromContentLibrary {
    from: PathBuf,
    replace: Option<Arc<Path>>,
    hash: [u8; 20],
    install_path: Arc<Path>,
    content_file: ContentInstallFile,
    mod_summary: Option<Arc<ContentSummary>>,
    dependencies: Vec<ContentInstallFile>,
}

#[derive(Clone)]
struct FilenameAndExtension {
    filename: Option<OsString>,
    extension: Option<OsString>,
}

impl From<&SafePath> for FilenameAndExtension {
    fn from(value: &SafePath) -> Self {
        FilenameAndExtension {
            filename: value.file_name().map(OsString::from),
            extension: value.extension().map(OsString::from),
        }
    }
}

impl From<&Path> for FilenameAndExtension {
    fn from(value: &Path) -> Self {
        FilenameAndExtension {
            filename: value.file_name().map(OsString::from),
            extension: value.extension().map(OsString::from),
        }
    }
}

#[derive(Default)]
struct InstalledContentIds {
    modrinth_projects: FxHashSet<Arc<str>>,
    curseforge_projects: FxHashSet<u32>,
    summary_ids: FxHashSet<Arc<str>>,
}

impl BackendState {
    pub async fn install_content(self: &Arc<Self>, content: ContentInstall, modal_action: ModalAction) {
        let needs_installed_content_ids = content.files.iter().any(|content_file| {
            match content_file.download {
                ContentDownload::Modrinth { install_dependencies, .. } | ContentDownload::Curseforge { install_dependencies, .. } => {
                    if install_dependencies {
                        return true;
                    }
                }
                _ => {}
            }
            false
        });
        let installed_content_ids: Option<Mutex<InstalledContentIds>> = if needs_installed_content_ids {
            let mut installed_content_ids = InstalledContentIds::default();

            if let InstallTarget::Instance(instance) = content.target {
                let content_futures = ContentFolder::iter().map(|folder| {
                    Instance::load_content(self.clone(), instance, folder)
                });
                let content_summaries = futures::future::join_all(content_futures).await;
                for summaries in content_summaries {
                    if let Some(summaries) = summaries {
                        for summary in summaries.iter() {
                            match summary.content_source {
                                ContentSource::ModrinthProject { ref project_id } => {
                                    installed_content_ids.modrinth_projects.insert(project_id.clone());
                                },
                                ContentSource::CurseforgeProject { project_id } => {
                                    installed_content_ids.curseforge_projects.insert(project_id);
                                },
                                _ => {}
                            }
                            if let Some(id) = &summary.content_summary.id {
                                installed_content_ids.summary_ids.insert(id.clone());
                            }
                        }
                    }
                }
            }

            Some(Mutex::new(installed_content_ids))
        } else {
            None
        };

        let semaphore = tokio::sync::Semaphore::new(8);
        let mut tasks = Vec::new();

        for content_file in content.files.iter() {
            tasks.push(self.install_into_content_library(&content, &modal_action, content_file, &semaphore, installed_content_ids.as_ref(), false));
        }

        let result: Result<Vec<InstallFromContentLibrary>, ContentInstallError> = futures::future::try_join_all(tasks).await;

        let mut files = match result {
            Ok(files) => files,
            Err(error) => {
                modal_action.set_error_message(Arc::from(format!("{}", error).as_str()));
                return;
            }
        };

        let mut dependencies = Vec::new();
        for file in &mut files {
            dependencies.extend(std::mem::take(&mut file.dependencies));
        }

        while !dependencies.is_empty() {
            let mut new_tasks = Vec::new();

            for dependency in &dependencies {
                new_tasks.push(self.install_into_content_library(&content, &modal_action, dependency, &semaphore, installed_content_ids.as_ref(), true));
            }

            let new_results = futures::future::join_all(new_tasks).await;

            dependencies.clear();

            for result in new_results {
                let Ok(mut file) = result else {
                    continue;
                };
                dependencies.extend(std::mem::take(&mut file.dependencies));
                files.push(file);
            }
        }

        let mut instance_dir = None;

        let mut loader_hint = content.loader_hint;
        let mut version_hint = content.version_hint;

        match content.target {
            bridge::install::InstallTarget::Instance(instance_id) => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(instance_id) {
                    if instance.configuration.get().loader == Loader::Vanilla {
                        if loader_hint == Loader::Unknown {
                            loader_hint = determine_loader_from_content(&files).unwrap_or(Loader::Unknown);
                        }
                        if loader_hint != Loader::Unknown {
                            instance.configuration.modify(|config| {
                                config.loader = content.loader_hint;
                            });
                        }
                    }

                    instance_dir = Some(instance.dot_minecraft_path.clone());
                }
            },
            bridge::install::InstallTarget::Library => {},
            bridge::install::InstallTarget::NewInstance { name } => {
                if version_hint.is_none() {
                    version_hint = determine_minecraft_version_from_content(&files);
                }
                if version_hint.is_none() {
                    if let Ok(meta) = self.meta.fetch(&MinecraftVersionManifestMetadataItem).await {
                        version_hint = Some(meta.latest.release.into());
                    }
                }

                if let Some(version_hint) = version_hint {
                    if loader_hint == Loader::Unknown {
                        loader_hint = determine_loader_from_content(&files).unwrap_or(Loader::Unknown);
                    }

                    let mut name = name;
                    if name.is_none() {
                        name = determine_name_from_content(&files);
                    }
                    let name = name.as_deref().unwrap_or("New Instance");

                    // todo: use icon of mod/modpack/etc. for icon of instance
                    instance_dir = self.create_instance_sanitized(&name, &version_hint, loader_hint, None).await
                        .map(|v| v.join(".minecraft").into());
                }
            },
        }

        let sources = files.iter()
            .filter_map(|install| {
                if install.content_file.content_source != ContentSource::Manual {
                    Some((install.hash.clone(), install.content_file.content_source.clone()))
                } else {
                    None
                }
            });
        self.mod_metadata_manager.set_content_sources(sources);

        if let Some(instance_dir) = instance_dir {
            for install in files {
                let target_path = instance_dir.join(&install.install_path);

                let _ = std::fs::create_dir_all(target_path.parent().unwrap());

                if let Some(replace) = install.replace {
                    self.replace_aux_path(&replace, &install.mod_summary, &target_path);
                    let _ = std::fs::remove_file(replace);
                }
                let _ = std::fs::hard_link(install.from, target_path);
            }
        }
    }

    async fn install_into_content_library(
        &self,
        content: &ContentInstall,
        modal_action: &ModalAction,
        content_file: &ContentInstallFile,
        semaphore: &Semaphore,
        installed_content_ids: Option<&Mutex<InstalledContentIds>>,
        skip_if_already_installed: bool,
    ) -> Result<InstallFromContentLibrary, ContentInstallError> {
        let content_install_file = match content_file.download {
            ContentDownload::Modrinth { ref project_id, ref version_id, install_dependencies } => {
                if let Some(installed_content_ids) = installed_content_ids {
                    let unique = installed_content_ids.lock().modrinth_projects.insert(project_id.clone());
                    if skip_if_already_installed && !unique {
                        return Err(ContentInstallError::ContentAlreadyInstalled);
                    }
                }

                let permit = semaphore.acquire().await;

                let title = format!("Fetching versions for Modrinth project {}", project_id);
                let tracker = ProgressTracker::new(title.into(), self.send.clone());
                tracker.add_total(1);
                modal_action.trackers.push(tracker.clone());

                let mut is_wrong_version = false;
                let mut is_wrong_loader = false;

                let version = if let Some(version_id) = version_id {
                    let version = self.meta.fetch(&ModrinthVersionMetadataItem(version_id.clone())).await?;

                    tracker.add_count(1);
                    tracker.set_finished(ProgressTrackerFinishType::Normal);
                    drop(tracker);

                    Some(version)
                } else {
                    let modrinth_loader = content.loader_hint.as_modrinth_loader();
                    let loaders = if modrinth_loader != ModrinthLoader::Unknown {
                        Some(Arc::from([modrinth_loader]))
                    } else {
                        None
                    };

                    let mut result = self.meta.fetch(&ModrinthProjectVersionsMetadataItem(&ModrinthProjectVersionsRequest {
                        project_id: project_id.clone(),
                        game_versions: content.version_hint.clone().map(|v| [v].into()),
                        loaders,
                    })).await;

                    tracker.add_count(1);

                    let mut not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                        result.as_ref().ok().map(|r| r.0.is_empty()).unwrap_or(false);
                    if not_found && modrinth_loader != ModrinthLoader::Unknown {
                        tracker.add_total(1);

                        result = self.meta.fetch(&ModrinthProjectVersionsMetadataItem(&ModrinthProjectVersionsRequest {
                            project_id: project_id.clone(),
                            game_versions: content.version_hint.clone().map(|v| [v].into()),
                            loaders: None,
                        })).await;
                        not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                            result.as_ref().ok().map(|r| r.0.is_empty()).unwrap_or(false);
                        is_wrong_loader = true;

                        tracker.add_count(1);
                    }
                    if not_found && content.version_hint.is_some() {
                        tracker.add_total(1);

                        result = self.meta.fetch(&ModrinthProjectVersionsMetadataItem(&ModrinthProjectVersionsRequest {
                            project_id: project_id.clone(),
                            game_versions: None,
                            loaders: None,
                        })).await;
                        not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                            result.as_ref().ok().map(|r| r.0.is_empty()).unwrap_or(false);
                        is_wrong_loader = true;
                        is_wrong_version = true;

                        tracker.add_count(1);
                    }

                    tracker.set_finished(ProgressTrackerFinishType::from_err(not_found || result.is_err()));
                    drop(tracker);

                    result?.0.first().map(|v| Arc::new(v.clone()))
                };


                drop(permit);

                let Some(version) = version else {
                    return Err(ContentInstallError::UnableToFindVersion);
                };

                if &version.project_id != project_id {
                    return Err(ContentInstallError::MismatchedProjectIdForVersion(
                        version.id.clone(),
                        project_id.clone(),
                        version.project_id.clone()
                    ));
                }

                let install_file = version
                    .files
                    .iter()
                    .find(|file| file.primary)
                    .unwrap_or(version.files.first().unwrap());

                let url = &install_file.url;
                let sha1 = &install_file.hashes.sha1;
                let size = install_file.size;

                let Some(safe_filename) = SafePath::new(&install_file.filename) else {
                    return Err(ContentInstallError::InvalidFilename(install_file.filename.clone()));
                };

                let (path, hash, mod_summary) = self.download_file_into_library(&modal_action,
                    (&safe_filename).into(), url, sha1, size, &semaphore).await?;

                if is_wrong_version {
                    let is_strict_minecraft_version = if let Some(mod_summary) = &mod_summary {
                        mod_summary.extra.is_strict_minecraft_version()
                    } else {
                        true
                    };
                    if is_strict_minecraft_version {
                        return Err(ContentInstallError::UnableToFindVersion);
                    }
                }
                if is_wrong_loader {
                    let is_strict_loader = if let Some(mod_summary) = &mod_summary {
                        mod_summary.extra.is_strict_loader()
                    } else {
                        true
                    };
                    if is_strict_loader {
                        return Err(ContentInstallError::UnableToFindVersion);
                    }
                }

                let install_path = match &content_file.path {
                    ContentInstallPath::Raw(path) => path.clone(),
                    ContentInstallPath::Safe(safe_path) => safe_path.to_path(Path::new("")).into(),
                    ContentInstallPath::Automatic => {
                        let base = if let Some(mod_summary) = &mod_summary {
                            match mod_summary.extra {
                                ContentType::Fabric | ContentType::Forge | ContentType::LegacyForge | ContentType::NeoForge | ContentType::JavaModule | ContentType::ModrinthModpack { .. } => {
                                    Path::new("mods")
                                },
                                ContentType::ResourcePack => {
                                    Path::new("resourcepacks")
                                }
                            }
                        } else if let Some(loaders) = &version.loaders {
                            let mut base = None;
                            for loader in loaders.iter() {
                                base = loader.install_directory();
                                if base.is_some() {
                                    break;
                                }
                            }
                            if let Some(base) = base {
                                Path::new(base)
                            } else {
                                return Err(ContentInstallError::UnableToDetermineContentType(install_file.filename.clone()))
                            }
                        } else {
                            return Err(ContentInstallError::UnableToDetermineContentType(install_file.filename.clone()))
                        };

                        safe_filename.to_path(base).into()
                    },
                };

                let dependencies = if install_dependencies {
                    if let Some(dependencies) = &version.dependencies {
                        dependencies.iter().filter_map(|dep| {
                            if let Some(project_id) = &dep.project_id && dep.dependency_type == ModrinthDependencyType::Required {
                                Some(ContentInstallFile {
                                    replace_old: None,
                                    path: ContentInstallPath::Automatic,
                                    download: ContentDownload::Modrinth {
                                        project_id: project_id.clone(),
                                        version_id: dep.version_id.clone(),
                                        install_dependencies: true
                                    },
                                    content_source: ContentSource::ModrinthProject { project_id: project_id.clone() }
                                })
                            } else {
                                None
                            }
                        }).collect()
                    } else {
                        Default::default()
                    }
                } else {
                    Default::default()
                };

                InstallFromContentLibrary {
                    from: path,
                    replace: content_file.replace_old.clone(),
                    hash,
                    install_path,
                    content_file: content_file.clone(),
                    mod_summary,
                    dependencies,
                }
            },
            ContentDownload::Curseforge { project_id, install_dependencies } => {
                if let Some(installed_content_ids) = installed_content_ids {
                    let unique = installed_content_ids.lock().curseforge_projects.insert(project_id);
                    if skip_if_already_installed && !unique {
                        return Err(ContentInstallError::ContentAlreadyInstalled);
                    }
                }

                let permit = semaphore.acquire().await;

                let title = format!("Fetching versions for Curseforge project {}", project_id);
                let tracker = ProgressTracker::new(title.into(), self.send.clone());
                tracker.add_total(1);
                modal_action.trackers.push(tracker.clone());

                let mod_loader_type = match content.loader_hint {
                    Loader::Vanilla => None,
                    Loader::Fabric => Some(CurseforgeModLoaderType::Fabric as u32),
                    Loader::Forge => Some(CurseforgeModLoaderType::Forge as u32),
                    Loader::NeoForge => Some(CurseforgeModLoaderType::NeoForge as u32),
                    Loader::Unknown => None,
                };

                let mut is_wrong_version = false;
                let mut is_wrong_loader = false;

                let mut result = self.meta.fetch(&CurseforgeGetModFilesMetadataItem(&CurseforgeGetModFilesRequest {
                    mod_id: project_id,
                    game_version: content.version_hint.clone().map(|v| v.into()),
                    mod_loader_type,
                    page_size: Some(1)
                })).await;

                tracker.add_count(1);

                let mut not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                    result.as_ref().ok().map(|r| r.data.is_empty()).unwrap_or(false);
                if not_found && mod_loader_type.is_some() {
                    tracker.add_total(1);

                    result = self.meta.fetch(&CurseforgeGetModFilesMetadataItem(&CurseforgeGetModFilesRequest {
                        mod_id: project_id,
                        game_version: content.version_hint.clone().map(|v| v.into()),
                        mod_loader_type: None,
                        page_size: Some(1)
                    })).await;
                    not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                        result.as_ref().ok().map(|r| r.data.is_empty()).unwrap_or(false);
                    is_wrong_loader = true;

                    tracker.add_count(1);
                }
                if not_found && content.version_hint.is_some() {
                    tracker.add_total(1);

                    result = self.meta.fetch(&CurseforgeGetModFilesMetadataItem(&CurseforgeGetModFilesRequest {
                        mod_id: project_id,
                        game_version: None,
                        mod_loader_type: None,
                        page_size: Some(1)
                    })).await;
                    not_found = matches!(result, Err(MetaLoadError::NonOK(404))) ||
                        result.as_ref().ok().map(|r| r.data.is_empty()).unwrap_or(false);
                    is_wrong_loader = true;
                    is_wrong_version = true;

                    tracker.add_count(1);
                }

                tracker.set_finished(ProgressTrackerFinishType::from_err(not_found || result.is_err()));
                drop(tracker);

                drop(permit);

                let versions = result?;
                let Some(file) = versions.data.first() else {
                    return Err(ContentInstallError::UnableToFindVersion);
                };

                if file.mod_id != project_id {
                    return Err(ContentInstallError::MismatchedProjectIdForVersion(
                        file.file_name.clone(),
                        format!("{}", project_id.clone()).into(),
                        format!("{}", file.mod_id).into()
                    ));
                }

                let sha1 = file.hashes.iter()
                    .find(|hash| hash.algo == 1).map(|hash| &hash.value);
                let size = file.file_length as usize;

                let Some(url) = file.download_url.as_ref() else {
                    return Err(ContentInstallError::NoThirdPartyDownloads);
                };

                let Some(sha1) = sha1 else {
                    return Err(ContentInstallError::MissingHash);
                };

                let Some(safe_filename) = SafePath::new(&file.file_name) else {
                    return Err(ContentInstallError::InvalidFilename(file.file_name.clone()));
                };

                let (path, hash, mod_summary) = self.download_file_into_library(&modal_action,
                    (&safe_filename).into(), url, sha1, size, &semaphore).await?;

                if is_wrong_version {
                    let is_strict_minecraft_version = if let Some(mod_summary) = &mod_summary {
                        mod_summary.extra.is_strict_minecraft_version()
                    } else {
                        true
                    };
                    if is_strict_minecraft_version {
                        return Err(ContentInstallError::UnableToFindVersion);
                    }
                }
                if is_wrong_loader {
                    let is_strict_loader = if let Some(mod_summary) = &mod_summary {
                        mod_summary.extra.is_strict_loader()
                    } else {
                        true
                    };
                    if is_strict_loader {
                        return Err(ContentInstallError::UnableToFindVersion);
                    }
                }

                let install_path = match &content_file.path {
                    ContentInstallPath::Raw(path) => path.clone(),
                    ContentInstallPath::Safe(safe_path) => safe_path.to_path(Path::new("")).into(),
                    ContentInstallPath::Automatic => {
                        let base = if let Some(mod_summary) = &mod_summary {
                            match mod_summary.extra {
                                ContentType::Fabric | ContentType::Forge | ContentType::LegacyForge | ContentType::NeoForge | ContentType::JavaModule | ContentType::ModrinthModpack { .. } => {
                                    Path::new("mods")
                                },
                                ContentType::ResourcePack => {
                                    Path::new("resourcepacks")
                                }
                            }
                        } else {
                            return Err(ContentInstallError::UnableToDetermineContentType(file.file_name.clone()))
                        };

                        safe_filename.to_path(base).into()
                    },
                };

                let dependencies = if install_dependencies {
                    file.dependencies.iter().filter_map(|dep| {
                        if dep.relation_type == CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY {
                            Some(ContentInstallFile {
                                replace_old: None,
                                path: ContentInstallPath::Automatic,
                                download: ContentDownload::Curseforge {
                                    project_id: dep.mod_id,
                                    install_dependencies: true
                                },
                                content_source: ContentSource::CurseforgeProject { project_id }
                            })
                        } else {
                            None
                        }
                    }).collect()
                } else {
                    Default::default()
                };

                InstallFromContentLibrary {
                    from: path,
                    replace: content_file.replace_old.clone(),
                    hash,
                    install_path,
                    content_file: content_file.clone(),
                    mod_summary,
                    dependencies,
                }
            },
            ContentDownload::Url { ref url, ref sha1, size } => {
                let name = match &content_file.path {
                    ContentInstallPath::Raw(path) => (&**path).into(),
                    ContentInstallPath::Safe(safe_path) => safe_path.into(),
                    ContentInstallPath::Automatic => unimplemented!(),
                };

                let (path, hash, mod_summary) = self.download_file_into_library(&modal_action,
                    name, url, sha1, size, &semaphore).await?;

                let install_path = match &content_file.path {
                    ContentInstallPath::Raw(path) => path.clone(),
                    ContentInstallPath::Safe(safe_path) => safe_path.to_path(Path::new("")).into(),
                    ContentInstallPath::Automatic => unimplemented!(),
                };

                InstallFromContentLibrary {
                    from: path,
                    replace: content_file.replace_old.clone(),
                    hash,
                    install_path,
                    content_file: content_file.clone(),
                    mod_summary,
                    dependencies: Default::default(),
                }
            },
            ContentDownload::File { path: ref copy_path } => {
                let title = format!("Copying {}", copy_path.file_name().unwrap().to_string_lossy());
                let tracker = ProgressTracker::new(title.into(), self.send.clone());
                modal_action.trackers.push(tracker.clone());

                tracker.set_total(3);
                tracker.notify();

                let data = tokio::fs::read(copy_path).await?;

                tracker.set_count(1);
                tracker.notify();

                let mut hasher = Sha1::new();
                hasher.update(&data);
                let hash: [u8; 20] = hasher.finalize().into();

                let hash_as_str = hex::encode(hash);

                let hash_folder = self.directories.content_library_dir.join(&hash_as_str[..2]);
                let _ = tokio::fs::create_dir_all(&hash_folder).await;
                let mut path = hash_folder.join(hash_as_str);

                let extension = match &content_file.path {
                    ContentInstallPath::Raw(path) => path.extension(),
                    ContentInstallPath::Safe(safe_path) => safe_path.extension().map(OsStr::new),
                    ContentInstallPath::Automatic => copy_path.extension(),
                };

                if let Some(extension) = extension {
                    path.set_extension(extension);
                }

                let mod_summary = {
                    let path = path.clone();
                    let mod_metadata_manager = self.mod_metadata_manager.clone();
                    let tracker = tracker.clone();
                    tokio::task::spawn_blocking(move || {
                        let valid_hash_on_disk = crate::check_sha1_hash(&path, hash).unwrap_or(false);

                        tracker.set_count(2);
                        tracker.notify();

                        if !valid_hash_on_disk {
                            std::fs::write(&path, &data)?;
                        }

                        std::io::Result::Ok(mod_metadata_manager.get_bytes(&data))
                    }).await.unwrap()?
                };

                tracker.set_count(3);
                tracker.notify();

                let install_path = match &content_file.path {
                    ContentInstallPath::Raw(path) => path.clone(),
                    ContentInstallPath::Safe(safe_path) => safe_path.to_path(Path::new("")).into(),
                    ContentInstallPath::Automatic => {
                        let Some(file_name) = copy_path.file_name() else {
                            return Err(ContentInstallError::NoFilename);
                        };

                        let base = if let Some(mod_summary) = &mod_summary {
                            match mod_summary.extra {
                                ContentType::Fabric | ContentType::Forge | ContentType::LegacyForge | ContentType::NeoForge | ContentType::JavaModule | ContentType::ModrinthModpack { .. } => {
                                    Path::new("mods")
                                },
                                ContentType::ResourcePack => {
                                    Path::new("resourcepacks")
                                }
                            }
                        } else {
                            return Err(ContentInstallError::UnableToDetermineContentType(file_name.to_string_lossy().into()))
                        };

                        base.join(file_name).into()
                    },
                };

                InstallFromContentLibrary {
                    from: path,
                    replace: content_file.replace_old.clone(),
                    hash: hash.into(),
                    install_path,
                    content_file: content_file.clone(),
                    mod_summary,
                    dependencies: Default::default(),
                }
            },
        };

        if let Some(installed_content_ids) = installed_content_ids &&
            let Some(mod_summary) = &content_install_file.mod_summary &&
            let Some(id) = &mod_summary.id
        {
            let unique = installed_content_ids.lock().summary_ids.insert(id.clone());
            if skip_if_already_installed && !unique {
                return Err(ContentInstallError::ContentAlreadyInstalled);
            }
        }

        Ok(content_install_file)
    }

    fn replace_aux_path(&self, replace: &Path, new_summary: &Option<Arc<ContentSummary>>, new_path: &Path) {
        let Some(old_summary) = self.mod_metadata_manager.get_path(&replace) else {
            return;
        };

        let Some(new_summary) = new_summary else {
            return;
        };

        let Some(old_aux_path) = crate::pandora_aux_path(&old_summary.id, &old_summary.name, &replace) else {
            return;
        };

        if !old_aux_path.exists() {
            return;
        }

        let Some(new_aux_path) = crate::pandora_aux_path(&new_summary.id, &new_summary.name, new_path) else {
            return;
        };

        if old_aux_path != new_aux_path {
            _ = std::fs::rename(&old_aux_path, &new_aux_path);
        }
    }

    async fn download_file_into_library(&self, modal_action: &ModalAction, name: FilenameAndExtension, url: &Arc<str>, sha1: &Arc<str>, size: usize, semaphore: &tokio::sync::Semaphore) -> Result<(PathBuf, [u8; 20], Option<Arc<ContentSummary>>), ContentInstallError> {
        let mut result = self.download_file_into_library_inner(modal_action, name, url, sha1, size, semaphore).await?;

        if let Some(summary) = &result.2 {
            if let ContentType::ModrinthModpack { downloads, .. } = &summary.extra {
                let mut tasks = Vec::new();

                for download in downloads.iter() {
                    let Some(path) = SafePath::new(&download.path) else {
                        continue;
                    };

                    let name = FilenameAndExtension {
                        filename: path.file_name().map(OsString::from),
                        extension: path.extension().map(OsString::from),
                    };

                    tasks.push(self.download_file_into_library_inner(modal_action, name,
                        &download.downloads[0], &download.hashes.sha1, download.file_size, semaphore));
                }

                _ = futures::future::try_join_all(tasks).await;
            }
            result.2 = self.mod_metadata_manager.get_path(&result.0);
        }

        Ok(result)
    }

    async fn download_file_into_library_inner(&self, modal_action: &ModalAction, name: FilenameAndExtension, url: &Arc<str>, sha1: &Arc<str>, size: usize, semaphore: &tokio::sync::Semaphore) -> Result<(PathBuf, [u8; 20], Option<Arc<ContentSummary>>), ContentInstallError> {
        let mut expected_hash = [0u8; 20];
        let Ok(_) = hex::decode_to_slice(&**sha1, &mut expected_hash) else {
            log::warn!("Content install has invalid sha1: {}", sha1);
            return Err(ContentInstallError::InvalidHash(sha1.clone()));
        };

        // Re-encode as hex just in case the given sha1 was uppercase
        let hash_as_str = hex::encode(expected_hash);

        let hash_folder = self.directories.content_library_dir.join(&hash_as_str[..2]);
        let _ = tokio::fs::create_dir_all(&hash_folder).await;
        let mut path = hash_folder.join(hash_as_str);

        if let Some(extension) = name.extension {
            path.set_extension(extension);
        }

        let _permit = semaphore.acquire().await.unwrap();

        let lockfile = Lockfile::create(path.with_added_extension("lock").into()).await;

        let file_name = name.filename.clone();

        let title = format!("Downloading {}", file_name.as_deref().map(|s| s.to_string_lossy()).unwrap_or(std::borrow::Cow::Borrowed("???")));
        let tracker = ProgressTracker::new(title.into(), self.send.clone());
        modal_action.trackers.push(tracker.clone());

        tracker.set_total(size);
        tracker.notify();

        let valid_hash_on_disk = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                crate::check_sha1_hash(&path, expected_hash).unwrap_or(false)
            }).await.unwrap()
        };

        if valid_hash_on_disk {
            tracker.set_count(size);
            tracker.set_finished(ProgressTrackerFinishType::Normal);
            tracker.notify();
            let summary = self.mod_metadata_manager.get_path(&path);
            return Ok((path, expected_hash, summary));
        }

        let response = self.redirecting_http_client.get(&**url).send().await?;

        if response.status() != StatusCode::OK {
            return Err(ContentInstallError::NotOK(response.status()));
        }

        let mut file = std::fs::File::create(&path)?;

        use futures::StreamExt;
        let mut stream = response.bytes_stream();

        let mut total_bytes = 0;

        let mut hasher = Sha1::new();
        while let Some(item) = stream.next().await {
            let item = item?;

            total_bytes += item.len();
            tracker.add_count(item.len());
            tracker.notify();

            hasher.write_all(&item)?;
            file.write_all(&item)?;
        }

        tracker.set_finished(ProgressTrackerFinishType::Normal);

        let actual_hash = hasher.finalize();

        let wrong_hash = *actual_hash != expected_hash;
        let wrong_size = total_bytes != size;

        if wrong_hash || wrong_size {
            let _ = file.set_len(0);
            drop(file);
            let _ = std::fs::remove_file(&path);

            if wrong_hash {
                return Err(ContentInstallError::WrongHash);
            } else if wrong_size {
                return Err(ContentInstallError::WrongFilesize);
            } else {
                unreachable!();
            }
        }

        drop(lockfile);

        let summary = self.mod_metadata_manager.get_path(&path);
        Ok((path, expected_hash, summary))
    }
}

fn determine_loader_from_content(content: &[InstallFromContentLibrary]) -> Option<Loader> {
    for content in content {
        if let Some(summary) = &content.mod_summary {
            match &summary.extra {
                ContentType::Fabric => return Some(Loader::Fabric),
                ContentType::LegacyForge => return Some(Loader::Forge),
                ContentType::Forge => return Some(Loader::Forge),
                ContentType::NeoForge => return Some(Loader::NeoForge),
                ContentType::JavaModule => {},
                ContentType::ModrinthModpack { dependencies, .. } => {
                    for (key, _) in dependencies {
                        match &**key {
                            "forge" => return Some(Loader::Forge),
                            "neoforge" => return Some(Loader::NeoForge),
                            "fabric-loader" => return Some(Loader::Fabric),
                            _ => {}
                        }
                    }
                },
                ContentType::ResourcePack => {},
            }
        }
    }
    None
}

fn determine_minecraft_version_from_content(content: &[InstallFromContentLibrary]) -> Option<Arc<str>> {
    for content in content {
        if let Some(summary) = &content.mod_summary {
            match &summary.extra {
                ContentType::Fabric => {},
                ContentType::LegacyForge => {},
                ContentType::Forge => {},
                ContentType::NeoForge => {},
                ContentType::JavaModule => {},
                ContentType::ModrinthModpack { dependencies, .. } => {
                    for (key, value) in dependencies {
                        if &**key == "minecraft" {
                            return Some(value.clone());
                        }
                    }
                },
                ContentType::ResourcePack => {},
            }
        }
    }
    None
}

fn determine_name_from_content(content: &[InstallFromContentLibrary]) -> Option<Arc<str>> {
    for content in content {
        if let Some(summary) = &content.mod_summary {
            if let Some(name) = &summary.name {
                return Some(name.clone());
            }
        }
    }
    None
}
