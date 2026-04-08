rm -rf ./build/Kursal*
rm -rf ./build/latest.json
mkdir -p build

# ── Signing key ────────────────────────────────────────────────────────────────
export TAURI_SIGNING_PRIVATE_KEY=$(cat ./keys/publishing.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(cat ./keys/publishing.key.pwd)

# ── Version ────────────────────────────────────────────────────────────────────
VERSION=$(grep '^version' ./Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "Building releases for v$VERSION"

cd kursal-tauri
rustup upgrade
bun install
bun run build


# ── WINDOWS (x64) ─────────────────────────────────────────────────────────────
rm -f ../target/x86_64-pc-windows-msvc/release/kursal-app.exe
rm -rf ../target/x86_64-pc-windows-msvc/release/bundle/

bun tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --config '{"build":{"beforeBuildCommand":""}}'

cp ../target/x86_64-pc-windows-msvc/release/kursal-app.exe ../build/Kursal_x64.exe
cp ../target/x86_64-pc-windows-msvc/release/bundle/nsis/Kursal_*_x64-setup.exe ../build/Kursal_x64-setup.exe
cp ../target/x86_64-pc-windows-msvc/release/bundle/nsis/Kursal_*_x64-setup.exe.sig ../build/Kursal_x64-setup.exe.sig

upx --best --lzma ../build/Kursal_x64.exe # always useful

WIN_X64_SIG=$(cat ../target/x86_64-pc-windows-msvc/release/bundle/nsis/Kursal_*_x64-setup.exe.sig)


# ── MAC (aarch64) ─────────────────────────────────────────────────────────────
rm -rf ../target/aarch64-apple-darwin/release/bundle/

bun tauri build --bundles app,dmg,updater --target aarch64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'

cp ../target/aarch64-apple-darwin/release/bundle/dmg/Kursal_*.dmg ../build/Kursal.dmg
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz ../build/Kursal.app.tar.gz
cp ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal.app.tar.gz.sig

MAC_ARM_SIG=$(cat ../target/aarch64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig)


# ── MAC (x86_64) ─────────────────────────────────────────────────────────────
rm -rf ../target/x86_64-apple-darwin/release/bundle

bun tauri build --bundles app,dmg,updater --target x86_64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'

cp ../target/x86_64-apple-darwin/release/bundle/dmg/Kursal_*.dmg ../build/Kursal_x64.dmg
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz ../build/Kursal_x64.app.tar.gz
cp ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig ../build/Kursal_x64.app.tar.gz.sig

MAC_X64_SIG=$(cat ../target/x86_64-apple-darwin/release/bundle/macos/Kursal.app.tar.gz.sig)


# ── LINUX (arm64) ─────────────────────────────────────────────────────────────
docker volume create kursal-cargo-registry-arm64
docker volume create kursal-cargo-target-arm64

_run_arm64() {
  docker run \
    --name kursal-linux-arm64 \
    --platform linux/arm64 \
    -v "$(PWD)/../":/workspace \
    -v kursal-cargo-registry-arm64:/root/.cargo/registry \
    -v kursal-cargo-target-arm64:/root/kursal-target \
    -w /workspace/kursal-tauri \
    ubuntu:24.04 bash -c '
      if [ ! -f /root/.setup-done ]; then
        echo "==> Running first-time setup..." &&
        apt-get update && apt-get install -y \
          curl wget file unzip build-essential \
          libssl-dev pkg-config \
          libgtk-3-dev libwebkit2gtk-4.1-dev \
          libayatana-appindicator3-dev librsvg2-dev \
          fuse libfuse2 squashfs-tools \
          protobuf-compiler xdg-utils &&
        curl https://sh.rustup.rs -sSf | sh -s -- -y &&
        curl -fsSL https://bun.sh/install | bash &&
        touch /root/.setup-done &&
        echo "==> Setup complete!"
      else
        echo "==> Setup already done, skipping."
      fi &&
      source ~/.cargo/env &&
      export PATH="$HOME/.bun/bin:$PATH" &&
      bun install &&
      rustup upgrade &&
      rm -rf /root/kursal-target/release/bundle/ &&
      export TAURI_SIGNING_PRIVATE_KEY=$(cat /workspace/keys/publishing.key) &&
      export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(cat /workspace/keys/publishing.key.pwd) &&
      export CARGO_TARGET_DIR=/root/kursal-target &&
      export APPIMAGE_EXTRACT_AND_RUN=1 &&
      bun run tauri build --bundles deb,rpm,appimage,updater --config '"'"'{"build":{"beforeBuildCommand":""}}'"'"' &&
      cp /root/kursal-target/release/bundle/deb/*.deb /workspace/build/Kursal_arm.deb &&
      cp /root/kursal-target/release/bundle/deb/*.deb.sig /workspace/build/Kursal_arm.deb.sig &&
      cp /root/kursal-target/release/bundle/rpm/*.rpm /workspace/build/Kursal_arm.rpm &&
      cp /root/kursal-target/release/bundle/rpm/*.rpm.sig /workspace/build/Kursal_arm.rpm.sig &&
      cp /root/kursal-target/release/bundle/appimage/*.AppImage /workspace/build/Kursal_arm.AppImage &&
      cp /root/kursal-target/release/bundle/appimage/*.AppImage.sig /workspace/build/Kursal_arm.AppImage.sig
    '
}

if docker container inspect kursal-linux-arm64 &>/dev/null; then
  docker start -ai kursal-linux-arm64
else
  _run_arm64
fi
docker container stop kursal-linux-arm64

LINUX_ARM_SIG=$(cat ../build/Kursal_arm.AppImage.sig)


# ── LINUX (x64) ───────────────────────────────────────────────────────────────
docker volume create kursal-cargo-registry-x64
docker volume create kursal-cargo-target-x64

_run_x64() {
  docker run \
    --name kursal-linux-x64 \
    --platform linux/amd64 \
    -v "$(PWD)/../":/workspace \
    -v kursal-cargo-registry-x64:/root/.cargo/registry \
    -v kursal-cargo-target-x64:/root/kursal-target \
    -w /workspace/kursal-tauri \
    ubuntu:24.04 bash -c '
      if [ ! -f /root/.setup-done ]; then
        echo "==> Running first-time setup..." &&
        apt-get update && apt-get install -y \
          curl wget file unzip build-essential \
          libssl-dev pkg-config \
          libgtk-3-dev libwebkit2gtk-4.1-dev \
          libayatana-appindicator3-dev librsvg2-dev \
          fuse libfuse2 squashfs-tools \
          protobuf-compiler xdg-utils &&
        curl https://sh.rustup.rs -sSf | sh -s -- -y &&
        curl -fsSL https://bun.sh/install | bash &&
        touch /root/.setup-done &&
        echo "==> Setup complete!"
      else
        echo "==> Setup already done, skipping."
      fi &&
      source ~/.cargo/env &&
      export PATH="$HOME/.bun/bin:$PATH" &&
      bun install &&
      rustup upgrade &&
      rm -rf /root/kursal-target/release/bundle/ &&
      export TAURI_SIGNING_PRIVATE_KEY=$(cat /workspace/keys/publishing.key) &&
      export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(cat /workspace/keys/publishing.key.pwd) &&
      export CARGO_TARGET_DIR=/root/kursal-target &&
      export APPIMAGE_EXTRACT_AND_RUN=1 &&
      bun run tauri build --bundles deb,rpm,appimage,updater --config '"'"'{"build":{"beforeBuildCommand":""}}'"'"' &&
      cp /root/kursal-target/release/bundle/deb/*.deb /workspace/build/Kursal_x64.deb &&
      cp /root/kursal-target/release/bundle/deb/*.deb.sig /workspace/build/Kursal_x64.deb.sig &&
      cp /root/kursal-target/release/bundle/rpm/*.rpm /workspace/build/Kursal_x64.rpm &&
      cp /root/kursal-target/release/bundle/rpm/*.rpm.sig /workspace/build/Kursal_x64.rpm.sig &&
      cp /root/kursal-target/release/bundle/appimage/*.AppImage /workspace/build/Kursal_x64.AppImage &&
      cp /root/kursal-target/release/bundle/appimage/*.AppImage.sig /workspace/build/Kursal_x64.AppImage.sig
    '
}

if docker container inspect kursal-linux-x64 &>/dev/null; then
  docker start -ai kursal-linux-x64
else
  _run_x64
fi
docker container stop kursal-linux-x64

LINUX_X64_SIG=$(cat ../build/Kursal_x64.AppImage.sig)


# ── ANDROID ───────────────────────────────────────────────────────────────────
rm -rf src-tauri/gen/android/app/build/outputs/apk/universal/release/

JAVA_HOME=/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home PATH=$JAVA_HOME/bin:$PATH bun tauri android build --apk --target aarch64 --config '{"build":{"beforeBuildCommand":""}}'
cp -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk ../build/Kursal.apk


# ── Generate latest.json ──────────────────────────────────────────────────────
BASE_URL="https://app.kursal.chat"

cat > ../build/latest.json <<EOF
{
  "version": "$VERSION",
  "notes": "",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "windows-x86_64": {
      "url": "$BASE_URL/Kursal_x64-setup.exe",
      "signature": "$WIN_X64_SIG"
    },
    "darwin-aarch64": {
      "url": "$BASE_URL/Kursal.app.tar.gz",
      "signature": "$MAC_ARM_SIG"
    },
    "darwin-x86_64": {
      "url": "$BASE_URL/Kursal_x64.app.tar.gz",
      "signature": "$MAC_X64_SIG"
    },
    "linux-aarch64": {
      "url": "$BASE_URL/Kursal_arm.AppImage",
      "signature": "$LINUX_ARM_SIG"
    },
    "linux-x86_64": {
      "url": "$BASE_URL/Kursal_x64.AppImage",
      "signature": "$LINUX_X64_SIG"
    }
  }
}
EOF

echo "✅ latest.json written for v$VERSION"
echo "=> make sure to write some release notes in it :p"


# TODO: iOS
# bun tauri ios build --open --config '{"build":{"beforeBuildCommand":""}}'

# and run this in parallel (pain) - https://github.com/tauri-apps/tauri/issues/14940
# xcodebuild archive \
#          -project gen/apple/kursal-app.xcodeproj \
#          -scheme kursal-app_iOS \
#          -archivePath gen/apple/build/App.xcarchive \
#          -configuration Release
#          CODE_SIGNING_REQUIRED=NO 
#          CODE_SIGNING_ALLOWED=NO

# mkdir -p ../build/Payload
# cp -r src-tauri/gen/apple/build/kursal-app_iOS.xcarchive/Products/Applications/Kursal.app ../build/Payload
# cd ../build
# zip -r Kursal.ipa Payload
# rm -rf Payload
# cd ../kursal-tauri