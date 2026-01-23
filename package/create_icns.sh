#!/bin/sh

set -e
cd "$(dirname "$0")"
mkdir mac.iconset

# based on https://gist.github.com/mark-ingenosity/6b9a29123e3df925e56029917cfb3f3a

# run inkscape from the command line to generate the iconset formatted for icns
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_16x16.png"      --export-width=16 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_16x16@2x.png"   --export-width=32 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_32x32.png"      --export-width=32 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_32x32@2x.png"   --export-width=64 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_128x128.png"    --export-width=128 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_128x128@2x.png" --export-width=256 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_256x256.png"    --export-width=256 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_256x256@2x.png" --export-width=512 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_512x512.png"    --export-width=512 "$PWD/mac.svg"
/Applications/Inkscape.app/Contents/MacOS/inkscape --export-filename="$PWD/mac.iconset/icon_512x512@2x.png" --export-width=1024 "$PWD/mac.svg"

# optimize pngs
for i in ./mac.iconset/*.png; do
    optipng -o7 "$i"
done

# run osx iconutil app to convert the iconset to icns format
iconutil -c icns "mac.iconset"

# delete folder
rm -R mac.iconset
