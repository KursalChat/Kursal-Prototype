cargo fmt -p kursal-core -p kursal-app -p kursal-cli
cargo clippy -p kursal-core -p kursal-app -p kursal-cli

cd kursal-tauri
bun run check
cd ..


./bin/test-mac.sh