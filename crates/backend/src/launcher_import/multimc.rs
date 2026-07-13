use std::{path::{Path, PathBuf}, sync::Arc};

use auth::{credentials::AccountCredentials, models::{TokenWithExpiry, XstsToken}, secret::PlatformSecretStorage};
use bridge::{import::ImportFromOtherLauncherJob, modal_action::{ModalAction, ProgressTracker}};
use chrono::DateTime;
use schema::{instance::{InstanceConfiguration, LwjglLibraryPath}, loader::Loader};
use serde::Deserialize;
use uuid::Uuid;

use crate::{BackendState, account::BackendAccount, instance::InstanceStats};


#[derive(Deserialize)]
struct MMCPack {
    components: Vec<MMCPackComponent>
}

#[derive(Deserialize)]
struct MMCPackComponent {
    uid: Arc<str>,
    version: Arc<str>,
}

pub fn try_load_from_multimc(instance_cfg: &Path, mmc_pack: &Path) -> Option<(InstanceConfiguration, InstanceStats)> {
    let mmc_pack_bytes = std::fs::read(mmc_pack).ok()?;
    let instance_cfg_str = std::fs::read_to_string(instance_cfg).ok()?;

    let mmc_pack = serde_json::from_slice::<MMCPack>(&mmc_pack_bytes).ok()?;

    let mut minecraft_version = None;
    let mut loader = None;

    for component in mmc_pack.components {
        if &*component.uid == "net.minecraft" {
            minecraft_version = Some(component.version.into());
        } else if &*component.uid == "net.fabricmc.fabric-loader" {
            loader = Some(Loader::Fabric);
        } else if &*component.uid == "net.minecraftforge" {
            loader = Some(Loader::Forge);
        } else if &*component.uid == "net.neoforged" {
            loader = Some(Loader::NeoForge);
        }
    }

    let mut configuration = InstanceConfiguration::new(minecraft_version?, loader.unwrap_or(Loader::Vanilla));
    let mut stats = InstanceStats::default();

    let mut override_native_workarounds = false;
    let mut override_performance = false;
    let mut override_account = (false, None);

    let mut section = None;
    for line in instance_cfg_str.split(|v| v == '\n') {
        let line = line.trim_ascii_start();
        if line.is_empty() {
            continue;
        }

        let start = line.as_bytes()[0];
        match start {
            b';' | b'#' => continue,
            b'[' => {
                section = Some(line.trim_ascii_end());
            },
            _ => {
                let Some((key, value)) = line.split_once("=") else {
                    continue;
                };


                let mut value = value.trim_ascii();
                if value.len() > 1 && value.starts_with('"') && value.ends_with('"') {
                    value = &value[1..value.len()-1];
                } else if value.len() > 1 && value.starts_with('\'') && value.ends_with('\'') {
                    value = &value[1..value.len()-1];
                }

                match (section, key) {
                    // JVM Binary
                    (Some("[General]"), "OverrideJavaLocation") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.jvm_binary.get_or_insert_default().enabled = enabled;
                    },
                    (Some("[General]"), "JavaPath") => {
                        configuration.jvm_binary.get_or_insert_default().path = Some(Path::new(value).into());
                    },
                    // Java Args
                    (Some("[General]"), "OverrideJavaArgs") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.jvm_flags.get_or_insert_default().enabled = enabled;
                    },
                    (Some("[General]"), "JvmArgs") => {
                        configuration.jvm_flags.get_or_insert_default().flags = value.into();
                    },
                    // Memory
                    (Some("[General]"), "OverrideMemory") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.memory.get_or_insert_default().enabled = enabled;
                    },
                    (Some("[General]"), "MinMemAlloc") => {
                        let Ok(min) = value.parse::<u32>() else {
                            continue;
                        };
                        configuration.memory.get_or_insert_default().min = min;
                    },
                    (Some("[General]"), "MaxMemAlloc") => {
                        let Ok(max) = value.parse::<u32>() else {
                            continue;
                        };
                        configuration.memory.get_or_insert_default().max = max;
                    },
                    // Native workarounds
                    (Some("[General]"), "OverrideNativeWorkarounds") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        override_native_workarounds = enabled;
                    },
                    (Some("[General]"), "UseNativeOpenAL") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.system_libraries.get_or_insert_default().override_openal = enabled;
                    },
                    (Some("[General]"), "CustomOpenALPath") => {
                        if value.is_empty() {
                            configuration.system_libraries.get_or_insert_default().openal = LwjglLibraryPath::Auto;
                        } else {
                            configuration.system_libraries.get_or_insert_default().openal = LwjglLibraryPath::Explicit(Path::new(value).into());
                        }
                    },
                    (Some("[General]"), "UseNativeGLFW") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.system_libraries.get_or_insert_default().override_glfw = enabled;
                    },
                    (Some("[General]"), "CustomGLFWPath") => {
                        if value.is_empty() {
                            configuration.system_libraries.get_or_insert_default().glfw = LwjglLibraryPath::Auto;
                        } else {
                            configuration.system_libraries.get_or_insert_default().glfw = LwjglLibraryPath::Explicit(Path::new(value).into());
                        }
                    },
                    // Linux Performance
                    (Some("[General]"), "OverridePerformance") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        override_performance = enabled;
                    },
                    (Some("[General]"), "EnableFeralGamemode") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.linux_wrapper.get_or_insert_default().use_gamemode = enabled;
                    },
                    (Some("[General]"), "EnableMangoHud") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.linux_wrapper.get_or_insert_default().use_mangohud = enabled;
                    },
                    (Some("[General]"), "UseDiscreteGpu") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        configuration.linux_wrapper.get_or_insert_default().use_discrete_gpu = enabled;
                    },
                    (Some("[General]"), "UseAccountForInstance") => {
                        let Ok(enabled) = value.parse::<bool>() else {
                            continue;
                        };
                        override_account.0 = enabled;
                    },
                    (Some("[General]"), "InstanceAccountId") => {
                        override_account.1 = value.parse::<Uuid>().ok();
                    },
                    (Some("[General]"), "totalTimePlayed") => {
                        let Ok(time_played) = value.parse::<u64>() else {
                            continue;
                        };
                        stats.total_playtime_secs = time_played;
                    },
                    (Some("[General]"), "lastLaunchTime") => {
                        let Ok(last_launcher_time) = value.parse::<i64>() else {
                            continue;
                        };
                        stats.last_played_unix_ms = Some(last_launcher_time);
                    }
                    _ => {}
                }
            }
        }
    }

    if !override_native_workarounds {
        configuration.system_libraries = None;
    }
    if !override_performance {
        configuration.linux_wrapper = None;
    }

    if override_account.0 {
        configuration.preferred_account = override_account.1;
    }

    Some((configuration, stats))
}

