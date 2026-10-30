#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
APP_LINK="$ROOT_DIR/build/native/Mass Print.app"
APP_PATH=${APP_LINK:A}
test -f "$APP_PATH/Contents/Info.plist"
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_PATH/Contents/Info.plist")
PROFILE=${MASS_PRINT_NOTARY_PROFILE:-}
KEYCHAIN_PATH=${MASS_PRINT_NOTARY_KEYCHAIN:-}
SIGNING_IDENTITY=${MASS_PRINT_CODESIGN_IDENTITY:-}

if [[ -z "$PROFILE" || -z "$SIGNING_IDENTITY" ]]; then
  print -u2 "MASS_PRINT_NOTARY_PROFILE and MASS_PRINT_CODESIGN_IDENTITY are required"
  exit 1
fi

NOTARY_AUTH=(--keychain-profile "$PROFILE")
if [[ -n "$KEYCHAIN_PATH" ]]; then
  NOTARY_AUTH+=(--keychain "$KEYCHAIN_PATH")
fi

"$ROOT_DIR/scripts/package-native-release.sh"
ZIP_PATH="$ROOT_DIR/dist-native/Mass.Print_${VERSION}_universal.app.zip"
xcrun notarytool submit "$ZIP_PATH" "${NOTARY_AUTH[@]}" --wait --timeout 30m
xcrun stapler staple "$APP_PATH"
xcrun stapler validate "$APP_PATH"

"$ROOT_DIR/scripts/package-native-release.sh"
DMG_PATH="$ROOT_DIR/dist-native/Mass.Print_${VERSION}_universal.dmg"
codesign --force --timestamp --sign "$SIGNING_IDENTITY" "$DMG_PATH"
xcrun notarytool submit "$DMG_PATH" "${NOTARY_AUTH[@]}" --wait --timeout 30m
xcrun stapler staple "$DMG_PATH"
xcrun stapler validate "$DMG_PATH"

"$ROOT_DIR/scripts/verify-native-distribution.sh"
shasum -a 256 "$ROOT_DIR/dist-native"/*
