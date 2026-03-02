#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}
export PANDORA_RELEASE_VERSION=$version

cargo build --release --target x86_64-pc-windows-msvc
strip target/x86_64-pc-windows-msvc/release/pandora_launcher.exe

mkdir -p dist

mv target/x86_64-pc-windows-msvc/release/pandora_launcher dist/PandoraLauncher-Windows-x86_64.exe

cargo install cargo-packager
env -u CARGO_PACKAGER_SIGN_PRIVATE_KEY cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "productName": "Pandora Launcher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "authors": ["Moulberry"],'\
'  "binaries": [{ "path": "PandoraLauncher-Windows-x86_64.exe", "main": true }],'\
'  "icons": ["package/windows.ico"]'\
'}'

mv -f dist/PandoraLauncher-Windows-x86_64.exe dist/PandoraLauncher-Windows-x86_64-Portable.exe
mv -f 'dist/PandoraLauncher-Windows-x86_64_'$version'_x64-setup.exe' dist/PandoraLauncher-Windows-x86_64-Setup.exe

if [[ -n "$CARGO_PACKAGER_SIGN_PRIVATE_KEY" ]]; then
    cargo packager signer sign dist/PandoraLauncher-Windows-x86_64-Portable.exe

    echo "{
    \"version\": \"$version\",
    \"downloads\": {
        \"x86_64\": {
            \"executable\": {
                \"download\": \"https://github.com/Moulberry/PandoraLauncher/releases/download/v$version/PandoraLauncher-Windows-x86_64-Portable.exe\",
                \"size\": $(wc -c < dist/PandoraLauncher-Windows-x86_64-Portable.exe),
                \"sha1\": \"$(sha1sum dist/PandoraLauncher-Windows-x86_64-Portable.exe | cut -d ' ' -f 1)\",
                \"sig\": \"$(cat dist/PandoraLauncher-Windows-x86_64-Portable.exe.sig)\"
            }
        }
    }
}" > dist/update_manifest_windows.json

    rm dist/*.sig
fi
