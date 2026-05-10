use std::{path::Path, sync::Arc};

use bridge::{import::{ImportFromOtherLauncherJob, OtherLauncher}, modal_action::ModalAction};
use schema::instance::InstanceConfiguration;
use crate::{BackendState, launcher_import::{
        atlauncher::import_from_atlauncher, curseforge::import_from_curseforge, modrinth::{import_instances_from_modrinth, read_profiles_from_modrinth_db}, multimc::{import_from_multimc, try_load_from_multimc}
    }
};

mod multimc;
mod modrinth;
mod atlauncher;
mod curseforge;

pub fn get_import_from_other_launcher_job(other_launcher: OtherLauncher, path: Arc<Path>) -> Option<ImportFromOtherLauncherJob> {
    if !path.is_dir() {
        return None;
    }
    match other_launcher {
        OtherLauncher::Prism | OtherLauncher::MultiMC => {
            if !path.join("prismlauncher.cfg").is_file() && !path.join("multimc.cfg").is_file() && !path.join("fjordlauncher.cfg").is_file() {
                return None;
            }
            Some(ImportFromOtherLauncherJob {
                import_accounts: path.join("accounts.json").is_file(),
                paths: collect_subfolders_matching(&path.join("instances"), &|path| {
                    path.join("instance.cfg").exists() && path.join("mmc-pack.json").exists()
                }),
                root: path,
            })
        },
        OtherLauncher::CurseForge => {
            Some(ImportFromOtherLauncherJob {
                import_accounts: false,
                paths: collect_subfolders_matching(&path.join("Instances"), &|path| {
                    path.join("minecraftinstance.json").exists()
                }),
                root: path
            })
        }
        OtherLauncher::Modrinth => {
            let paths = match read_profiles_from_modrinth_db(&path) {
                Ok(paths) => paths?,
                Err(err) => {
                    log::error!("Unable to read modrinth profile database: {err}");
                    return None;
                },
            };

            Some(ImportFromOtherLauncherJob {
                import_accounts: false,
                paths,
                root: path,
            })
        },
        OtherLauncher::ATLauncher => {
            if !path.join("configs/ATLauncher.json").is_file() {
                return None;
            }
            Some(ImportFromOtherLauncherJob {
                import_accounts: path.join("configs/accounts.json").is_file(),
                paths: collect_subfolders_matching(&path.join("instances"), &|path| {
                    path.join("instance.json").exists()
                }),
                root: path,
            })
        },
    }
}

fn collect_subfolders_matching(folder: &Path, check: &dyn Fn(&Path) -> bool) -> Vec<Arc<Path>> {
    let Ok(read_dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !(check)(&path) {
            continue;
        }
        paths.push(path.into());
    }
    paths
}

pub fn try_load_from_other_launcher_formats(folder: &Path) -> Option<InstanceConfiguration> {
    let multimc_instance_cfg = folder.join("instance.cfg");
    let multimc_mmc_pack = folder.join("mmc-pack.json");
    if multimc_instance_cfg.exists() && multimc_mmc_pack.exists() {
        return Some(try_load_from_multimc(&multimc_instance_cfg, &multimc_mmc_pack)?.0);
    }

    None
}

pub async fn import_from_other_launcher(backend: &BackendState, launcher: OtherLauncher, import_job: ImportFromOtherLauncherJob, modal_action: ModalAction) {
    match launcher {
        OtherLauncher::Prism | OtherLauncher::MultiMC => {
            import_from_multimc(backend, import_job, modal_action).await;
        },
        OtherLauncher::CurseForge => {
            import_from_curseforge(backend, import_job, modal_action);
        }
        OtherLauncher::Modrinth => {
            if let Err(err) = import_instances_from_modrinth(backend, import_job, &modal_action) {
                log::error!("Sqlite error while importing from modrinth: {err}");
                modal_action.set_error_message("Sqlite error while importing from modrinth, see logs for more info".into());
            }
        },
        OtherLauncher::ATLauncher => {
            import_from_atlauncher(backend, import_job, modal_action).await;
        }
    }
}
