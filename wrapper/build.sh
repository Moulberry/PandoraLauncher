#!/bin/sh

cd "$(dirname "$0")"
rm -f com/moulberry/pandora/LaunchWrapper.class

javac --release 8 com/moulberry/pandora/LaunchWrapper.java
jar cvfm LaunchWrapper.jar manifest.txt com/moulberry/pandora/LaunchWrapper.class
