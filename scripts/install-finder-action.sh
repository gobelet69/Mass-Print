#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=${0:A:h}
SOURCE_WORKFLOW="$SCRIPT_DIR/../finder/Print with Mass Print.workflow"
SERVICES_DIR="$HOME/Library/Services"
TARGET_WORKFLOW="$SERVICES_DIR/Print with Mass Print.workflow"

if [[ ! -d "$SOURCE_WORKFLOW" ]]; then
  print -u2 "Finder action not found: $SOURCE_WORKFLOW"
  exit 1
fi

if [[ -e "$TARGET_WORKFLOW" ]]; then
  read "reply?Replace the existing 'Print with Mass Print' Quick Action? [y/N] "
  [[ "$reply" == [yY] ]] || exit 0
  rm -rf -- "$TARGET_WORKFLOW"
fi

mkdir -p -- "$SERVICES_DIR"
cp -R -- "$SOURCE_WORKFLOW" "$TARGET_WORKFLOW"
/System/Library/CoreServices/pbs -flush 2>/dev/null || true
print "Installed: $TARGET_WORKFLOW"
print "In Finder, select files and use Quick Actions > Print with Mass Print."

