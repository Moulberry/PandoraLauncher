#!/bin/sh

set -e
cd "$(dirname "$0")"
mkdir windows_icons

# based on https://gist.github.com/mark-ingenosity/6b9a29123e3df925e56029917cfb3f3a

/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/windows_icons/icon_16x16.png"      --export-width=16 "$PWD/windows.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/windows_icons/icon_32x32.png"      --export-width=32 "$PWD/windows.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/windows_icons/icon_48x48.png"      --export-width=48 "$PWD/windows.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/windows_icons/icon_256x256.png"    --export-width=256 "$PWD/windows.svg"

# optimize pngs
for i in ./windows_icons/*.png; do
    optipng -o7 "$i"
done

# use imagemagick to create ico file
convert $PWD/windows_icons/*.png windows.ico
