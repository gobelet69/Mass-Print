# Mass Print visual audit — 2026-10-29

## Outcome

**PASS.** The signed universal macOS application was built and launched from
`build/native/Mass Print.app`. The audit exercised empty and populated queues,
multi-document import, a real cross-application file drag, manual reordering,
natural A–Z and Z–A sorting, Finder Services delivery, safe print preparation,
error reporting, minimum-window layout, and light/dark appearances.

No real print job was submitted. The CUPS queue contained zero active jobs
before and after every preparation/error run. No release was published.

## Fixed defects

1. **Print preparation had no visible progress affordance.** The busy state
   exposed only Cancel while Print All remained disabled. It now replaces the
   print action with a native indeterminate progress indicator and Cancel, with
   the accessibility label `Preparing print jobs`.
2. **Sort state was technically correct but visually ambiguous.** Filename
   sorting reported `Ascending`/`Descending`, and the icon-only direction
   control exposed generic `Up`/`Down` labels. It now reports `Name · A–Z` and
   `Name · Z–A`, uses a bidirectional sort icon, and exposes the contextual
   labels `Sort ascending` or `Sort descending`.
3. **Original-order copy implied a sort direction.** `Original Order ·
   Ascending` is now the native-language `Original order`; reversed original
   order remains explicit.
4. **Drop-state cleanup was incomplete at the list end.** Successful row and
   end-of-list drops now clear insertion state explicitly, and the end target
   gains the same two-point insertion indicator as row targets. Canceled drags
   leave no stuck highlight.
5. **Status and counter copy was inconsistent.** `document(s)` was replaced by
   correct singular/plural forms; one-item states now read `1 Document`, `1
   document`, and `1 page`; large document/page and result counts use locale
   grouping.
6. **The first progress treatment overflowed the minimum-width footer.** This
   was caught in the post-fix screenshot pass. Removing redundant visible text
   retained the native spinner and accessible label while restoring the
   original compact geometry.

## Runtime matrix

| Scenario | Evidence | Result |
| --- | --- | --- |
| Empty window | `screenshots/after/empty-light.png`, `screenshots/after/empty-dark.png` | PASS — clear hierarchy, native controls, no clipping |
| Multiple-document import | `screenshots/after/import-multiple-light.png`, `screenshots/after/import-multiple-dark.png` | PASS — six synthetic files, long/unicode names, correct row metadata |
| Active drag and drop | `screenshots/after/drag-drop-target-light.png` | PASS — real AppKit file-URL drag switched the live target to `Drop to add to the queue` |
| Completed drop | `screenshots/after/drag-drop-complete-light.png` | PASS — one synthetic file added; singular copy correct |
| Canceled drag | `screenshots/before/reorder-canceled-light.png` | PASS — no stuck highlight or changed order |
| Manual reordering | `screenshots/before/reorder-held-drag-dark.png`, `screenshots/after/reorder-light.png` | PASS — held native drag moved the row and changed state to Custom Order |
| Sort A–Z | `screenshots/after/sort-a-z-light.png` | PASS — natural order and explicit A–Z state |
| Sort Z–A | `screenshots/after/sort-z-a-light.png` | PASS — reverse natural order and explicit Z–A state |
| Print preparation | `screenshots/before/print-preparation-dark.png`, `screenshots/after/print-preparation-dark.png` | PASS — spinner, Cancel, disabled mutation controls; no spooler job |
| Error state | `screenshots/before/error-result-dark.png`, `screenshots/after/error-result-light.png` | PASS — concise batch summary and per-row warning affordances |
| Compact layout | `screenshots/before/compact-list-light.png`, post-fix preparation capture | PASS — minimum rendered window 900 × 632 points; final footer fits |
| Finder integration | live Finder Services AX inspection and invocation | PASS — `Print with Mass Print` visible under Services and delivered the selected synthetic file |

## Finder and drag behavior

Finder exposed `Print with Mass Print` under the selected file's **Services**
submenu. Invoking it activated Mass Print and delivered the file. When the file
was already queued, the app correctly surfaced the duplicate rejection instead
of inserting a second row. Its appearance under Finder's top-level **Quick
Actions** submenu remains controlled by the user's macOS Services configuration,
as documented in the README.

The audit also used the saved local fixture `evidence/drag-source.swift` to
create a genuine `public.file-url` drag without touching user documents. The
live window entered and exited the highlighted drop state, completed the import,
and remained usable. A separate canceled row drag left no stuck interaction
state or insertion line.

## Safety and privacy

- Every imported document was generated under a temporary directory.
- Preparation/error tests renamed the synthetic source directory after queueing,
  forcing preparation to fail before `/usr/bin/lp` could be called.
- Active print jobs: **0 before; 0 after**.
- Screenshots and raw UI evidence remain local to this repository and are not
  embedded in public release material.
- The original system appearance was restored to dark mode.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace`: PASS — 27 passed, 2 ignored manual probes
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- `./scripts/build-native-app.sh`: PASS
- `codesign --verify --deep --strict`: PASS
- `plutil -lint`: PASS
- Universal binary: PASS — `x86_64 arm64`
- Final real application launch and accessibility inspection: PASS
- Finder service declaration and live delivery: PASS
- Real print jobs submitted: **0**
- Release/publish performed: **no**

## Curated screenshots

Baseline and updated screenshots are stored in `screenshots/before/` and
`screenshots/after/`. The most representative updated states are:

- `screenshots/after/drag-drop-target-light.png`
- `screenshots/after/drag-drop-complete-light.png`
- `screenshots/after/sort-a-z-light.png`
- `screenshots/after/sort-z-a-light.png`
- `screenshots/after/reorder-light.png`
- `screenshots/after/print-preparation-dark.png`
- `screenshots/after/error-result-light.png`
- `screenshots/after/import-multiple-dark.png`

