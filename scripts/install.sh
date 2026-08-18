#!/usr/bin/env bash
# Screen Track — one-command installer for macOS.
#
# For end users: no Rust, no Node, no build tools. Downloads the prebuilt,
# self-contained app (AI model baked in), installs it to /Applications and
# launches it.
#
# Run it with a single line in Terminal:
#   curl -fsSL https://raw.githubusercontent.com/idhanth17/screen-track/master/scripts/install.sh | bash
#
# NOTE: a macOS release build (.dmg) must be published first — see the repo
# releases. Building the .dmg requires a Mac (`cd app && npx @tauri-apps/cli@2 build`).
set -euo pipefail

product="Screen Track"
# Apple-silicon build; swap to x64 on Intel Macs if that asset is published.
asset="https://github.com/idhanth17/screen-track/releases/latest/download/ScreenTrack-aarch64.dmg"
tmp="$(mktemp -d)"
dmg="$tmp/ScreenTrack.dmg"

echo "Downloading $product ..."
curl -fSL -o "$dmg" "$asset"

echo "Installing to /Applications ..."
mount_point="$(hdiutil attach "$dmg" -nobrowse -quiet | grep -o '/Volumes/.*' | head -1)"
cp -R "$mount_point/$product.app" "/Applications/" 2>/dev/null || \
  sudo cp -R "$mount_point/$product.app" "/Applications/"
hdiutil detach "$mount_point" -quiet || true
rm -rf "$tmp"

echo "Launching $product ..."
open "/Applications/$product.app"
echo ""
echo "$product is installed. Grant Accessibility permission when prompted"
echo "(System Settings -> Privacy & Security -> Accessibility) so it can track the foreground app."
