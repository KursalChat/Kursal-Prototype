#!/bin/bash

# security delete-generic-password -l "kursal" 2>/dev/null || true
rm -rf ~/Library/Caches/Kursal

cd kursal-tauri

if [[ "$1" == --dev-mode=* ]]; then
    DEV_ID="${1#--dev-mode=}"

    rm -rf ~/Library/Application\ Support/chat.kursal/"$DEV_ID".db
    rm -rf ~/Library/Application\ Support/chat.kursal/"$DEV_ID".key

    RUST_LOG=info bun run tauri dev -- -- --database-id="$DEV_ID" --unsafe-write-key-to-file
else
    rm -rf ~/Library/Application\ Support/chat.kursal
    RUST_LOG=info bun run tauri dev
fi