use std::path::Path;

use bridge::{import::{ImportFromOtherLauncher, ImportFromOtherLaunchers, OtherLauncher}, modal_action::ModalAction};
use schema::instance::InstanceConfiguration;
use crate::{BackendState, launcher_import::{
        modrinth::{import_instances_from_modrinth, read_profiles_from_modrinth_db},
        multimc::{import_from_multimc, try_load_from_multimc},
        atlauncher::import_from_atlauncher
    }
};

mod multimc;
mod modrinth;
mod atlauncher;

pub fn discover_instances_from_other_launchers() -> ImportFromOtherLaunchers {
    let mut imports = ImportFromOtherLaunchers::default();

    let Some(base_dirs) = directories::BaseDirs::new() else {
        return imports;
    };
    let data_dir = base_dirs.data_dir();

    let prism_instances = data_dir.join("PrismLauncher").join("instances");
    imports.imports[OtherLauncher::Prism] = from_subfolders(&prism_instances, &|path| {
        path.join("instance.cfg").exists() && path.join("mmc-pack.json").exists()
    });

    let multimc_instances = data_dir.join("multimc").join("instances");
    imports.imports[OtherLauncher::MultiMC] = from_subfolders(&multimc_instances, &|path| {
        path.join("instance.cfg").exists() && path.join("mmc-pack.json").exists()
    });

    if let Ok(import) = read_profiles_from_modrinth_db(data_dir) {
        imports.imports[OtherLauncher::Modrinth] = import;
    }

    let atlauncher_instances = data_dir.join("atlauncher").join("instances");
    imports.imports[OtherLauncher::ATLauncher] = from_subfolders(&atlauncher_instances, &|path| {
        path.join("instance.json").exists()
    });

    imports
}

fn from_subfolders(folder: &Path, check: &dyn Fn(&Path) -> bool) -> Option<ImportFromOtherLauncher> {
    let Ok(read_dir) = std::fs::read_dir(folder) else {
        return None;
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
        paths.push(path);
    }
    Some(ImportFromOtherLauncher {
        can_import_accounts: true,
        paths,
    })
}

pub fn try_load_from_other_launcher_formats(folder: &Path) -> Option<InstanceConfiguration> {
    let multimc_instance_cfg = folder.join("instance.cfg");
    let multimc_mmc_pack = folder.join("mmc-pack.json");
    if multimc_instance_cfg.exists() && multimc_mmc_pack.exists() {
        return try_load_from_multimc(&multimc_instance_cfg, &multimc_mmc_pack);
    }

    None
}

pub async fn import_from_other_launcher(backend: &BackendState, launcher: OtherLauncher, import_accounts: bool, import_instances: bool, modal_action: ModalAction) {
    let Some(base_dirs) = directories::BaseDirs::new() else {
        return;
    };
    let data_dir = base_dirs.data_dir();

    match launcher {
        OtherLauncher::Prism => {
            let prism = data_dir.join("PrismLauncher");
            import_from_multimc(backend, &prism, import_accounts, import_instances, modal_action).await;
        },
        OtherLauncher::Modrinth => {
            if import_instances {
                let modrinth = data_dir.join("ModrinthApp");
                if let Err(err) = import_instances_from_modrinth(backend, &modrinth, &modal_action) {
                    log::error!("Sqlite error while importing from modrinth: {err}");
                    modal_action.set_error_message("Sqlite error while importing from modrinth, see logs for more info".into());
                }
            }
        },
        OtherLauncher::MultiMC => {
            let multimc = data_dir.join("multimc");
            import_from_multimc(backend, &multimc, import_accounts, import_instances, modal_action).await;
        },
        OtherLauncher::ATLauncher => {
            let atlauncher = data_dir.join("atlauncher");
             import_from_atlauncher(backend, &atlauncher, import_accounts, import_instances, modal_action).await;
        }
    }
}
