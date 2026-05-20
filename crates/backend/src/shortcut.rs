use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
fn shortcut_extension() -> &'static str {
    "desktop"
}

#[cfg(target_os = "windows")]
fn shortcut_extension() -> &'static str {
    "lnk"
}

#[cfg(target_os = "macos")]
fn shortcut_extension() -> &'static str {
    "app"
}

fn normalize_shortcut_path(mut path: PathBuf) -> PathBuf {
    let extension = shortcut_extension();
    if !has_extension(&path, extension) {
        path.add_extension(extension);
    }
    path
}

pub fn known_shortcut_filenames(instance_name: &str) -> [String; 2] {
    [
        format!("{instance_name}.{}", shortcut_extension()),
        format!("Launch {instance_name}.{}", shortcut_extension()),
    ]
}

fn maybe_rename_shortcut_path(path: &Path, old_instance_name: &str, new_instance_name: &str) -> PathBuf {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return path.to_path_buf();
    };

    let replacement = if stem == old_instance_name {
        Some(new_instance_name.to_string())
    } else if stem == format!("Launch {old_instance_name}") {
        Some(format!("Launch {new_instance_name}"))
    } else {
        None
    };

    let Some(replacement) = replacement else {
        return path.to_path_buf();
    };

    path.with_file_name(format!("{replacement}.{}", shortcut_extension()))
}

#[cfg(target_os = "linux")]
pub fn create_shortcut(path: PathBuf, name: &str, bin: &Path, args: &[&str]) -> Option<PathBuf> {
    let path = normalize_shortcut_path(path);
    log::info!("Creating linux shortcut at {:?}", path);

    let Some(bin) = bin.to_str() else {
        return None;
    };
    let exec = shell_words::join(std::iter::once(bin).chain(args.iter().map(|s| *s)));

    _ = std::fs::write(
        &path,
        format!(
            r#"[Desktop Entry]
Type=Application
Version=1.0
Name={name}
Exec={exec}
Categories=Games;Minecraft;Launcher;
"#
        )
        .as_bytes(),
    );

    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    Some(path)
}

#[cfg(target_os = "windows")]
pub fn create_shortcut(path: PathBuf, name: &str, bin: &Path, args: &[&str]) -> Option<PathBuf> {
    let path = normalize_shortcut_path(path);
    log::info!("Creating windows shortcut at {:?}", path);

    let Ok(mut sl) = mslnk::ShellLink::new(bin) else {
        return None;
    };
    let args_str = crate::join_windows_shell(args);
    sl.set_arguments(Some(args_str));
    sl.set_name(Some(name.into()));
    _ = sl.create_lnk(&path);
    Some(path)
}

#[cfg(target_os = "macos")]
pub fn create_shortcut(path: PathBuf, name: &str, bin: &Path, args: &[&str]) -> Option<PathBuf> {
    let path = normalize_shortcut_path(path);
    log::info!("Creating macos shortcut at {:?}", path);

    let contents = path.join("Contents");

    _ = std::fs::create_dir_all(&contents);

    let info_plist = contents.join("Info.plist");
    _ = std::fs::write(
        &info_plist,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>run.sh</string>
    <key>CFBundleIdentifier</key>
    <string>com.moulberry.pandoralauncher.Shortcut</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>MacOSX</string>
    </array>
    <key>CFBundleVersion</key>
    <string>0</string>
</dict>
</plist>"#
        )
        .as_bytes(),
    );

    let macos = contents.join("MacOS");
    _ = std::fs::create_dir_all(&macos);

    let Some(bin) = bin.to_str() else {
        return None;
    };
    let exec = shell_words::join(std::iter::once(bin).chain(args.iter().map(|s| *s)));

    let script_path = macos.join("run.sh");
    _ = std::fs::write(
        &script_path,
        format!(
            r#"#!/bin/sh
{}"#,
            exec
        )
        .as_bytes(),
    );

    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
    Some(path)
}

pub fn update_shortcut(
    path: PathBuf,
    old_instance_name: &str,
    new_instance_name: &str,
    bin: &Path,
    args: &[&str],
) -> Option<PathBuf> {
    let old_path = normalize_shortcut_path(path);
    let new_path = maybe_rename_shortcut_path(&old_path, old_instance_name, new_instance_name);

    if old_path != new_path && old_path.exists() && !new_path.exists() {
        let _ = std::fs::rename(&old_path, &new_path);
    }

    create_shortcut(new_path, &format!("Launch {new_instance_name}"), bin, args)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    let Some(path_extension) = path.extension() else {
        return false;
    };

    path_extension.as_encoded_bytes() == extension.as_bytes()
}
