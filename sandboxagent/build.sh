#!/bin/sh

cd "$(dirname "$0")"
rm com/modrinth/sandboxagent/SandboxAgent.class

JAVAC_VERSION=$(javac -version 2>&1 | head -n 1 | cut -d' ' -f2)

if [[ "$JAVAC_VERSION" != "1.8."* ]]; then
    echo "Must use Java 1.8 javac, got $JAVAC_VERSION"
    javac -version
    exit 1
fi

javac com/modrinth/sandboxagent/SandboxAgent.java
jar cvfm SandboxAgent.jar manifest.txt com/modrinth/sandboxagent/SandboxAgent.class
