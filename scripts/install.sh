#!/usr/bin/env bash

load() {
    while true; do
        for char in '⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏'; do
            echo -ne "\r$char $1" >&2 
            sleep 0.08
        done
    done
}

install() { 
    local exec_dir="$HOME/.local/PandoraLauncher"
    local desktop_dir="$HOME/.local/share/applications"
    
    local exec_path="$exec_dir/pandora_launcher"
    local desktop_path="$desktop_dir/PandoraLauncher.desktop"
    
    load "Getting latest version..." &
    local PROCESO_PID=$!
    trap "kill $PROCESO_PID; echo; exit" SIGINT
    
    local platform=$(uname -m)
    [[ "$platform" == "x86_64" ]] || {
        kill $PROCESO_PID
        echo -ne "\033[1A\033[2K\c"
        echo -e "\r[Error] Unsuported platform '$platform'" >&2
        exit 1
    }
    
    local version=$(curl -s https://api.github.com/repos/Moulberry/PandoraLauncher/releases/latest | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')
    local nversion=$(echo $version | sed 's/v//')
    
    [[ -z "$version" ]] && {
        kill $PROCESO_PID
        echo -ne "\033[1A\033[2K\c"
        echo -e "\r[Error] Something went wrong getting the version" >&2
        exit 1
    }
    
    local download_url="https://github.com/Moulberry/PandoraLauncher/releases/download/$version/PandoraLauncher-Linux-$nversion-$platform"
    local icon_url="https://raw.githubusercontent.com/Moulberry/PandoraLauncher/refs/heads/master/assets/icons/pandora.svg"
    
    mkdir -p "$exec_dir" "$desktop_dir" 
    
    kill $PROCESO_PID
    echo -ne "\033[1A\033[2K\c"
    echo -e "\rVersion found: $version" >&2
    
    curl -L $download_url -o "$exec_path" || {
        echo 
        echo "[Error] Something went wrong downloading" >&2
        echo "-> $download_url" >&2
        exit 1
    }
    
    curl -L $icon_url -o "$exec_path.svg"  || {
        echo 
        echo "[Error] Something went wrong downloading" >&2
        echo "-> $icon_url" >&2
        exit 1
    }
    
    chmod +x "$exec_path" || exit 1
    
    printf "%s\n" "[Desktop Entry]
Version=1.0
Type=Application
Name=PandoraLauncher
TryExec=$exec_path
StartupNotify=true
Exec=$exec_path %U
Icon=$exec_path.svg
Categories=Utility;Games;
Keywords=Minecraft;" > "$desktop_path"
    
    update-desktop-database "$HOME/.local/share/applications"
    
    echo 
    echo "Installed!"
}

install
