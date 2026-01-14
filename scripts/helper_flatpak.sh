set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}

mkdir -p dist
echo '#!/bin/sh' > dist/PandoraLauncher-Linux # dummy file, will build during flatpak
chmod +x dist/PandoraLauncher-Linux

#cargo install cargo-packager
cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "formats": ["deb"],'\
'  "productName": "Pandora Launcher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.PandoraLauncher",'\
'  "resources": [],'\
'  "binaries": [{ "path": "PandoraLauncher-Linux", "main": true }],'\
'  "icons": ["package/icon_256x256.png"]'\
'}'

cd dist
bsdtar -xf *.deb
