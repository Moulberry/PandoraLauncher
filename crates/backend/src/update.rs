use std::{ffi::{OsStr, OsString}, io::Cursor, path::{Path, PathBuf}, sync::Arc};

use base64::Engine;
use bridge::{handle::FrontendHandle, message::MessageToFrontend, modal_action::{ModalAction, ProgressTracker}};
use reqwest::StatusCode;
use schema::pandora_update::{UpdateInstallType, UpdateManifest, UpdatePrompt};
use sha1::{Digest, Sha1};
use rand::RngCore;

use crate::directories::LauncherDirectories;

pub async fn check_for_updates(http_client: reqwest::Client, send: FrontendHandle) {
    if option_env!("PANDORA_UPDATE_PUBKEY").is_none() {
        return;
    }

    let Some(version) = option_env!("PANDORA_RELEASE_VERSION") else {
        log::warn!("Skipping update check because PANDORA_RELEASE_VERSION isn't set");

        #[cfg(not(debug_assertions))] // Don't show error in non-release builds
        send.send_warning("Unable to check for updates, missing PANDORA_RELEASE_VERSION");
        return;
    };

    let Some(repository_url) = option_env!("GITHUB_REPOSITORY_URL") else {
        log::warn!("Skipping update check because GITHUB_REPOSITORY_URL isn't set");

        #[cfg(not(debug_assertions))] // Don't show error in non-release builds
        send.send_warning("Unable to check for updates, missing GITHUB_REPOSITORY_URL");
        return;
    };

    let current_version = schema::forge::VersionFragment::string_to_parts(version);

    let url = format!("{repository_url}/releases/latest/download/update_{}.json", std::env::consts::OS);
    let response = http_client.get(url).send().await;

    let response = match response {
        Ok(response) => response,
        Err(err) => {
            log::error!("Error while requesting update manifest: {}", err);
            send.send_error("Unable to fetch Pandora update manifest, see logs for more details");
            return;
        },
    };

    if response.status() != StatusCode::OK {
        send.send_error(format!("Unable to fetch Pandora update manifest, non-200 status code: {}", response.status()));
        return;
    }

    let manifest_bytes = match response.bytes().await {
        Ok(manifest_bytes) => manifest_bytes,
        Err(err) => {
            log::error!("Error while downloading update manifest: {}", err);
            send.send_error("Unable to download Pandora update manifest, see logs for more details");
            return;
        },
    };

    let manifest = match serde_json::from_slice::<UpdateManifest>(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            log::error!("Error while parsing update manifest: {}", err);
            send.send_error("Unable to parse update manifest, see logs for more details");
            return;
        },
    };

    let update_version = schema::forge::VersionFragment::string_to_parts(&manifest.version);

    if current_version >= update_version {
        log::info!("Pandora is up-to-date");
        return;
    }

    let exes = if let Some(universal) = manifest.downloads.archs.get("universal") {
        universal
    } else if let Some(exes) = manifest.downloads.archs.get(std::env::consts::ARCH) {
        exes
    } else {
        log::warn!("Unable to update, can't find arch \"{}\" in {:?}", std::env::consts::ARCH, manifest.downloads.archs.keys());
        return;
    };

    let Some(install_type) = determine_update_install_type() else {
        log::warn!("Unable to update, can't determine installation type");
        return;
    };

    let install_type_key = install_type.key();
    let Some(executable) = exes.exes.get(install_type_key) else {
        log::warn!("Unable to update, installation type \"{}\" not in {:?}", install_type_key, exes.exes.keys());
        return;
    };

    send.send(MessageToFrontend::UpdateAvailable {
        update: UpdatePrompt {
            old_version: version.into(),
            new_version: manifest.version.clone(),
            install_type,
            exe: executable.clone(),
        }
    });
}

fn determine_update_install_type() -> Option<UpdateInstallType> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Some(UpdateInstallType::AppImage(appimage.into()));
    }

    let current_exe = std::env::current_exe().ok()?;

    if cfg!(target_os = "macos") && let Some(app) = determine_macos_app_path(&current_exe) {
        return Some(UpdateInstallType::App(app.to_path_buf()));
    }

    return Some(UpdateInstallType::Executable);
}

