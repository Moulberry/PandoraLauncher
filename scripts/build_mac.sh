set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}

cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

strip target/aarch64-apple-darwin/release/pandora_launcher
strip target/x86_64-apple-darwin/release/pandora_launcher

mkdir -p dist

lipo -create -output dist/PandoraLauncher-macOS target/x86_64-apple-darwin/release/pandora_launcher target/aarch64-apple-darwin/release/pandora_launcher

cargo install cargo-packager
cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "formats": ["dmg"],'\
'  "productName": "Pandora Launcher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "binaries": [{ "path": "PandoraLauncher-macOS", "main": true }],'\
'  "icons": ["package/mac.icns"]'\
'}'

mv dist/PandoraLauncher-macOS dist/PandoraLauncher-macOS-$version-Universal
