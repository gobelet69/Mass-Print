# Mass Print — Agent Instructions & Architectural Guidelines

## 1. Project Overview & Architecture

**Mass Print** is a native macOS batch printing utility with high print safety controls and document ordering:
- **Core Engine**: Rust 2021 workspace (`crates/mass-print-core` and `crates/mass-print-ffi`).
  - Implements document queue ordering, natural number sorting, format detection (PDF, images, text, Office conversion), spooler job tracking, and safe cancellation.
- **Frontend / UI**: Swift / AppKit (`NativeApp/`):
  - `MassPrintApp.swift`, `ContentView.swift`, `AppModel.swift`, `FileIngress.swift`, `RustBridge.swift`.
  - Native macOS AppKit window with Liquid Glass styling, drag-and-drop document queue, and Finder Service integration (`Print with Mass Print.workflow`).
- **Brand Identity**: Electric Blue (`#2266F2` Light / `#3880DC` Dark) with Solar Amber accents.

---

## 2. Invariants & Safety Rules

1. **Rust Safety & Error Handling**:
   - Zero avoidable panics. Never call `.unwrap()` or `.expect()` on user paths, external input, or print queue payloads.
   - All C-FFI entrypoints in `mass-print-ffi` must catch unwinds and return structured error codes.
2. **macOS iCloud Build Invariant**:
   - Compiling inside `~/Documents` without an external target directory can cause file locking.
   - Always run tests and builds with `CARGO_TARGET_DIR=/tmp/mass-print-target`:
     ```bash
     CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/mass-print-target cargo test --workspace
     ```
3. **No Blind Master Directory Writes**:
   - Never write to or delete files from `~/Documents/MASTER`.
4. **Preserve Finder Integration**:
   - The application bundles a Finder Quick Action / Service (`NSServices` in `Info.plist`). Preserve service entrypoints and queue file ingress handlers.

---

## 3. Verification Commands

Before submitting or committing changes:
```bash
# 1. Rust Formatting & Linting
cargo fmt --all -- --check
CARGO_TARGET_DIR=/tmp/mass-print-target cargo clippy --workspace --all-targets -- -D warnings

# 2. Rust Unit & Integration Tests (30 tests)
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/mass-print-target cargo test --workspace

# 3. Native App Bundle Build
./scripts/build-native-app.sh
```

---

## 4. Standards Reference

Refer to the shared DevKit standards in `../111iridescence-devkit/standards/`:
- `rust.md`: Idiomatic ownership, bounded buffers, zero unwraps.
- `swift-macos.md`: Swift UI conventions, AppKit glass, memory safety.
- `brand.md`: Public identity `111iridescence`, privacy rules, zero personal names.