fn determine_macos_app_path(current_exe: &Path) -> Option<&Path> {
    let parent = current_exe.parent()?;

    if parent.file_name()? != OsStr::new("MacOS") {
        return None;
    }

    let parent2 = parent.parent()?;

    if parent2.file_name()? != OsStr::new("Contents") {
        return None;
    }

    let parent3 = parent2.parent()?;

    if parent3.extension()? != OsStr::new("app") {
        return None;
    }

    Some(parent3)
}

pub async fn install_update(http_client: reqwest::Client, dirs: Arc<LauncherDirectories>, send: FrontendHandle, update: UpdatePrompt, modal_action: ModalAction) {
    if let Err(error) = install_update_inner(http_client, &dirs, send.clone(), update, modal_action.clone()).await {
        modal_action.set_error_message(error);
    }

    modal_action.set_finished();
    send.send(MessageToFrontend::Refresh);
}

async fn install_update_inner(http_client: reqwest::Client, dirs: &LauncherDirectories, send: FrontendHandle, update: UpdatePrompt, modal_action: ModalAction) -> Result<(), Arc<str>> {
    let title = format!("Downloading Pandora {}", update.new_version);
    let tracker = ProgressTracker::new(title.into(), send.clone());
    modal_action.trackers.push(tracker.clone());

    let mut expected_hash = [0u8; 20];
    let Ok(_) = hex::decode_to_slice(&*update.exe.sha1, &mut expected_hash) else {
        return Err("Unable to decode sha1 hash".into());
    };

    let Ok(response) = http_client.get(&*update.exe.download).send().await else {
        return Err("Error making download request".into());
    };

    if response.status() != StatusCode::OK {
        return Err("Download URL returned non-200 status code".into());
    }

    tracker.set_total(update.exe.size);
    tracker.notify();

    use futures::StreamExt;
    let mut stream = response.bytes_stream();

    let mut bytes = Vec::new();

    while let Some(item) = stream.next().await {
        let Ok(item) = item else {
            return Err("Error while downloading update".into());
        };

        bytes.extend_from_slice(&*item);
        tracker.add_count(item.len());
        tracker.notify();
    }

    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let actual_hash = hasher.finalize();

    if expected_hash != *actual_hash {
        return Err("Hash of downloaded file does not match".into());
    }

    let Some(pubkey) = option_env!("PANDORA_UPDATE_PUBKEY") else {
        return Err("Unable to update, missing PANDORA_UPDATE_PUBKEY at compile time".into());
    };

    let pubkey = base64::engine::general_purpose::STANDARD.decode(pubkey).unwrap();
    let sig = base64::engine::general_purpose::STANDARD.decode(&*update.exe.sig).unwrap();

    let pk = minisign_verify::PublicKey::decode(std::str::from_utf8(&pubkey).unwrap()).unwrap();
    let signature = minisign_verify::Signature::decode(std::str::from_utf8(&sig).unwrap()).unwrap();

    match pk.verify(&bytes, &signature, false) {
        Err(minisign_verify::Error::InvalidSignature) => {
            return Err("Invalid signature, file was not properly signed".into());
        },
        Err(err) => {
            return Err(format!("Error while validating signature: {:?}", err).into());
        },
        Ok(_) => {}
    }

    match update.install_type {
        UpdateInstallType::AppImage(appimage) => {
            let Some(filename) = appimage.file_name() else {
                return Err("Appimage path has no filename".into());
            };

            let new_filename = replace_os_str(filename, &update.old_version, &update.new_version);
            let new_appimage = appimage.with_file_name(new_filename);

            write_new_exe(appimage, new_appimage, &bytes, dirs)?;
        },
        UpdateInstallType::Executable => {
            let Ok(current_exe) = std::env::current_exe() else {
                return Err("Unable to determine current exe path".into());
            };

            let Some(filename) = current_exe.file_name() else {
                return Err("Current exe path has no filename".into());
            };

            let new_filename = replace_os_str(filename, &update.old_version, &update.new_version);
            let new_exe = current_exe.with_file_name(new_filename);

            write_new_exe(current_exe, new_exe, &bytes, dirs)?;
        },
        UpdateInstallType::App(current_app_folder) => {
            let mut temp_extract = dirs.temp_dir.join(format!("app_unpack_{}", rand::thread_rng().next_u64()));
            while temp_extract.exists() {
                log::warn!("Randomly generated app_unpack folder exists... what are the chances? ({:?})", temp_extract);
                temp_extract = dirs.temp_dir.join(format!("app_unpack_{}", rand::thread_rng().next_u64()));
            }

            let mut temp_backup = dirs.temp_dir.join(format!("app_backup_{}", rand::thread_rng().next_u64()));
            while temp_backup.exists() {
                log::warn!("Randomly generated app_backup folder exists... what are the chances? ({:?})", temp_backup);
                temp_backup = dirs.temp_dir.join(format!("app_backup_{}", rand::thread_rng().next_u64()));
            }

            let result = install_app_update(current_app_folder, &bytes, &temp_extract, &temp_backup);

            _ = std::fs::remove_dir_all(temp_backup);
            _ = std::fs::remove_dir_all(temp_extract);

            if let Err(err) = result {
                return Err(err);
            }
        },
    }

    send.send_success("Pandora update successful. Restart to apply changes");

    Ok(())
}

