# Mass Print

Mass Print is a focused macOS utility for arranging local documents and submitting them to a printer in an exact, user-controlled order. It never prints automatically when files arrive from Finder.

## Architecture

- `crates/mass-print-core`: dependency-light Rust domain layer for document inspection, deduplication, queue ordering, conversion, sequential spooler submission, and cancellation.
- `src-tauri`: thin Tauri 2 application shell. It exposes typed commands, receives files from Finder/open events, and keeps a single application instance.
- `src`: local TypeScript/CSS interface rendered by the system WebKit view. No document data leaves the Mac.
- `finder`: Finder Quick Action that opens selected files with the running or newly launched application.

The printing backend first prepares a PDF where needed, then calls the documented macOS CUPS commands sequentially. A successful `lp` response means the spooler accepted the job; it does not claim that physical printing completed.

## Requirements

- macOS 12.3 or later
- Xcode Command Line Tools
- Rust 1.90 through `rustup`
- Node.js 20 or later
- At least one configured macOS printer for real printing
- Optional: LibreOffice for DOC, DOCX, RTF, ODT, HTML, XLS, XLSX, PPT, PPTX, ODS, and ODP conversion

Apple Silicon is the primary target. Tauri and the Rust core also support Intel macOS; build on each architecture or create a universal binary in release automation.

## Development

```sh
npm ci
npm run tauri dev
```

Useful checks:

```sh
cargo test -p mass-print-core
cargo clippy -p mass-print-core --all-targets -- -D warnings
npm run check
npm run build
```

Generate icons after changing `assets/app-icon.svg`:

```sh
npm run icon
```

## Build and package

```sh
npm ci
npm run icon
npm run tauri build
```

The application and DMG are written under `src-tauri/target/release/bundle/`. The bundle identifier is `com.massprint.app`, and the minimum system version is macOS 12.3.

The project does not include a signing identity. Unsigned local builds can be run by the developer, but public distribution requires an Apple Developer ID certificate, hardened-runtime signing, notarization, and stapling. Do not bypass Gatekeeper.

## Using Mass Print

1. Add files with **Add Files** (`⌘O`) or drag them from Finder.
2. Drag rows to reorder them, or use the accessible up/down buttons.
3. Choose a printer, copies, grayscale, and two-sided printing.
4. Review the visible order and select **Print All** (`⌘P`).

Jobs are submitted one at a time in queue order. **Cancel** prevents further submissions and asks CUPS to cancel only the job identifiers submitted by the current batch.

### Ordering documents

Drag a row by its visible handle to create a custom print order. The blue
insertion line shows the exact destination, and the queue switches to **Custom
Order** so it never snaps back to a previous sort.

The compact toolbar above the queue can sort by natural filename, modification
date, file size, page count, file type, or original insertion order. Natural
filename sorting places `Chapter 2.pdf` before `Chapter 10.pdf`. Files with
unknown dates or page counts stay at the end. When a predefined sort is active,
newly added files are placed by that sort; in Custom Order they are appended.

## Finder Quick Action

After placing `Mass Print.app` in `/Applications`, launch it and select
**Install Finder Action** in the toolbar. The Quick Action is bundled inside
the application; downloading the source code or using Terminal is not required.

Mass Print validates the workflow with `plutil`, installs it atomically at
`~/Library/Services/Print with Mass Print.workflow`, refreshes the macOS
Services database, and restarts Finder. Re-running the installer repairs or
updates Mass Print's own workflow without creating duplicates.

In Finder, select files, right-click, and choose **Quick Actions → Print with Mass Print**. Depending on macOS settings, it may appear under **Services**. The action passes each selected path to Mass Print and opens or focuses its existing window. It never starts printing.

The action is an Automator service, not a Finder Sync app extension. Depending
on the macOS version, its checkbox is under **System Settings → Keyboard →
Keyboard Shortcuts → Services → Files and Folders**. It may therefore not be
listed in the separate **Finder Extensions** panel.

Use the **Remove** button beside **Finder Action ✓** to uninstall it. Developers
working from a source checkout can alternatively install or remove it with:

```sh
./scripts/install-finder-action.sh
./scripts/uninstall-finder-action.sh
```

Both scripts ask before replacing or removing an existing workflow.

## Supported formats

| Format | Preparation | Notes |
| --- | --- | --- |
| PDF | Direct | Page count is read without rendering the full document. |
| PNG, JPEG | macOS `cupsfilter` to PDF | One page per image. |
| TIFF | macOS `cupsfilter` to PDF | Multi-page count is left unknown before conversion. |
| TXT | macOS `cupsfilter` to PDF | Displayed page count is an estimate based on line count. |
| DOC/DOCX, RTF, ODT, HTML | LibreOffice to PDF | Requires an existing LibreOffice installation. |
| XLS/XLSX/ODS, PPT/PPTX/ODP | LibreOffice to PDF | Layout depends on installed fonts and LibreOffice rendering. |

Unsupported types are rejected clearly. Originals are never modified. Temporary conversion output is held in a private temporary directory and deleted after submission.

## Printing limitations

- Printer-specific options differ. Grayscale uses `ColorModel=Gray`, duplex uses `sides=two-sided-long-edge`, and fit uses `fit-to-page`; drivers may reject options they do not support.
- The first release exposes common settings only. Paper size and landscape are implemented in the Rust backend and can be enabled in the UI after capability discovery is added.
- Password-protected or malformed PDFs may show an unknown page count and can still fail at spool time.
- iCloud placeholders must be downloaded by macOS before they are readable.
- Physical output, driver-specific finishing, and Finder UI placement must be verified on a real Mac.

## Safety and privacy

Mass Print works locally, has no account system, analytics, upload endpoint, or cloud dependency. It requests no administrator privileges and never changes source documents. The app is deliberately not sandboxed in this initial direct-distribution build because it must retain access to Finder-selected files and local print tooling; App Store distribution would require security-scoped bookmarks and a separately reviewed printing design.

## Test policy

Core tests use temporary files and a canceled mock scenario; they never submit physical print jobs. Before release, test on macOS with a safe virtual/PDF printer or a designated test printer, then verify page order and content manually. Never point automated tests at a production printer.

