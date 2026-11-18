#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
NATIVE_TEST_BUILD="${MASS_PRINT_NATIVE_TEST_BUILD:-/tmp/mass-print-native-tests-build}"
case "$NATIVE_TEST_BUILD" in /tmp/*) ;; *) echo 'Native test build must be under /tmp' >&2; exit 1 ;; esac
cd "$PROJECT_DIR"
CARGO_TARGET_DIR="$NATIVE_TEST_BUILD" cargo build -p mass-print-ffi
swiftc -parse-as-library -framework AppKit -framework SwiftUI \
  -L "$NATIVE_TEST_BUILD/debug" -lmass_print_ffi \
  -Xlinker -rpath -Xlinker "$NATIVE_TEST_BUILD/debug" \
  NativeApp/{Models,RustBridge,FileIngress,AppModel}.swift \
  "$SCRIPT_DIR/main.swift" -o "$NATIVE_TEST_BUILD/NativeQueueChecks"
"$NATIVE_TEST_BUILD/NativeQueueChecks"