fn write_new_exe(old_exe: PathBuf, new_exe: PathBuf, data: &[u8], dirs: &LauncherDirectories) -> Result<(), String> {
    let mut new_exe_data = dirs.temp_dir.join(format!("new_exe_data_{}", rand::thread_rng().next_u64()));
    while new_exe_data.exists() {
        log::warn!("Randomly generated new_exe_data file exists... what are the chances? ({:?})", new_exe_data);
        new_exe_data = dirs.temp_dir.join(format!("new_exe_data_{}", rand::thread_rng().next_u64()));
    }

    if let Err(err) = std::fs::write(&new_exe_data, data) {
        log::error!("Error while writing new executable: {}", err);
        return Err("Error while writing new executable, see logs for more details".into());
    }

    let result = move_new_exe_into(old_exe, new_exe, &new_exe_data);

    _ = std::fs::remove_file(new_exe_data);

    result
}

fn move_new_exe_into(old_exe_path: PathBuf, new_exe_path: PathBuf, new_exe_data: &Path) -> Result<(), String> {
    let old_exe_path = old_exe_path.canonicalize().unwrap_or(old_exe_path);
    let new_exe_path = new_exe_path.canonicalize().unwrap_or(new_exe_path);

    if let Err(err) = std::fs::rename(&new_exe_data, &new_exe_path) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            // Runas doesn't support gui elevation on linux, so we just show an error
            #[cfg(target_os = "linux")]
            return Err("Unable to update executable file: permission denied".into());
            #[cfg(not(target_os = "linux"))]
            {
                #[cfg(unix)]
                let result = {
                    let mut command = OsString::new();
                    command.push("mv -f '");
                    command.push(new_exe_data.as_os_str());
                    command.push("' '");
                    command.push(new_exe_path.as_os_str());
                    command.push("' && chmod +x '");
                    command.push(new_exe_path.as_os_str());

                    if old_exe_path == new_exe_path {
                        command.push("'");
                    } else {
                        command.push("' && rm -f '");
                        command.push(old_exe_path.as_os_str());
                        command.push("'");
                    }

                    runas::Command::new("sh").arg("-c").arg(command).gui(true).status()
                };
                #[cfg(target_os = "windows")]
                let result = {
                    let mut command = OsString::new();
                    command.push("Move-Item -Path '");
                    command.push(new_exe_data.as_os_str());
                    command.push("' -Destination '");
                    command.push(new_exe_path.as_os_str());

                    if old_exe_path == new_exe_path {
                        command.push("' -Force");
                    } else {
                        command.push("' -Force; if ($?) { Remove-Item -Path '");
                        command.push(old_exe_path.as_os_str());
                        command.push("' }");
                    }

                    log::info!("{}", command.to_string_lossy());

                    runas::Command::new("powershell.exe")
                        .arg("-Command")
                        .arg(command)
                        .gui(true)
                        .status()
                };

                match result {
                    Ok(status) if status.success() => {
                        return Ok(())
                    },
                    Ok(status) => {
                        log::error!("Error completing elevated executable install: {}", status);
                        return Err("Error completing elevated executable installation, see logs for more details".into());
                    },
                    Err(err) => {
                        log::error!("Error completing elevated executable install: {}", err);
                        return Err("Error completing elevated executable installation, see logs for more details".into());
                    },
                }
            }
        }

        return Err(format!("Error while updating executable file: {:?}", err).into());
    }

    if old_exe_path != new_exe_path {
        _ = std::fs::remove_file(&old_exe_path);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        _ = std::fs::set_permissions(&new_exe_path, std::fs::Permissions::from_mode(0o755));
    }

    Ok(())
}

