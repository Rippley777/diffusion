#!/bin/bash
set -euo pipefail

# Build a self-contained Finder application for the selected Mac architecture.
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
    echo "Run this script on macOS." >&2
    exit 1
fi
target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$target" in
    aarch64-apple-darwin) arch=arm64 ;;
    x86_64-apple-darwin) arch=x86_64 ;;
    *) echo "Expected aarch64-apple-darwin or x86_64-apple-darwin." >&2; exit 1 ;;
esac
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
cargo build --release --locked -p diffusion --target "$target"
output="$PWD/target/macos/$arch"
app="$output/Diffusion.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$target/release/diffusion" "$app/Contents/MacOS/diffusion"
cp LICENSE "$app/Contents/Resources/LICENSE"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>diffusion</string>
<key>CFBundleIdentifier</key><string>com.rippley.diffusion</string>
<key>CFBundleName</key><string>Diffusion</string>
<key>CFBundleDisplayName</key><string>Diffusion</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
archive="$output/Diffusion-$version-macos-$arch.zip"
ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
echo "Created $archive"
