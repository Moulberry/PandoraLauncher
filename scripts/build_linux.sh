#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}
export PANDORA_RELEASE_VERSION=$version

sudo apt-get update --yes && sudo apt-get install --yes libssl-dev libdbus-1-dev libx11-xcb1 libxkbcommon-x11-dev pkg-config
cargo build --release --target x86_64-unknown-linux-gnu
strip target/x86_64-unknown-linux-gnu/release/pandora_launcher
mkdir -p dist
mv target/x86_64-unknown-linux-gnu/release/pandora_launcher dist/PandoraLauncher-Linux

cargo install cargo-packager
env -u CARGO_PACKAGER_SIGN_PRIVATE_KEY cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "formats": ["deb", "appimage"],'\
'  "productName": "Pandora Launcher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "binaries": [{ "path": "PandoraLauncher-Linux", "main": true }],'\
'  "icons": ["package/windows_icons/icon_16x16.png", "package/windows_icons/icon_32x32.png", "package/windows_icons/icon_48x48.png", "package/windows_icons/icon_256x256.png"]'\
'}'

mv -f dist/PandoraLauncher-Linux dist/PandoraLauncher-Linux-$version-x86_64-Portable
mv -f 'dist/PandoraLauncher-Linux_'$version'_x86_64.AppImage' dist/PandoraLauncher-Linux-$version-x86_64.AppImage

if [[ -n "$CARGO_PACKAGER_SIGN_PRIVATE_KEY" ]]; then
    cargo packager signer sign dist/PandoraLauncher-Linux-$version-x86_64-Portable
    cargo packager signer sign dist/PandoraLauncher-Linux-$version-x86_64.AppImage

    echo "{
    \"version\": \"$version\",
    \"downloads\": {
        \"x86_64\": {
            \"executable\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher-Linux-$version-x86_64-Portable\",
                \"size\": $(wc -c < dist/PandoraLauncher-Linux-$version-x86_64-Portable),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher-Linux-$version-x86_64-Portable | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher-Linux-$version-x86_64-Portable.sig)\"
            },
            \"appimage\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher-Linux-$version-x86_64.AppImage\",
                \"size\": $(wc -c < dist/PandoraLauncher-Linux-$version-x86_64.AppImage),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher-Linux-$version-x86_64.AppImage | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher-Linux-$version-x86_64.AppImage.sig)\"
            }
        }
    }
}" > dist/update_linux.json

    rm dist/*.sig
fi
