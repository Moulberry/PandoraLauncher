#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}
export PANDORA_RELEASE_VERSION=$version

cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

strip target/aarch64-apple-darwin/release/pandora_launcher
strip target/x86_64-apple-darwin/release/pandora_launcher

mkdir -p dist

lipo -create -output dist/PandoraLauncher-macOS target/x86_64-apple-darwin/release/pandora_launcher target/aarch64-apple-darwin/release/pandora_launcher

cargo install cargo-packager
env -u CARGO_PACKAGER_SIGN_PRIVATE_KEY cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "formats": ["dmg", "app"],'\
'  "productName": "PandoraLauncher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "binaries": [{ "path": "PandoraLauncher-macOS", "main": true }],'\
'  "icons": ["package/mac.icns"]'\
'}'

mv -f dist/PandoraLauncher-macOS dist/PandoraLauncher-macOS-$version-Universal-Portable
mv -f dist/PandoraLauncher*.dmg dist/PandoraLauncher-$version.dmg
tar -czf dist/PandoraLauncher.app.tar.gz dist/PandoraLauncher.app
rm -r dist/PandoraLauncher.app

if [[ -n "$CARGO_PACKAGER_SIGN_PRIVATE_KEY" ]]; then
    cargo packager signer sign dist/PandoraLauncher-macOS-$version-Universal-Portable
    cargo packager signer sign dist/PandoraLauncher-$version.dmg
    cargo packager signer sign dist/PandoraLauncher.app.tar.gz

    echo "{
    \"version\": \"$version\",
    \"downloads\": {
        \"universal\": {
            \"executable\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher-macOS-$version-Universal-Portable\",
                \"size\": $(wc -c < dist/PandoraLauncher-macOS-$version-Universal-Portable),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher-macOS-$version-Universal-Portable | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher-macOS-$version-Universal-Portable.sig)\"
            },
            \"app\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher.app.tar.gz\",
                \"size\": $(wc -c < dist/PandoraLauncher.app.tar.gz),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher.app.tar.gz | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher.app.tar.gz.sig)\"
            }
        }
    }
}" > dist/update_macos.json

    rm dist/*.sig
fi