#[derive(Deserialize, Debug)]
struct MultiMCAccountsJson {
    accounts: Vec<MultiMCAccount>
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum MultiMCAccount {
    Offline {
        active: Option<bool>,
        profile: Option<MultiMCAccountProfile>
    },
    MSA {
        active: Option<bool>,
        profile: Option<MultiMCAccountProfile>,
        #[serde(rename = "msa-client-id")]
        msa_client_id: Option<Arc<str>>,
        msa: Option<MultiMCAccountToken>,
        utoken: Option<MultiMCAccountToken>,
        #[serde(rename = "xrp-mc")]
        xrp_mc: Option<MultiMCAccountToken>,
        ygg: Option<MultiMCAccountToken>,
    }
}

#[derive(Deserialize, Debug)]
struct MultiMCAccountProfile {
    id: Uuid,
    name: Arc<str>,
}

#[derive(Deserialize, Debug)]
struct MultiMCAccountToken {
    token: Arc<str>,
    extra: Option<MultiMCAccountTokenExtra>,
    refresh_token: Option<Arc<str>>,
    exp: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct MultiMCAccountTokenExtra {
    uhs: Option<Arc<str>>,
}

pub async fn import_from_multimc(backend: &BackendState, import_job: ImportFromOtherLauncherJob, modal_action: ModalAction) {
    import_accounts_from_multimc(backend, &import_job, &modal_action).await;
    import_instances_from_multimc(backend, &import_job, &modal_action);
}

async fn import_accounts_from_multimc(backend: &BackendState, import_job: &ImportFromOtherLauncherJob, modal_action: &ModalAction) {
    if !import_job.import_accounts {
        return;
    }

    let tracker = ProgressTracker::new("Reading accounts.json".into(), backend.send.clone());
    modal_action.trackers.push(tracker.clone());
    tracker.notify();

    let accounts_path = import_job.root.join("accounts.json");
    let Ok(accounts_bytes) = std::fs::read(&accounts_path) else {
        return;
    };

    let Ok(accounts_json) = serde_json::from_slice::<MultiMCAccountsJson>(&accounts_bytes) else {
        return;
    };

    let secret_storage = match backend.secret_storage.get_or_init(PlatformSecretStorage::new).await {
        Ok(secret_storage) => secret_storage,
        Err(error) => {
            log::error!("Error initializing secret storage: {error}");
            return;
        }
    };

    let num_accounts = accounts_json.accounts.len();
    tracker.set_title("Importing accounts".into());
    tracker.add_total(num_accounts);

    backend.account_info.write().modify(|accounts| {
        for account in &accounts_json.accounts {
            match account {
                MultiMCAccount::Offline { active, profile } | MultiMCAccount::MSA { active, profile, .. } => {
                    let Some(profile) = profile else {
                        continue;
                    };

                    tracker.add_count(1);
                    tracker.notify();

                    if let Some(account) = accounts.accounts.get_mut(&profile.id) {
                        account.offline = false;
                        account.username = profile.name.clone();
                    } else {
                        accounts.accounts.insert(profile.id, BackendAccount {
                            username: profile.name.clone(),
                            offline: false,
                            head: None
                        });
                    }

                    if *active == Some(true) {
                        accounts.selected_account = Some(profile.id);
                    }
                },
            }
        }
    });

    tracker.set_title("Importing credentials".into());
    tracker.set_count(0);
    tracker.set_total(num_accounts);
    tracker.notify();

    for account in accounts_json.accounts {
        if let MultiMCAccount::MSA { active: _, profile, msa_client_id, msa, utoken, xrp_mc, ygg } = account {
            let Some(profile) = profile else {
                continue;
            };

            let mut credentials = AccountCredentials::default();
            let mut non_default_creds = false;

            let now = chrono::Utc::now();

            if let Some(msa) = msa && let Some(exp) = msa.exp {
                if let Some(msa_client_id) = msa_client_id && msa.refresh_token.is_some() {
                    non_default_creds = true;
                    credentials.msa_refresh = msa.refresh_token;
                    credentials.msa_refresh_force_client_id = Some(msa_client_id);
                }
                if let Some(exp) = DateTime::from_timestamp_secs(exp) && exp < now {
                    non_default_creds = true;
                    credentials.msa_access = Some(TokenWithExpiry {
                        token: msa.token,
                        expiry: exp,
                    });
                }
            }
            if let Some(xbl) = utoken && let Some(exp) = xbl.exp {
                if let Some(exp) = DateTime::from_timestamp_secs(exp) && exp < now {
                    non_default_creds = true;
                    credentials.xbl = Some(TokenWithExpiry {
                        token: xbl.token,
                        expiry: exp,
                    });
                }
            }
            if let Some(xsts) = xrp_mc
                && let Some(exp) = xsts.exp
                && let Some(extra) = xsts.extra
                && let Some(uhs) = extra.uhs
            {
                if let Some(exp) = DateTime::from_timestamp_secs(exp) && exp < now {
                    non_default_creds = true;
                    credentials.xsts = Some(XstsToken {
                        token: xsts.token,
                        expiry: exp,
                        userhash: uhs
                    });
                }
            }
            if let Some(ygg) = ygg && let Some(exp) = ygg.exp {
                if let Some(exp) = DateTime::from_timestamp_secs(exp) && exp < now {
                    non_default_creds = true;
                    credentials.access_token = Some(TokenWithExpiry {
                        token: ygg.token,
                        expiry: exp,
                    });
                }
            }

            if non_default_creds {
                _ = secret_storage.write_credentials(profile.id, &credentials).await;
            }
        }
    }

    tracker.set_count(num_accounts);
    tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Normal);
    tracker.notify();
}

struct MultiMCInstanceToImport {
    pandora_path: PathBuf,
    multimc_instance_cfg: PathBuf,
    multimc_mmc_pack: PathBuf,
    folder: Arc<Path>,
}

fn import_instances_from_multimc(backend: &BackendState, import_job: &ImportFromOtherLauncherJob, modal_action: &ModalAction) {
    if import_job.paths.is_empty() {
        return;
    }

    let all_tracker = ProgressTracker::new("Importing instances".into(), backend.send.clone());
    modal_action.trackers.push(all_tracker.clone());
    all_tracker.notify();

    let mut to_import = Vec::new();

    for folder in import_job.paths.iter() {
        if !folder.is_dir() {
            continue;
        }

        let Some(filename) = folder.file_name() else {
            continue;
        };

        let pandora_path = backend.directories.instances_dir.join(filename);
        if pandora_path.exists() {
           continue;
        }

        let multimc_instance_cfg = folder.join("instance.cfg");
        let multimc_mmc_pack = folder.join("mmc-pack.json");
        if !multimc_instance_cfg.exists() || !multimc_mmc_pack.exists() {
            continue;
        }

        to_import.push(MultiMCInstanceToImport {
            pandora_path,
            multimc_instance_cfg,
            multimc_mmc_pack,
            folder: folder.clone(),
        });
    }

    all_tracker.set_total(to_import.len());

    for to_import in to_import {
        let title = format!("Importing {}", to_import.folder.file_name().unwrap().to_string_lossy());
        let tracker = ProgressTracker::new(title.into(), backend.send.clone());
        modal_action.trackers.push(tracker.clone());
        tracker.notify();

        let Some((configuration, stats)) = try_load_from_multimc(&to_import.multimc_instance_cfg, &to_import.multimc_mmc_pack) else {
            tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
            tracker.notify();
            continue;
        };
        let Ok(configuration_bytes) = serde_json::to_vec(&configuration) else {
            tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
            tracker.notify();
            continue;
        };

        _ = std::fs::create_dir_all(&to_import.pandora_path);

        // Copy .minecraft folder
        let mmc_dot_minecraft = to_import.folder.join(".minecraft");
        let mmc_minecraft = to_import.folder.join("minecraft");
        let target_dot_minecraft = to_import.pandora_path.join(".minecraft");
        if mmc_minecraft.exists() {
            _ = std::fs::create_dir_all(&target_dot_minecraft);
            _ = crate::copy_content_recursive(&mmc_minecraft, &target_dot_minecraft, false, &|copied, total| {
                tracker.set_total(total as usize);
                tracker.set_count(copied as usize);
                tracker.notify();
            });
        } else if mmc_dot_minecraft.exists() {
            _ = std::fs::create_dir_all(&target_dot_minecraft);
            _ = crate::copy_content_recursive(&mmc_dot_minecraft, &target_dot_minecraft, false, &|copied, total| {
                tracker.set_total(total as usize);
                tracker.set_count(copied as usize);
                tracker.notify();
            });
        }

        // Copy icon
        _ = std::fs::copy(to_import.folder.join("icon.png"), to_import.pandora_path.join("icon.png"));

        // Write info_v1.json
        let info_path = to_import.pandora_path.join("info_v1.json");
        _ = crate::write_safe(&info_path, &configuration_bytes);

        // Write stats_v1.json if we have some stats
        if stats != InstanceStats::default() {
            let stats_path = to_import.pandora_path.join("stats_v1.json");
            if let Ok(stats_bytes) = serde_json::to_vec(&stats) {
                _ = crate::write_safe(&stats_path, &stats_bytes);
            }
        }

        all_tracker.add_count(1);
        all_tracker.notify();

        tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Fast);
        tracker.notify();
    }

    all_tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Normal);
    all_tracker.notify();
}
