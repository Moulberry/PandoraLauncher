use std::path::{Path, PathBuf};

use bridge::{import::{ImportFromOtherLauncher, ImportFromOtherLaunchers, ImportStatus, OtherLauncher}, modal_action::ModalAction};
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
// Each launcher importer supported does the following:
//
// - Get every launcher to scan the directory to see which is valid.
// 		(this might get expensive but i don't see any other way. Worst case scenario we just shove rayon at it?)
// - If it is a valid launcher, we need to return an object of said launcher with the basic details.
// - The user on the front-end selects what they want to import.
// 		- Options between: accounts (specific account?), instances, deduplication (if possible)
// - The backend then processes each launcher like we currently do. (in parallel?)

/// Basic instance discover program. Finds them one by one and generates a list of them.
/// An extension in a way of the custom path version, but returns more information.
pub fn discover_instances_from_other_launchers(backend: &BackendState) -> ImportFromOtherLaunchers {
    let mut imports = ImportFromOtherLaunchers::default();

    let Some(base_dirs) = directories::BaseDirs::new() else {
        return imports;
    };
    let data_dir = base_dirs.data_dir();
    let pandora_dir = &backend.directories.instances_dir;

    let prism_instances = data_dir.join("PrismLauncher").join("instances");
    imports.imports[OtherLauncher::Prism] = get_launcher_details(backend, OtherLauncher::Prism, &prism_instances);

    let multimc_instances = data_dir.join("multimc").join("instances");
    imports.imports[OtherLauncher::MultiMC] = get_launcher_details(backend, OtherLauncher::MultiMC, &multimc_instances);

    let modrinth_dir = data_dir.join("ModrinthApp");
    if let Ok(import) = read_profiles_from_modrinth_db(&modrinth_dir, &pandora_dir) {
        imports.imports[OtherLauncher::Modrinth] = import;
    }

    let atlauncher_instances = data_dir.join("atlauncher");
    imports.imports[OtherLauncher::ATLauncher] = get_launcher_details(backend, OtherLauncher::ATLauncher, &atlauncher_instances);

  imports
}

/// Loop through all potential instances and returns the first one found.
pub fn discover_instances_from_path(backend: &BackendState, path: PathBuf) -> Option<ImportFromOtherLauncher> {
 	debug!("Received request to update data w/path: {:?}", path);

    // modrith doesn't conform to standards, hence we deal with it separately...
    if let Ok(modrinth) = read_profiles_from_modrinth_db(&path, &backend.directories.instances_dir) {
        if modrinth.is_some() { return modrinth; }
    }

  	for launcher in OtherLauncher::iter()
        .filter(|launcher| *launcher != OtherLauncher::Modrinth)
    {
        let details = get_launcher_details(backend, launcher, &path);
        if details.is_some() {
            debug!("Returning data from {}", launcher);
            return details;
        }
    }

    debug!("Backend found nothing");
    None
}

/// Checks to see if the provided path is a valid instance.
///
/// Path could be one of the following:
/// - `{dir}`
/// - `{dir}/{some_dir}`
/// - `{dir}/instances/{some_instance}`
///
/// where `dir` is the launcher default dir or the dir provided by the user.
fn instance_check(launcher: OtherLauncher, path: &PathBuf) -> bool {
    match launcher {
        OtherLauncher::MultiMC | OtherLauncher::Prism => multimc::is_valid_mmcinstance(path),
        OtherLauncher::Modrinth => false,
        OtherLauncher::ATLauncher => atlauncher::is_valid_atinstance(path),
    }
}

/// Attempts to get all the information for that specific launcher.
fn get_launcher_details(backend: &BackendState, launcher: OtherLauncher, path: &PathBuf) -> Option<ImportFromOtherLauncher> {
    let mut import_data = ImportFromOtherLauncher::new_launcher(launcher);

    // we take the best case scenario and check the current folder.
    let is_instance = instance_check(launcher, &path);
    if is_instance {
        import_data.instances.insert(path.clone(), ImportStatus::Importing);
    } else {
        // otherwise we check one level deep
        for path in loop_subfolders(&path, &|path| instance_check(launcher, &path.to_path_buf())) {
            import_data.instances.insert(path, ImportStatus::Importing);
        }
        // and we check the instances folder just in case we're in the main directory.
        for path in loop_subfolders(&path.join("instances"), &|path| instance_check(launcher, &path.to_path_buf())) {
            import_data.instances.insert(path, ImportStatus::Importing);
        }
    }

    // cross-check the instances to be imported with the current imported instances to alert of dupes.
    import_data.instances.iter_mut().for_each(|(instance, status)| {
        if backend.directories.instances_dir.join(instance.file_name().unwrap()).exists() {
            *status = ImportStatus::Duplicate;
        }
    });

    // check for accounts info.
    import_data.account = match launcher {
        OtherLauncher::MultiMC | OtherLauncher::Prism => multimc::is_valid_mmcaccount(&path),
        OtherLauncher::Modrinth => None,
        OtherLauncher::ATLauncher => atlauncher::is_valid_ataccount(&path),
    };

    // only if we have something, then the details are valid.
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

pub async fn import_from_other_launcher(backend: &BackendState, details: ImportFromOtherLauncher, modal_action: ModalAction) {
    println!("Recieved import request with data: {:#?}", details);
    match details.launcher {
        OtherLauncher::Prism => {
            import_from_multimc(backend, details, modal_action).await;
        },
        OtherLauncher::Modrinth => {
            if details.instances.iter().any(|(_, status)| *status == ImportStatus::Importing) {
                // In theory, (due to limitations) the parent, parent path of any instance should be our folder..
                // if it's not, something went wrong. but at that point something else is probably completely broken...
                let modrinth = (&details.instances).iter().nth(0).unwrap().0.parent().unwrap().parent().unwrap();
                if let Err(err) = import_instances_from_modrinth(backend, &modrinth, &details.instances, &modal_action) {
                    log::error!("Sqlite error while importing from modrinth: {err}");
                    modal_action.set_error_message("Sqlite error while importing from modrinth, see logs for more info".into());
                }
            }
        },
        OtherLauncher::MultiMC => {
            import_from_multimc(backend, details, modal_action).await;
        },
         OtherLauncher::ATLauncher => {
          	import_from_atlauncher(backend, details, modal_action).await;
        }
    }
}

pub fn try_load_from_other_launcher_formats(folder: &Path) -> Option<InstanceConfiguration> {
    let multimc_instance_cfg = folder.join("instance.cfg");
    let multimc_mmc_pack = folder.join("mmc-pack.json");
    if multimc_instance_cfg.exists() && multimc_mmc_pack.exists() {
        return try_load_from_multimc(&multimc_instance_cfg, &multimc_mmc_pack);
    }

    None
}
