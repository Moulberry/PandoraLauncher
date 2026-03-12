use std::{collections::HashMap, path::{Path, PathBuf}};

use bridge::{import::{ImportFromOtherLauncher, ImportFromOtherLaunchers, ImportStatus, OtherLauncher, OtherLauncherIter}, modal_action::ModalAction};
use log::debug;
use schema::instance::InstanceConfiguration;
use strum::IntoEnumIterator;
use crate::{BackendState,
    launcher_import::{
        modrinth::{import_instances_from_modrinth, read_profiles_from_modrinth_db},
	    multimc::{import_from_multimc, try_load_from_multimc},
	    atlauncher::import_from_atlauncher
    }
};

mod multimc;
mod modrinth;
mod atlauncher;
// Leaving this here as a note...
//
// Each launcher importer we support needs to:
// - implement the below API.
//
// besides that, unpon requesting the files for a certain launcher we need to...
// - Get every launcher to scan the directory to see which is valid.
// 		(this might get expensive but i don't see any other way. Worst case scenario we just shove rayon at it?)
// - If it is a valid launcher, we need to return an object of said launcher with the bsaic details.
// 		- Alternatively, an generalised object in a vector? of all the launchers.
// - The user on the front-end selects what they want to import.
// 		- Options between: accounts (specific account?), instances, deduplication (if possible)
// - The backend then processes each launcher like we currently do. (in parallel?)

pub fn discover_instances_from_other_launchers(backend: &BackendState) -> ImportFromOtherLaunchers {
    let mut imports = ImportFromOtherLaunchers::default();

    let Some(base_dirs) = directories::BaseDirs::new() else {
        return imports;
    };
    let data_dir = base_dirs.data_dir();
    let pandora_dir = &backend.directories.instances_dir;

    let prism_instances = data_dir.join("PrismLauncher").join("instances");
    imports.imports[OtherLauncher::Prism] = from_subfolders(OtherLauncher::Prism, &prism_instances, &pandora_dir, &|path| {
        path.join("instance.cfg").exists() && path.join("mmc-pack.json").exists()
    });

    let multimc_instances = data_dir.join("multimc").join("instances");
    imports.imports[OtherLauncher::MultiMC] = from_subfolders(OtherLauncher::MultiMC, &multimc_instances, &pandora_dir, &|path| {
        path.join("instance.cfg").exists() && path.join("mmc-pack.json").exists()
    });

    if let Ok(import) = read_profiles_from_modrinth_db(data_dir, &pandora_dir) {
        imports.imports[OtherLauncher::Modrinth] = import;
    }

    let atlauncher_instances = data_dir.join("atlauncher");
    imports.imports[OtherLauncher::ATLauncher] = get_launcher_details(backend, OtherLauncher::ATLauncher, &atlauncher_instances);

    imports
}

pub fn discover_instances_from_path(backend: &BackendState, path: PathBuf) -> Option<ImportFromOtherLauncher> {
 	debug!("Received request to update data w/path: {:?}", path);

  	for launcher in OtherLauncher::iter() {
        let details = get_launcher_details(backend, launcher, &path);
        if details.is_some() {
            debug!("Returning data from {}", launcher);
            return details;
        }
    }

    debug!("Backend found nothing");
    None
}

fn instance_check(launcher: OtherLauncher, path: &PathBuf) -> bool {
    match launcher {
        OtherLauncher::MultiMC | OtherLauncher::Prism => false,
        OtherLauncher::Modrinth => false,
        OtherLauncher::ATLauncher => atlauncher::is_valid_atinstance(path),
    }
}

fn get_launcher_details(backend: &BackendState, launcher: OtherLauncher, path: &PathBuf) -> Option<ImportFromOtherLauncher> {
    let mut import_data = ImportFromOtherLauncher::new_launcher(launcher);

    let is_instance = instance_check(launcher, &path);
    if is_instance {
        import_data.instances.insert(path.clone(), ImportStatus::Importing);
    } else {
        for path in loop_subfolders(&path, &|path| instance_check(launcher, &path.to_path_buf())) {
            import_data.instances.insert(path, ImportStatus::Importing);
        }
        for path in loop_subfolders(&path.join("instances"), &|path| instance_check(launcher, &path.to_path_buf())) {
            import_data.instances.insert(path, ImportStatus::Importing);
        }
    }

    import_data.instances.iter_mut().for_each(|(instance, status)| {
        if backend.directories.instances_dir.join(instance.file_name().unwrap()).exists() {
            *status = ImportStatus::Duplicate;
        }
    });

    import_data.account = match launcher {
        OtherLauncher::MultiMC | OtherLauncher::Prism => None,
        OtherLauncher::Modrinth => None,
        OtherLauncher::ATLauncher => atlauncher::is_valid_ataccount(&path),
    };

    let valid_details = import_data.account.is_some() && !import_data.instances.is_empty();
    valid_details.then(|| import_data)
}

fn loop_subfolders(folder: &Path, check: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let Ok(read_dir) = std::fs::read_dir(folder) else {
        return vec![]
    };
    let mut paths = vec![];
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
    paths
}

fn from_subfolders(launcher: OtherLauncher, folder: &Path, pandora: &Path, check: &dyn Fn(&Path) -> bool) -> Option<ImportFromOtherLauncher> {
    let subfolders = loop_subfolders(folder, check);
    if subfolders.is_empty() { return None; }

    let paths = subfolders.iter().map(|path| {
        let state = if pandora.join(path.file_name().unwrap()).exists() { ImportStatus::Duplicate } else { ImportStatus::Importing };
        (path.to_path_buf(), state)
    }).collect::<HashMap<PathBuf, ImportStatus>>();
    Some(ImportFromOtherLauncher {
    	launcher,
    	instances: paths,
        account: None,
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

pub async fn import_from_other_launcher(backend: &BackendState, details: ImportFromOtherLauncher, modal_action: ModalAction) {
    let Some(base_dirs) = directories::BaseDirs::new() else {
        return;
    };
    let data_dir = base_dirs.data_dir();

    match details.launcher {
        OtherLauncher::Prism => {
            let prism = data_dir.join("PrismLauncher");
            import_from_multimc(backend, &prism, details.account.is_some(), details.instances, modal_action).await;
        },
        OtherLauncher::Modrinth => {
        	// bit harder to say to modrithn, "hey here are the paths.", so just going to ignore this for now.
         	// TODO: The above is possible, it just needs some work. THIS PR SHOULD NOT BE MERGED UNTIL THIS IS RESOLVED.
            if details.instances.iter().any(|(_, status)| *status == ImportStatus::Importing) {
                let modrinth = data_dir.join("ModrinthApp");
                if let Err(err) = import_instances_from_modrinth(backend, &modrinth, &modal_action) {
                    log::error!("Sqlite error while importing from modrinth: {err}");
                    modal_action.set_error_message("Sqlite error while importing from modrinth, see logs for more info".into());
                }
            }
        },
        OtherLauncher::MultiMC => {
            let multimc = data_dir.join("multimc");
            import_from_multimc(backend, &multimc, details.account.is_some(), details.instances, modal_action).await;
        },
         OtherLauncher::ATLauncher => {
         	let atlauncher = data_dir.join("atlauncher");
          	import_from_atlauncher(backend, &atlauncher, details, modal_action).await;
        }
    }
}
