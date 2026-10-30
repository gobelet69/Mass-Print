# Changelog

All notable changes to Mass Print are documented here.

## [0.2.1] - 2026-10-30

### Fixed

- Keep the native macOS interface responsive when Finder sends thousands of
  documents in one operation.
- Move queue mutations and snapshots away from the main thread while keeping
  cancellation immediately available.
- Cache natural filename keys for faster A–Z and Z–A sorting of large queues.
- Prepare native text and image documents in bounded parallel windows while
  preserving print order and stopping before the next window on cancellation.
- Clear drag insertion state reliably after completed and canceled reordering.
- Show native print-preparation progress and unambiguous sorting state.
- Build and sign the native application reliably on File Provider-backed
  workspaces.

### Changed

- Use locale-aware document, page, and batch-result counts.
- Clarify Finder Services integration and the distinction between local ad-hoc
  builds and notarized distributable builds.

