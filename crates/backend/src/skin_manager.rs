use std::{collections::HashMap, io::Cursor, path::Path, sync::Arc, time::SystemTime};

use bridge::{message::{AccountSkinResult, BridgeDataLoadState, MessageToFrontend, SkinLibrary}};
use image::{DynamicImage, RgbaImage};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};
use tokio::sync::oneshot::Sender;
use uuid::Uuid;

use crate::{BackendState, FolderChanges};

pub struct SkinManager {
    skin_cache: HashMap<RgbaImage, UniqueBytes>,
    skins_download: FxHashMap<Arc<str>, SkinEntry>,
    pub skin_library_state: BridgeDataLoadState,
    skin_library_last: Vec<(SystemTime, Arc<Path>, UniqueBytes)>,
    skin_library: Arc<[UniqueBytes]>,
    skin_library_changes: FolderChanges,
    skin_path_map: HashMap<UniqueBytes, Arc<Path>>
}

impl Default for SkinManager {
    fn default() -> Self {
        Self {
            skin_cache: HashMap::new(),
            skins_download: Default::default(),
            skin_library_state: Default::default(),
            skin_library_last: Default::default(),
            skin_library: Default::default(),
            skin_library_changes: FolderChanges::all_dirty(),
            skin_path_map: HashMap::new(),
        }
    }
}

enum SkinEntry {
    Loading {
        accounts: Vec<Uuid>,
        frontend_requests: Vec<(SkinVariant, Sender<AccountSkinResult>)>,
    },
    Loaded {
        skin: UniqueBytes,
        head: UniqueBytes,
    },
    Failed,
}

impl SkinManager {
    fn create_skin(&mut self, image: RgbaImage, bytes: &[u8]) -> UniqueBytes {
        if let Some(cached) = self.skin_cache.get(&image) {
            cached.clone()
        } else {
            let skin = UniqueBytes::new(bytes);
            self.skin_cache.insert(image.clone(), skin.clone());
            skin
        }
    }

    pub fn remove_skin(backend: &BackendState, image_bytes: UniqueBytes) {
        let skin_manager = backend.skin_manager.write();
        let Some(skin_path) = skin_manager.skin_path_map.get(&image_bytes) else {
            log::warn!("Unable to find skin for deletion");
            return;
        };
        let Ok(_) = std::fs::remove_file(skin_path) else {
            log::warn!("Unable to delete skin at {}", skin_path.display());
            return;
        };
    }

    pub fn frontend_request(
        backend: &BackendState,
        skin_url: Arc<str>,
        skin_variant: SkinVariant,
        send: Sender<AccountSkinResult>
    ) {
        {
            let mut skin_manager = backend.skin_manager.write();
            if let Some(existing) = skin_manager.skins_download.get_mut(&skin_url) {
                match existing {
                    SkinEntry::Loading { frontend_requests, .. } => {
                        frontend_requests.push((skin_variant, send));
                    },
                    SkinEntry::Loaded { skin, .. } => {
                        let skin = skin.clone();
                        drop(skin_manager);
                        _ = send.send(AccountSkinResult::Success {
                            skin: Some(skin),
                            variant: skin_variant,
                        });
                    },
                    SkinEntry::Failed => {}
                }
                return;
            }
            skin_manager.skins_download.insert(skin_url.clone(), SkinEntry::Loading {
                accounts: Vec::new(),
                frontend_requests: vec![(skin_variant, send)]
            });
        }

        Self::download_skin(backend, skin_url);
    }

    pub fn update_account(backend: &BackendState, account: Uuid, skin_url: Arc<str>) {
        {
            let mut skin_manager = backend.skin_manager.write();
            if let Some(existing) = skin_manager.skins_download.get_mut(&skin_url) {
                match existing {
                    SkinEntry::Loading { accounts, .. } => {
                        accounts.push(account);
                    },
                    SkinEntry::Loaded { head, .. } => {
                        let head = head.clone();
                        drop(skin_manager);
                        backend.account_info.write().modify(move |account_info| {
                            if let Some(account) = account_info.accounts.get_mut(&account) {
                                account.head = Some(head);
                            }
                        });
                    },
                    SkinEntry::Failed => {}
                }
                return;
            }
            skin_manager.skins_download.insert(skin_url.clone(), SkinEntry::Loading { accounts: vec![account], frontend_requests: Vec::new() });
        }

        Self::download_skin(backend, skin_url);
    }

