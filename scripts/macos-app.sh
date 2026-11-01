#!/usr/bin/env bash
# Pack air-control into AirControl.app so macOS Dock / Launchpad show Frame 6.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PROFILE="${1:-debug}"
BIN="$ROOT/target/$PROFILE/air-control"
APP="$ROOT/target/$PROFILE/AirControl.app"
ICON_PNG="$ROOT/crates/air-control-app/ui/icon.png"
ICON_ICNS="$ROOT/crates/air-control-app/ui/AppIcon.icns"

if [[ ! -x "$BIN" ]]; then
  echo "missing binary: $BIN — run cargo build -p air-control-app first" >&2
  exit 1
fi

if [[ ! -f "$ICON_ICNS" ]]; then
  echo "building AppIcon.icns..."
  ICONSET="$(mktemp -d)/AppIcon.iconset"
  mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ICON_PNG" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    dbl=$((size * 2))
    if [[ "$dbl" -le 1024 ]]; then
      sips -z "$dbl" "$dbl" "$ICON_PNG" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
    fi
  done
  iconutil -c icns "$ICONSET" -o "$ICON_ICNS"
  rm -rf "$(dirname "$ICONSET")"
fi

# Update in place — do NOT rm -rf the .app. Recreating the bundle every launch
# changes the code identity and macOS drops Accessibility / Camera TCC grants.
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/AirControl"
chmod +x "$APP/Contents/MacOS/AirControl"
cp "$ICON_ICNS" "$APP/Contents/Resources/AppIcon.icns"

VERSION="$(grep -E '^version' "$ROOT/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"

cat >"$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ru</string>
  <key>CFBundleDisplayName</key>
  <string>AirControl</string>
  <key>CFBundleExecutable</key>
  <string>AirControl</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>app.aircontrol.desktop</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>AirControl</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSCameraUsageDescription</key>
  <string>AirControl использует камеру, чтобы распознавать жесты рук и управлять компьютером и инструментами.</string>
  <key>NSMicrophoneUsageDescription</key>
  <string>AirControl использует микрофон только когда вы включаете голосовые команды.</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

# Ad-hoc sign so TCC can attach grants to this bundle id across rebuilds better.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
fi

# Refresh Launch Services so Dock picks up the new icon.
LSREG="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREG" ]]; then
  "$LSREG" -f "$APP" >/dev/null 2>&1 || true
fi

echo "$APP"
