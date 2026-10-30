# Mass Print

Mass Print is a native macOS batch-printing application. Its interface and
Finder integration use SwiftUI and AppKit; its tested queue, metadata, sorting,
conversion, and printing engine remains written in Rust. The distributed app
does not contain a WebView or the Tauri runtime.

## Features

- Native macOS window, controls, menus, file picker, drag and drop, and dark mode.
- Sequential batch printing through the macOS CUPS spooler.
- Visible drag handles and manual queue ordering with stable document IDs.
- Natural filename, modification date, size, page count, file type, and original-order sorting.
- PDF, PNG, JPEG, TIFF, text, RTF, HTML, Microsoft Office, and OpenDocument input.
- Native Finder service named **Print with Mass Print**.
- Multiple copies, grayscale, duplex, landscape, and fit-to-page options.

Office and OpenDocument files require LibreOffice for PDF conversion. Original
documents are never modified.

## Install

Download the universal DMG from GitHub Releases and drag **Mass Print.app** to
`/Applications`. Open it once so Launch Services registers the native Finder
service. No separate Quick Action installer is required.

Select one or more documents in Finder and use **Services → Print with Mass
Print**. Depending on macOS configuration, the same service can also appear in
**Quick Actions**. It adds the selected files to the visible queue and never
starts printing automatically.

Version 0.2 removes the legacy Automator workflow installed by older releases
after verifying that it belongs to Mass Print.

## Architecture

- `NativeApp/`: SwiftUI/AppKit application and native Services provider.
- `crates/mass-print-core/`: GUI-independent Rust engine.
- `crates/mass-print-ffi/`: narrow C ABI used by the Swift application.
- `scripts/build-native-app.sh`: universal Intel/Apple Silicon app build.

Swift sends typed JSON requests across the private in-process ABI. Queue order
has a single Rust source of truth, so the visible and printed orders cannot
diverge.

## Build

Requires macOS 13+, Xcode command-line tools, and Rust 1.90:

```sh
./scripts/build-native-app.sh
open "build/native/Mass Print.app"
```

Create a DMG and zipped application with:

```sh
./scripts/package-native-release.sh
```

## Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CI also builds both CPU architectures, verifies the universal Mach-O, registers
the native Service, invokes it through `NSPerformService`, and confirms that it
launches Mass Print.

## Security

Local builds are ad-hoc signed. Public release artifacts must be signed with a
Developer ID Application certificate, submitted to Apple's notarization
service, stapled, and accepted by Gatekeeper before the release workflow can
publish them. The workflow fails closed when signing or notarization credentials
are unavailable; it never falls back to an unnotarized public artifact.

## License

MIT
