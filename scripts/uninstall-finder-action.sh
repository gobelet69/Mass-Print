#!/bin/zsh
set -euo pipefail

TARGET_WORKFLOW="$HOME/Library/Services/Print with Mass Print.workflow"
if [[ ! -e "$TARGET_WORKFLOW" ]]; then
  print "The Mass Print Finder action is not installed."
  exit 0
fi

read "reply?Remove the 'Print with Mass Print' Quick Action? [y/N] "
[[ "$reply" == [yY] ]] || exit 0
rm -rf -- "$TARGET_WORKFLOW"
/System/Library/CoreServices/pbs -flush 2>/dev/null || true
/System/Library/CoreServices/pbs -update 2>/dev/null || true
/usr/bin/killall Finder 2>/dev/null || true
print "Removed: $TARGET_WORKFLOW"

