#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}
export PANDORA_RELEASE_VERSION=$version

cargo build --release --frozen --target aarch64-apple-darwin
cargo build --release --frozen --target x86_64-apple-darwin

strip target/aarch64-apple-darwin/release/pandora_launcher
strip target/x86_64-apple-darwin/release/pandora_launcher

mkdir -p dist

lipo -create -output dist/PandoraLauncher-macOS-Universal target/x86_64-apple-darwin/release/pandora_launcher target/aarch64-apple-darwin/release/pandora_launcher

cargo install cargo-packager
env -u CARGO_PACKAGER_SIGN_PRIVATE_KEY cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "formats": ["dmg", "app"],'\
'  "productName": "PandoraLauncher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "authors": ["Moulberry"],'\
'  "binaries": [{ "path": "PandoraLauncher-macOS-Universal", "main": true }],'\
'  "icons": ["package/mac.icns"],'\
'  "macos": {"entitlements": "package/mac/entitlements.plist", "infoPlistPath": "package/mac/Info.plist"}'\
'}'

mv -f dist/PandoraLauncher-macOS-Universal dist/PandoraLauncher-macOS-Universal-Portable
mv -f dist/PandoraLauncher*.dmg dist/PandoraLauncher.dmg
tar -czf dist/PandoraLauncher.app.tar.gz dist/PandoraLauncher.app
rm -r dist/PandoraLauncher.app

if [[ -n "$CARGO_PACKAGER_SIGN_PRIVATE_KEY" ]]; then
    cargo packager signer sign dist/PandoraLauncher-macOS-Universal-Portable
    cargo packager signer sign dist/PandoraLauncher.dmg
    cargo packager signer sign dist/PandoraLauncher.app.tar.gz

    echo "{
    \"version\": \"$version\",
    \"downloads\": {
        \"universal\": {
            \"executable\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher-macOS-Universal-Portable\",
                \"size\": $(wc -c < dist/PandoraLauncher-macOS-Universal-Portable),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher-macOS-Universal-Portable | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher-macOS-Universal-Portable.sig)\"
            },
            \"app\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher.app.tar.gz\",
                \"size\": $(wc -c < dist/PandoraLauncher.app.tar.gz),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher.app.tar.gz | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher.app.tar.gz.sig)\"
            }
        }
    }
}" > dist/update_manifest_macos.json

    rm dist/*.sig
fi
