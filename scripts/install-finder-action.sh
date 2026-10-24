#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=${0:A:h}
SOURCE_WORKFLOW="$SCRIPT_DIR/../finder/Print with Mass Print.workflow"
SERVICES_DIR="$HOME/Library/Services"
TARGET_WORKFLOW="$SERVICES_DIR/Print with Mass Print.workflow"
STAGING_WORKFLOW="$SERVICES_DIR/.Print with Mass Print.workflow.installing"

if [[ ! -d "$SOURCE_WORKFLOW" ]]; then
  print -u2 "Finder action not found: $SOURCE_WORKFLOW"
  exit 1
fi

/usr/bin/plutil -lint "$SOURCE_WORKFLOW/Contents/Info.plist"
/usr/bin/plutil -lint "$SOURCE_WORKFLOW/Contents/document.wflow"

if [[ -e "$TARGET_WORKFLOW" ]]; then
  read "reply?Replace the existing 'Print with Mass Print' Quick Action? [y/N] "
  [[ "$reply" == [yY] ]] || exit 0
fi

mkdir -p -- "$SERVICES_DIR"
rm -rf -- "$STAGING_WORKFLOW"
cp -R -- "$SOURCE_WORKFLOW" "$STAGING_WORKFLOW"
/usr/bin/plutil -lint "$STAGING_WORKFLOW/Contents/Info.plist"
/usr/bin/plutil -lint "$STAGING_WORKFLOW/Contents/document.wflow"
rm -rf -- "$TARGET_WORKFLOW"
mv -- "$STAGING_WORKFLOW" "$TARGET_WORKFLOW"
/System/Library/CoreServices/pbs -flush 2>/dev/null || true
/System/Library/CoreServices/pbs -update 2>/dev/null || true
/usr/bin/killall Finder 2>/dev/null || true
print "Installed: $TARGET_WORKFLOW"
print "In Finder, select files and use Quick Actions > Print with Mass Print."

