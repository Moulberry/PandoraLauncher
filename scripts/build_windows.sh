set -e

if [ -z "$1" ]; then
    echo "Missing version argument"
    exit 1
fi

version=${1#v}

cargo build --release --target x86_64-pc-windows-msvc
strip target/x86_64-pc-windows-msvc/release/pandora_launcher.exe

mkdir -p dist

mv target/x86_64-pc-windows-msvc/release/pandora_launcher dist/PandoraLauncher-Windows.exe

cargo install cargo-packager
cargo packager --config '{'\
'  "name": "pandora-launcher",'\
'  "outDir": "./dist",'\
'  "productName": "Pandora Launcher",'\
'  "version": "'"$version"'",'\
'  "identifier": "com.moulberry.pandoralauncher",'\
'  "resources": [],'\
'  "binaries": [{ "path": "PandoraLauncher-Windows.exe", "main": true }],'\
'  "icons": ["package/windows.ico"]'\
'}'

mv dist/PandoraLauncher-Windows.exe dist/PandoraLauncher-Windows-$version-x86_64.exe
