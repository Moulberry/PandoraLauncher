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

    let url = format!("{repository_url}/releases/latest/download/update_manifest_{}.json", std::env::consts::OS);
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

            // This is temporary to address pre-3.3.0 including the version inside the filename
            // This should be removed at some point in the near future
            let new_filename = replace_os_str(filename, &format!("-{}", &update.old_version), "");
            let new_appimage = appimage.with_file_name(new_filename);

            replace_exe(appimage, new_appimage, &bytes, dirs)?;
        },
        UpdateInstallType::Executable => {
            let Ok(current_exe) = std::env::current_exe() else {
                return Err("Unable to determine current exe path".into());
            };

            let Some(filename) = current_exe.file_name() else {
                return Err("Current exe path has no filename".into());
            };

            // This is temporary to address pre-3.3.0 including the version inside the filename
            // This should be removed at some point in the near future
            let new_filename = replace_os_str(filename, &format!("-{}", &update.old_version), "");
            let new_exe = current_exe.with_file_name(new_filename);

            replace_exe(current_exe, new_exe, &bytes, dirs)?;
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

fn add_new_extension(path: &Path) -> PathBuf {
    let mut new_exe_data = path.with_added_extension(format!("{}.new", rand::thread_rng().next_u64()));
    while new_exe_data.exists() {
        log::warn!("Randomly generated new_exe_data file exists... what are the chances? ({:?})", new_exe_data);
        new_exe_data = path.with_added_extension(format!("{}.new", rand::thread_rng().next_u64()));
    }
    return new_exe_data;
}

fn write_new_exe_temp(new_exe: &Path, data: &[u8], dirs: &LauncherDirectories) -> Result<PathBuf, String> {
    let new_exe_data = add_new_extension(new_exe);

    let Err(err) = std::fs::write(&new_exe_data, data) else {
        return Ok(new_exe_data);
    };

    if err.kind() != std::io::ErrorKind::PermissionDenied {
        log::error!("Error while writing new executable: {}", err);
        return Err("Error while writing new executable, see logs for more details".into());
    }

    let new_exe_data = add_new_extension(&dirs.temp_dir.join("new_exe_data"));

    if let Err(err) = std::fs::write(&new_exe_data, data) {
        log::error!("Error while writing new executable: {}", err);
        return Err("Error while writing new executable, see logs for more details".into());
    }

    Ok(new_exe_data)
}

fn replace_exe(old_exe: PathBuf, new_exe: PathBuf, data: &[u8], dirs: &LauncherDirectories) -> Result<(), String> {
    let new_exe_temp = write_new_exe_temp(&new_exe, data, dirs)?;

    #[cfg(unix)]
    {
        let result = move_new_exe_into(old_exe, new_exe, &new_exe_temp);
        _ = std::fs::remove_file(new_exe_temp);
        return result;
    }
    #[cfg(windows)]
    {
        launch_update_helper(old_exe, new_exe, &new_exe_temp);
        return Ok(());
    }
}

fn try_canonicalize(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;

    if cfg!(windows) {
        let path_bytes = path.as_os_str().as_encoded_bytes();
        let canonical_bytes = canonical.as_os_str().as_encoded_bytes();
        if canonical_bytes.len() == path_bytes.len()+4
            && &canonical_bytes[..4] == b"\\\\?\\"
            && &canonical_bytes[4..] == path_bytes
        {
            None
        } else {
            Some(canonical)
        }
    } else {
        Some(canonical)
    }
}

// Windows doesn't like replacing currently running executables, so we spawn a powershell script that waits for the program to exit
#[cfg(windows)]
fn launch_update_helper(old_exe_path: PathBuf, new_exe_path: PathBuf, new_exe_data: &Path) {
    let mut ps_arguments: Vec<&OsStr> = Vec::new();

    let id_string = format!("{}", std::process::id());

    ps_arguments.push(OsStr::new("Write-Host"));
    ps_arguments.push(OsStr::new("Waiting for launcher to close..."));
    ps_arguments.push(OsStr::new(";"));

    ps_arguments.push(OsStr::new("Wait-Process"));
    ps_arguments.push(OsStr::new("-Id"));
    ps_arguments.push(OsStr::new(&id_string));
    ps_arguments.push(OsStr::new("-ErrorAction"));
    ps_arguments.push(OsStr::new("SilentlyContinue;"));

    ps_arguments.push(OsStr::new("if"));
    ps_arguments.push(OsStr::new("($?)"));
    ps_arguments.push(OsStr::new("{"));

    ps_arguments.push(OsStr::new("Move-Item"));
    ps_arguments.push(OsStr::new("-Path"));
    ps_arguments.push(new_exe_data.as_os_str());
    ps_arguments.push(OsStr::new("-Destination"));
    ps_arguments.push(new_exe_path.as_os_str());

    if old_exe_path == new_exe_path {
        ps_arguments.push(OsStr::new("-Force"));
    } else {
        ps_arguments.push(OsStr::new("-Force;"));
        ps_arguments.push(OsStr::new("if"));
        ps_arguments.push(OsStr::new("($?)"));
        ps_arguments.push(OsStr::new("{"));
        ps_arguments.push(OsStr::new("Remove-Item"));
        ps_arguments.push(OsStr::new("-Path"));
        ps_arguments.push(old_exe_path.as_os_str());
        ps_arguments.push(OsStr::new("-Force"));
        ps_arguments.push(OsStr::new("}"));
    }

    ps_arguments.push(OsStr::new("}"));

    let ps_command = crate::join_windows_shell_os(&ps_arguments);

    log::info!("Running with powershell.exe: {}", ps_command.to_string_lossy());

    std::process::Command::new("powershell.exe")
        .arg("-Command")
        .arg(ps_command)
        .spawn()
        .unwrap();
}

#[cfg(unix)]
fn move_new_exe_into(old_exe_path: PathBuf, new_exe_path: PathBuf, new_exe_data: &Path) -> Result<(), String> {
    let old_exe_path = try_canonicalize(&old_exe_path).unwrap_or(old_exe_path);
    let new_exe_path = try_canonicalize(&new_exe_path).unwrap_or(new_exe_path);

    if let Err(err) = std::fs::rename(&new_exe_data, &new_exe_path) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            log::info!("Permission denied while trying to update executable, need to elevate");

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

            // todo: replace runas with workspace command crate
            let result = if cfg!(target_os = "linux") {
                log::info!("Running with pkexec: {}", command.to_string_lossy());
                std::process::Command::new("pkexec").arg("sh").arg("-c").arg(command).status()
            } else {
                log::info!("Running with runas: {}", command.to_string_lossy());
                runas::Command::new("sh").arg("-c").arg(command).gui(true).status()
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

        return Err(format!("Error while updating executable file: {:?}", err).into());
    }

    if old_exe_path != new_exe_path {
        _ = std::fs::remove_file(&old_exe_path);
    }

    use std::os::unix::fs::PermissionsExt;
    _ = std::fs::set_permissions(&new_exe_path, std::fs::Permissions::from_mode(0o755));

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

    if needs_authorization && temp_backup.exists() {
        _ = std::fs::rename(&temp_backup, &current_app_folder);
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

#[cfg(windows)]
fn run_admin_powershell(script: &OsStr) -> std::process::ExitStatus {
    unsafe {
        let mut sei: windows::Win32::UI::Shell::SHELLEXECUTEINFOW = std::mem::zeroed();
        _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED |  windows::Win32::System::Com::COINIT_DISABLE_OLE1DDE,
        );

        use std::os::windows::ffi::OsStrExt;
        let encoded = crate::join_windows_shell_os(&[OsStr::new("-Command"), script]).encode_wide()
            .chain(OsStr::new("\0").encode_wide())
            .collect::<Vec<_>>();

        sei.fMask = windows::Win32::UI::Shell::SEE_MASK_NOASYNC | windows::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
        sei.cbSize = std::mem::size_of::<windows::Win32::UI::Shell::SHELLEXECUTEINFOW>() as _;
        sei.lpVerb = windows::core::w!("runas");
        sei.lpFile = windows::core::w!("powershell.exe");
        sei.lpParameters = windows::core::PCWSTR::from_raw(encoded.as_ptr());
        sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_NORMAL.0;

        if windows::Win32::UI::Shell::ShellExecuteExW(&mut sei).is_err() || sei.hProcess.is_invalid() {
            return std::mem::transmute(!0);
        }

        windows::Win32::System::Threading::WaitForSingleObject(sei.hProcess, windows::Win32::System::Threading::INFINITE);

        let mut code = 0;
        if windows::Win32::System::Threading::GetExitCodeProcess(sei.hProcess, &mut code).is_err() {
            std::mem::transmute(!0)
        } else {
            std::mem::transmute(code)
        }
    }
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
