#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
APP_LINK="$ROOT_DIR/build/native/Mass Print.app"
APP_PATH=${APP_LINK:A}
test -f "$APP_PATH/Contents/Info.plist"
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_PATH/Contents/Info.plist")
DMG_PATH="$ROOT_DIR/dist-native/Mass.Print_${VERSION}_universal.dmg"

test -d "$APP_PATH"
test -f "$DMG_PATH"
codesign --verify --deep --strict "$APP_PATH"

SIGNING_DETAILS=$(codesign -dv --verbose=4 "$APP_PATH" 2>&1)
if ! print -r -- "$SIGNING_DETAILS" | grep -q '^Authority=Developer ID Application:'; then
  print -u2 "Mass Print is not signed with a Developer ID Application certificate"
  exit 1
fi

codesign --verify "$DMG_PATH"
xcrun stapler validate "$APP_PATH"
xcrun stapler validate "$DMG_PATH"
spctl --assess --type execute --verbose=4 "$APP_PATH"
hdiutil verify "$DMG_PATH"

print "Mass Print $VERSION is Developer ID signed, notarized, stapled, and Gatekeeper accepted"
