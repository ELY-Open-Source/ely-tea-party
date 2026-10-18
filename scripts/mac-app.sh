#!/bin/sh
# mac-app.sh: Ely Tea Party.app in target/release and its disk image, signed ad hoc, or by SIGN_IDENTITY and notarized; DEV=1 makes Ely Tea Party (dev).app alone, to sit beside a release.
set -eu
cd "$(dirname "$0")/.."
sign=${SIGN_IDENTITY:-}
name="Ely Tea Party"
id=net.toddzheng.ely-tea-party
if [ -n "${DEV:-}" ]; then
  [ -z "$sign" ] || { echo "mac-app: a dev build is signed ad hoc, not by SIGN_IDENTITY" >&2; exit 1; }
  name="Ely Tea Party (dev)"
  id=$id.dev
fi
if [ -n "$sign" ]; then
  [ -s "${NOTARY_KEY:-}" ] && [ -n "${NOTARY_KEY_ID:-}" ] && [ -n "${NOTARY_ISSUER:-}" ] || {
    echo "mac-app: SIGN_IDENTITY needs NOTARY_KEY (a .p8 file), NOTARY_KEY_ID and NOTARY_ISSUER" >&2
    exit 1
  }
  security find-identity -v -p codesigning | grep -q -F "\"$sign\"" || {
    echo "mac-app: no valid identity \"$sign\" in the keychains" >&2
    exit 1
  }
fi
cargo build --release --locked
version=$(cargo pkgid)
version=${version##*[#@]}
case $version in [0-9]*) ;; *) echo "mac-app: cargo pkgid names no version" >&2; exit 1 ;; esac
out=${CARGO_TARGET_DIR:-target}/release
app="$out/$name.app"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo run --release --locked --quiet --example icons -- \
  iconset assets/icons/ely-tea-party-macos.svg "$work/ely-tea-party.iconset"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$out/ely-tea-party" "$app/Contents/MacOS/"
iconutil -c icns "$work/ely-tea-party.iconset" -o "$app/Contents/Resources/ely-tea-party.icns"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>$name</string>
  <key>CFBundleExecutable</key><string>ely-tea-party</string>
  <key>CFBundleIconFile</key><string>ely-tea-party</string>
  <key>CFBundleIdentifier</key><string>$id</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$name</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
plutil -lint -s "$app/Contents/Info.plist"
if [ -n "$sign" ]; then
  codesign --force --options runtime --timestamp --sign "$sign" "$app"
else
  codesign --force --sign - "$app"
fi
if [ -n "${DEV:-}" ]; then
  echo "mac-app: $app"
  exit 0
fi
mkdir "$work/disk"
ditto "$app" "$work/disk/Ely Tea Party.app"
ln -s /Applications "$work/disk/Applications"
dmg=$out/ely-tea-party-$version.dmg
hdiutil create -quiet -volname "Ely Tea Party" -srcfolder "$work/disk" -ov -format UDZO "$dmg"
if [ -n "$sign" ]; then
  codesign --force --timestamp --sign "$sign" "$dmg"
  notary() { xcrun notarytool "$@" --key "$NOTARY_KEY" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER"; }
  notary submit "$dmg" --wait --output-format json > "$work/notary.json" || true
  status=$(plutil -extract status raw -o - "$work/notary.json" 2>/dev/null || echo none)
  echo "mac-app: notarization $status"
  if [ "$status" != Accepted ]; then
    cat "$work/notary.json" >&2
    id=$(plutil -extract id raw -o - "$work/notary.json" 2>/dev/null) && notary log "$id" >&2
    exit 1
  fi
  xcrun stapler staple "$dmg"
  spctl --assess --type open --context context:primary-signature --verbose "$dmg"
fi
echo "mac-app: $app"
echo "mac-app: $dmg"
