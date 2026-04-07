#!/bin/bash

security delete-generic-password -l "kursal" 2>/dev/null || true
rm -rf ~/Library/Caches/Kursal

cd kursal-core
cargo test --lib -- --test-threads=1 --nocapture