    pub fn is_valid_size(image: &DynamicImage) -> bool {
        if image.width() != 64 {
            return false;
        }
        image.height() == 64 || image.height() == 32
    }

    fn set_failed(skin_manager: Arc<RwLock<Self>>, skin_url: Arc<str>) {
        let previous = skin_manager.write().skins_download.insert(skin_url, SkinEntry::Failed);

        if let Some(SkinEntry::Loading { frontend_requests, .. }) = previous {
            for (_, frontend_request) in frontend_requests {
                _ = frontend_request.send(AccountSkinResult::UnableToLoadSkin);
            }
        }
    }

    pub fn download_skin(backend: &BackendState, skin_url: Arc<str>) {
        let skin_manager = backend.skin_manager.clone();
        let account_info = backend.account_info.clone();
        let http_client = backend.http_client.clone();

        tokio::task::spawn(async move {
            log::info!("Downloading skin from {}", skin_url);
            let Ok(response) = http_client.get(&*skin_url).send().await else {
                log::warn!("Http error while requesting skin from {}", skin_url);
                Self::set_failed(skin_manager, skin_url);
                return;
            };
            let Ok(bytes) = response.bytes().await else {
                log::warn!("Http error while downloading skin bytes from {}", skin_url);
                Self::set_failed(skin_manager, skin_url);
                return;
            };
            let Ok(mut image) = image::load_from_memory(&bytes) else {
                log::warn!("Image load error for skin from {}", skin_url);
                Self::set_failed(skin_manager, skin_url);
                return;
            };
            if !Self::is_valid_size(&image) {
                log::warn!("Invalid skin size for {}, got {}x{}", skin_url, image.width(), image.height());
                Self::set_failed(skin_manager, skin_url);
                return;
            }

            log::info!("Successfully downloaded skin from {}", skin_url);

            let mut head = image.crop(8, 8, 8, 8);
            let head_overlay = image.crop(40, 8, 8, 8);

            image::imageops::overlay(&mut head, &head_overlay, 0, 0);

            let mut head_bytes = Vec::new();
            let mut cursor = Cursor::new(&mut head_bytes);
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut cursor,
                image::codecs::png::CompressionType::Best,
                Default::default()
            );
            if head.write_with_encoder(encoder).is_err() {
                log::warn!("Error creating head for {}", skin_url);
                Self::set_failed(skin_manager, skin_url);
                return;
            }

            let mut skin_manager_guard = skin_manager.write();
            let head_png = UniqueBytes::new(&head_bytes);
            let skin = skin_manager_guard.create_skin(image.into_rgba8(), &*bytes);

            let previous = skin_manager_guard.skins_download.insert(skin_url.clone(), SkinEntry::Loaded {
                skin: skin.clone(),
                head: head_png.clone()
            });

            drop(skin_manager_guard);

            let Some(SkinEntry::Loading { accounts, frontend_requests }) = previous else {
                return;
            };

            for (variant, frontend_request) in frontend_requests {
                _ = frontend_request.send(AccountSkinResult::Success {
                    skin: Some(skin.clone()),
                    variant,
                });
            }

            if accounts.is_empty() {
                return;
            }

            let mut account_info = account_info.write();
            account_info.modify(move |info| {
                for uuid in accounts {
                    if let Some(account) = info.accounts.get_mut(&uuid) {
                        account.head = Some(head_png.clone());
                    }
                }
            });
        });
    }

    pub fn skin_library_mark_dirty(backend: &Arc<BackendState>, changes: FolderChanges) {
        if changes.is_empty() {
            return;
        }

        let mut skin_manager = backend.skin_manager.write();
        changes.apply_to(&mut skin_manager.skin_library_changes);

        skin_manager.skin_library_state.set_dirty();
        if skin_manager.skin_library_state.should_load() {
            drop(skin_manager);
            Self::load_skin_library(backend);
        }
    }

    pub fn load_skin_library(backend: &Arc<BackendState>) {
        let (all_dirty, dirty_paths) = {
            let mut skin_manager = backend.skin_manager.write();
            if skin_manager.skin_library_changes.is_empty() {
                return;
            }

            if !skin_manager.skin_library_state.should_load() {
                return;
            }
            skin_manager.skin_library_state.load_started();

            skin_manager.skin_library_changes.take()
        };

        let backend = backend.clone();
        backend.file_watching.write().watch_filesystem(backend.directories.skin_library_dir.clone(), crate::WatchTarget::SkinLibraryDir);

        if all_dirty {
            tokio::task::spawn_blocking(move || {
                let Ok(read_dir) = std::fs::read_dir(&backend.directories.skin_library_dir) else {
                    let mut skin_manager = backend.skin_manager.write();
                    skin_manager.skin_library_last = Default::default();
                    skin_manager.skin_library = Default::default();
                    skin_manager.skin_path_map = Default::default();
                    backend.send.send(MessageToFrontend::SkinLibraryUpdated {
                        skin_library: SkinLibrary {
                            state: skin_manager.skin_library_state.clone(),
                            skins: skin_manager.skin_library.clone(),
                            folder: backend.directories.skin_library_dir.clone(),
                        }
                    });
                    skin_manager.skin_library_state.load_finished();
                    if skin_manager.skin_library_state.should_load() {
                        SkinManager::load_skin_library(&backend);
                    }
                    return;
                };

                let mut skins = Vec::new();
                for entry in read_dir {
                    let Ok(entry) = entry else {
                        break;
                    };

                    let path = entry.path();

                    let Ok(bytes) = std::fs::read(&path) else {
                        continue;
                    };
                    let Ok(image) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) else {
                        continue;
                    };
                    if !Self::is_valid_size(&image) {
                        continue;
                    }

                    let mut time = SystemTime::UNIX_EPOCH;
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(created) = metadata.created() {
                            time = time.max(created);
                        }
                        if let Ok(modified) = metadata.modified() {
                            time = time.max(modified);
                        }
                    }

                    let path: Arc<Path> = path.into();


                    skins.push((time, path, image, bytes));
                }

                skins.sort_by_key(|(time, _, _, _)| *time);

                let mut skin_manager = backend.skin_manager.write();
                let skins = skins.into_iter().map(|(time, path, image, bytes)| {
                    (time, path, skin_manager.create_skin(image.to_rgba8(), &bytes))
                }).collect::<Vec<_>>();
                skin_manager.skin_library = skins.iter().map(|(_, _, bytes)| bytes.clone()).collect();
                skin_manager.skin_path_map = skins.iter().map(|(_, path, bytes)| (bytes.clone(), path.clone())).collect();
                skin_manager.skin_library_last = skins;
                backend.send.send(MessageToFrontend::SkinLibraryUpdated {
                    skin_library: SkinLibrary {
                        state: skin_manager.skin_library_state.clone(),
                        skins: skin_manager.skin_library.clone(),
                        folder: backend.directories.skin_library_dir.clone(),
                    }
                });
                skin_manager.skin_library_state.load_finished();
                if skin_manager.skin_library_state.should_load() {
                    SkinManager::load_skin_library(&backend);
                }
            });
        } else {
            tokio::task::spawn_blocking(move || {
                let mut skins = Vec::new();
                for path in &dirty_paths {
                    let mut time = SystemTime::UNIX_EPOCH;
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(created) = metadata.created() {
                            time = time.max(created);
                        }
                        if let Ok(modified) = metadata.modified() {
                            time = time.max(modified);
                        }
                    }

                    let Ok(bytes) = std::fs::read(&path) else {
                        continue;
                    };
                    let Ok(image) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) else {
                        continue;
                    };
                    if !Self::is_valid_size(&image) {
                        continue;
                    }

                    skins.push((time, path.clone(), image, bytes));
                }

                let mut skin_manager = backend.skin_manager.write();

                let mut skins = skins.into_iter().map(|(time, path, image, bytes)| {
                    (time, path, skin_manager.create_skin(image.to_rgba8(), &bytes))
                }).collect::<Vec<_>>();

                for existing in std::mem::take(&mut skin_manager.skin_library_last) {
                    if !dirty_paths.contains(&existing.1) {
                        skins.push(existing);
                    }
                }

                skins.sort_by_key(|(time, _, _)| *time);

                skin_manager.skin_library = skins.iter().map(|(_, _, bytes)| bytes.clone()).collect();
                skin_manager.skin_path_map = skins.iter().map(|(_, path, bytes)| (bytes.clone(), path.clone())).collect();
                skin_manager.skin_library_last = skins;
                backend.send.send(MessageToFrontend::SkinLibraryUpdated {
                    skin_library: SkinLibrary {
                        state: skin_manager.skin_library_state.clone(),
                        skins: skin_manager.skin_library.clone(),
                        folder: backend.directories.skin_library_dir.clone(),
                    }
                });
                skin_manager.skin_library_state.load_finished();
                if skin_manager.skin_library_state.should_load() {
                    SkinManager::load_skin_library(&backend);
                }
            });
        }
    }
}
