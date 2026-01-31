set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}

sudo apt-get update --yes && sudo apt-get install --yes libssl-dev libdbus-1-dev libx11-xcb1 libxkbcommon-x11-dev pkg-config
cargo build --release --target x86_64-unknown-linux-gnu
strip target/x86_64-unknown-linux-gnu/release/pandora_launcher
mkdir -p dist
mv target/x86_64-unknown-linux-gnu/release/pandora_launcher dist/PandoraLauncher-Linux

cargo install cargo-packager
cargo packager --config '{'\
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

mv dist/PandoraLauncher-Linux dist/PandoraLauncher-Linux-$version-x86_64-Portable
