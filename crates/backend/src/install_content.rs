use std::{ffi::{OsStr, OsString}, io::Write, path::{Path, PathBuf}, sync::Arc};

use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath}, instance::{ContentType, ContentSummary}, modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType}, safe_path::SafePath
};
use reqwest::StatusCode;
use schema::{content::ContentSource, loader::Loader, modrinth::{ModrinthLoader, ModrinthProjectVersionsRequest}};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::{lockfile::Lockfile, metadata::{items::{MinecraftVersionManifestMetadataItem, ModrinthProjectVersionsMetadataItem, ModrinthVersionMetadataItem}, manager::MetaLoadError}, BackendState};

#[derive(thiserror::Error, Debug)]
pub enum ContentInstallError {
    #[error("Unable to find appropriate version for dependency")]
    UnableToFindDependencyVersion,
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
    #[error("Hash isn't a valid sha1 hash:\n{0}")]
    InvalidHash(Arc<str>),
    #[error("Failed to perform I/O operation:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to load metadata:\n{0}")]
    MetaLoadError(#[from] MetaLoadError),
    #[error("Mismatched project id for version {0}, expected {1} got {2}")]
    MismatchedProjectIdForVersion(Arc<str>, Arc<str>, Arc<str>),
}

struct InstallFromContentLibrary {
    from: PathBuf,
    replace: Option<Arc<Path>>,
    hash: [u8; 20],
    install_path: Arc<Path>,
    content_file: ContentInstallFile,
    mod_summary: Option<Arc<ContentSummary>>,
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

impl BackendState {
    pub async fn install_content(&self, content: ContentInstall, modal_action: ModalAction) {
        let semaphore = tokio::sync::Semaphore::new(8);

        let mut tasks = Vec::new();

        for content_file in content.files.iter() {
            tasks.push(async {
                match content_file.download {
                    bridge::install::ContentDownload::Modrinth { ref project_id, ref version_id } => {
                        let version = if let Some(version_id) = version_id {
                            let version = self.meta.fetch(&ModrinthVersionMetadataItem(version_id.clone())).await?;
                            Some(version)
                        } else {
                            let versions = self.meta.fetch(&ModrinthProjectVersionsMetadataItem(&ModrinthProjectVersionsRequest {
                                project_id: project_id.clone(),
                                game_versions: content.version_hint.clone().map(|v| [v].into()),
                                loaders: None,
                            })).await?;

                            let modrinth_loader = content.loader_hint.as_modrinth_loader();
                            let version = if modrinth_loader != ModrinthLoader::Unknown {
                                versions.0.iter()
                                    .find(|version| if let Some(loaders) = &version.loaders {
                                        loaders.contains(&modrinth_loader)
                                    } else {
                                        false
                                    })
                                    .or(versions.0.first())
                            } else {
                                versions.0.first()
                            };

                            version.map(|v| Arc::new(v.clone()))
                        };

                        if let Some(version) = version {
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

                            let install_path = match &content_file.path {
                                ContentInstallPath::Raw(path) => path.clone(),
                                ContentInstallPath::Safe(safe_path) => safe_path.to_path(Path::new("")).into(),
                                ContentInstallPath::Automatic => {
                                    let base = if let Some(mod_summary) = &mod_summary {
                                        match mod_summary.extra {
                                            ContentType::Fabric | ContentType::Forge | ContentType::NeoForge | ContentType::JavaModule | ContentType::ModrinthModpack { .. } => {
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

                            Ok(InstallFromContentLibrary {
                                from: path,
                                replace: content_file.replace_old.clone(),
                                hash,
                                install_path,
                                content_file: content_file.clone(),
                                mod_summary
                            })
                        } else {
                            Err(ContentInstallError::UnableToFindDependencyVersion)
                        }
                    },
                    bridge::install::ContentDownload::Url { ref url, ref sha1, size } => {
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

                        return Ok(InstallFromContentLibrary {
                            from: path,
                            replace: content_file.replace_old.clone(),
                            hash,
                            install_path,
                            content_file: content_file.clone(),
                            mod_summary
                        });
                    },
                    bridge::install::ContentDownload::File { path: ref copy_path } => {
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
                            ContentInstallPath::Automatic => unimplemented!(),
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
                            ContentInstallPath::Automatic => unimplemented!(),
                        };

                        return Ok(InstallFromContentLibrary {
                            from: path,
                            replace: content_file.replace_old.clone(),
                            hash: hash.into(),
                            install_path,
                            content_file: content_file.clone(),
                            mod_summary,
                        });
                    },
                }
            });
        }

        let result: Result<Vec<InstallFromContentLibrary>, ContentInstallError> = futures::future::try_join_all(tasks).await;
        match result {
            Ok(files) => {
                let mut instance_dir = None;

                match content.target {
                    bridge::install::InstallTarget::Instance(instance_id) => {
                        if let Some(instance) = self.instance_state.write().instances.get_mut(instance_id) {
                            if instance.configuration.get().loader == Loader::Vanilla && content.loader_hint != Loader::Unknown {
                                instance.configuration.modify(|config| {
                                    config.loader = content.loader_hint;
                                });
                            }

                            instance_dir = Some(instance.dot_minecraft_path.clone());
                        }
                    },
                    bridge::install::InstallTarget::Library => {},
                    bridge::install::InstallTarget::NewInstance { name } => {
                        let mut minecraft_version = content.version_hint;
                        if minecraft_version.is_none() {
                            if let Ok(meta) = self.meta.fetch(&MinecraftVersionManifestMetadataItem).await {
                                minecraft_version = Some(meta.latest.release.into());
                            }
                        }

                        if let Some(minecraft_version) = minecraft_version {
                            instance_dir = self.create_instance_sanitized(&name, &minecraft_version, content.loader_hint).await
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
                            let _ = std::fs::remove_file(replace);
                        }
                        let _ = std::fs::hard_link(install.from, target_path);
                    }
                }
            },
            Err(error) => {
                modal_action.set_error_message(Arc::from(format!("{}", error).as_str()));
            },
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
            eprintln!("Content install has invalid sha1: {}", sha1);
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

        let lockfile = Lockfile::create(path.with_added_extension("lock").into()).await;

        let _permit = semaphore.acquire().await.unwrap();

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
            tracker.set_finished(ProgressTrackerFinishType::Fast);
            tracker.notify();
            let summary = self.mod_metadata_manager.get_path(&path);
            return Ok((path, expected_hash, summary));
        }

        let response = self.redirecting_http_client.get(&**url).send().await?;

        if response.status() != StatusCode::OK {
            return Err(ContentInstallError::NotOK(response.status()));
        }

        // Tokio doesn't have lock, so we use std temporarily to lock it
        let file = std::fs::File::create(&path)?;
        _ = file.lock();

        let mut file = tokio::fs::File::from_std(file);

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
            file.write_all(&item).await?;
        }

        tracker.set_finished(ProgressTrackerFinishType::Fast);

        let actual_hash = hasher.finalize();

        let wrong_hash = *actual_hash != expected_hash;
        let wrong_size = total_bytes != size;

        if wrong_hash || wrong_size {
            let _ = file.set_len(0).await;
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;

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