fn install_app_update(current_app_folder: PathBuf, bytes: &[u8], temp_extract: &Path, temp_backup: &Path) -> Result<(), Arc<str>> {
    let gz_decoder = flate2::bufread::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(gz_decoder);

    if let Err(err) = archive.unpack(&temp_extract) {
        log::error!("Unable to unpack .app.tar.gz: {}", err);
        return Err("Error while unpacking .app.tar.gz archive, see logs for more details".into());
    }

    let app_dir = match find_child_with_extension(&temp_extract, OsStr::new("app")) {
        Ok(None) => {
            return Err("Unable to find .app folder in extracted archive".into());
        },
        Err(err) => {
            log::error!("Unable to find .app folder: {}", err);
            return Err("I/O error while finding .app folder, see logs for more details".into());
        },
        Ok(Some(app_dir)) => app_dir,
    };

    // Backup current .app folder
    let needs_authorization = match std::fs::rename(&current_app_folder, &temp_backup) {
        Ok(_) => false,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => true,
        Err(err) => {
            log::error!("Unable to backup current .app: {}", err);
            return Err("I/O error while backing up current .app, see logs for more details".into());
        }
    };

    if temp_backup.exists() {
        return Err("Rename from current .app to temp backup errored, but then succeeded".into());
    }

    if needs_authorization {
        // Move current -> backup, then app -> current in single elevated command
        let mut command = OsString::new();
        command.push("mv -f '");
        command.push(current_app_folder.as_os_str());
        command.push("' '");
        command.push(temp_backup.as_os_str());
        command.push("' && mv -f '");
        command.push(app_dir.as_os_str());
        command.push("' '");
        command.push(current_app_folder.as_os_str());
        command.push("'");

        let result = runas::Command::new("sh").arg("-c").arg(command).gui(true).status();

        let success = match result {
            Ok(status) if status.success() => true,
            Ok(status) => {
                log::error!("Error completing elevated .app install: {}", status);
                false
            },
            Err(err) => {
                log::error!("Error completing elevated .app install: {}", err);
                false
            },
        };

        if !success {
            if temp_backup.exists() {
                let mut command = OsString::new();
                command.push("mv -f '");
                command.push(temp_backup.as_os_str());
                command.push("' '");
                command.push(current_app_folder.as_os_str());
                command.push("'");

                _ = runas::Command::new("sh").arg("-c").arg(command).gui(true).status();
            }

            return Err("Error completing elevated .app installation, see logs for more details".into());
        }
    } else {
        if let Err(err) = std::fs::rename(&app_dir, &current_app_folder) {
            _ = std::fs::rename(&temp_backup, &current_app_folder);
            log::error!("Error renaming new .app to old .app: {}", err);
            return Err("Error completing elevated .app installation, see logs for more details".into());
        }
    }

    Ok(())
}

fn replace_os_str(input: &OsStr, from: &str, to: &str) -> OsString {
    let encoded = input.as_encoded_bytes();

    let from_bytes = from.as_bytes();
    let to_bytes = to.as_bytes();

    let mut new_encoded = Vec::new();
    let mut index = 0;
    while index < encoded.len() {
        if encoded[index..].starts_with(from_bytes) {
            new_encoded.extend_from_slice(to_bytes);
            index += from_bytes.len();
        } else {
            new_encoded.push(encoded[index]);
            index += 1;
        }
    }

    // SAFETY: We construct new_encoded from a mixture of encoded and valid utf-8
    unsafe { OsString::from_encoded_bytes_unchecked(new_encoded) }
}

fn find_child_with_extension(folder: &Path, extension: &OsStr) -> std::io::Result<Option<PathBuf>> {
    let read_dir = std::fs::read_dir(folder)?;

    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if path.extension() == Some(extension) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}
