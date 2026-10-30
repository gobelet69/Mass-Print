# Mass Print release guard check — 2026-10-29

## Final status: WARN

The application source, tests, universal build, signing integrity, runtime
launch, Finder Service delivery, performance fixes, and visual corrections
pass their checks. The current tree is suitable for continued development and
review, but **not for publication in its present state**: the fixes are
uncommitted while the bundle still reports version 0.2.0 and `HEAD` is already
tagged `v0.2.0`; distribution artifacts are ad-hoc signed and not notarized.

No release, tag, push, pull request, installation over the user's application,
or real print submission was performed.

## Corrected issues

| Priority | Problem and reproduction | Correction | Verification |
| --- | --- | --- | --- |
| P1 | Opening 3,000 files caused >30 s UI timeout, ~90% CPU and sampled 1.7 GiB | Single external-event scene and coalesced ingress | Real app accepted 3,000 files, stayed responsive, peak 152,128 KiB |
| P1 | Queue bridge work could block the main thread; cancellation could wait behind a batch | Ordered background engine queue and separate cancellation queue | Large-queue probe, real UI, and cancellation regression pass |
| P1 | Print preparation exposed no visible progress | Native indeterminate progress indicator plus Cancel and AX label | Updated dark-mode runtime capture and accessibility inspection pass |
| P1 | File Provider metadata made native bundle signing intermittently fail | Build/sign in a local temporary stage, clear xattrs before signing, cache verified app | Repeated builds, strict signature checks and packaged-app checks pass |
| P2 | Natural sorting retokenized names for every comparison | Cached sort keys | 2.674/2.575 ms medians, 68.8%/70.0% below baseline |
| P2 | Sort state and original-order copy were ambiguous | Explicit A–Z/Z–A labels and native-language original-order state | Updated sort captures and AX labels pass |
| P2 | Successful/canceled drops could leave incomplete insertion state | Explicit row/end cleanup and matching end insertion indicator | Real file-URL drop, completed drop and canceled row drag pass |
| P2 | Counts and status copy had pluralization/grouping inconsistencies | Native singular/plural copy and locale grouping | One-item and 3,000-item runtime states pass |
| P2 | Safe batch cancellation lacked an isolated regression path | Injectable fake spooler test | Ordered accepted/canceled results; zero real jobs |
| P2 | Initial progress treatment overflowed the minimum footer | Removed redundant visible label, retained accessible spinner | 900 x 632 layout capture passes |

No reproducible P0 defect was found. No unresolved reproducible P1 or P2 defect
remains in the audited scope.

## Modified source files

- `.gitignore`
- `NativeApp/AppModel.swift`
- `NativeApp/ContentView.swift`
- `NativeApp/FileIngress.swift`
- `NativeApp/Models.swift`
- `crates/mass-print-core/src/printing.rs`
- `crates/mass-print-core/src/queue.rs`
- `crates/mass-print-ffi/src/lib.rs`
- `scripts/build-native-app.sh`
- `scripts/package-native-release.sh`
- `qa/` audit reports, raw evidence, fixture source, and screenshots

Source diff at the gate: 432 insertions and 87 deletions across ten tracked
source/build files. `git diff --check`: PASS.

## Test and runtime evidence

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --workspace` | PASS — 27 passed, 2 ignored manual probes |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Tracked-file credential/secret scan | PASS |
| Dependency license inventory | PASS — 132 packages, no unknown license expression |
| Repository license | PASS — MIT |
| Native build | PASS |
| Strict deep code-sign verification | PASS |
| Bundle executable architecture | PASS — arm64 and x86_64 |
| Real application launch and empty-window AX inspection | PASS |
| 3,000-document real UI handoff | PASS |
| Finder Service declaration and live delivery | PASS |
| Safe preparation/cancellation | PASS — zero real jobs |
| Light and dark visual matrix | PASS |
| Installed-copy smoke test from packaged app | PASS |
| Gatekeeper assessment | WARN — rejected because artifacts are ad-hoc signed/not notarized |

## Performance comparison

The final verification confirms the large-list and sorting gains. Key values:

- 3,000-file handoff: from >30 seconds/unresponsive and sampled 1.7 GiB to a
  responsive queue with 152,128 KiB observed peak RSS.
- A–Z sort: 8.583 ms to 2.674 ms median (68.8% reduction).
- Z–A sort: 8.592 ms to 2.575 ms median (70.0% reduction).
- Safe 25-document preparation: 2,930.465 ms to 2,431.140 ms (17.0% lower),
  with zero submitted jobs.

Full data: [`../performance/2026-10-29-mass-print-verification.md`](../performance/2026-10-29-mass-print-verification.md).

## Visual evidence

Before and after captures are indexed in
[`../visual/2026-10-29-mass-print.md`](../visual/2026-10-29-mass-print.md).
The updated set includes empty light/dark, multi-import light/dark, active and
completed file drops, reorder, A–Z, Z–A, dark print preparation, and error
states. The original system dark appearance was restored after the audit.

Representative comparisons:

- Before: `../visual/screenshots/before/print-preparation-dark.png`
- After: `../visual/screenshots/after/print-preparation-dark.png`
- Before: `../visual/screenshots/before/sort-a-z-dark.png`
- After: `../visual/screenshots/after/sort-a-z-light.png`
- Before: `../visual/screenshots/before/reorder-held-drag-dark.png`
- After: `../visual/screenshots/after/reorder-light.png`

## Local artifact manifest

The artifacts were produced only for gate verification and remain local:

| Artifact | SHA-256 |
| --- | --- |
| `dist-native/Mass.Print_0.2.0_universal.app.zip` | `77c5d4485ec1cbd32b5e4da04bb65f05c2b083beff3366c4a600757348ee6c62` |
| `dist-native/Mass.Print_0.2.0_universal.dmg` | `d313b6a26aac75e0d846acd9268d95bb8cd9c0d33fc29da220397224ccaa29be` |

Both package forms contain the expected bundle, have the same executable as
the verified build, are universal, and pass strict deep code-sign verification.

## Remaining issues and publication blockers

1. **WARN — version/provenance:** the fix tree is dirty, `HEAD` is exactly
   `v0.2.0`, and the bundle is also 0.2.0. A future publication needs an
   intentional version bump, release notes/changelog, clean commit, and tag.
2. **WARN — distribution trust:** the artifacts are ad-hoc signed and not
   notarized, so Gatekeeper rejects them outside the local smoke-test context.
   Developer ID signing/notarization is required if that is the chosen channel.
3. **P3 / WARN — preparation throughput:** one `cupsfilter` process per source
   document remains; bounded concurrency needs broader format/resource tests.
4. **P3 / WARN — leak attribution:** counts were flat, but a full Instruments
   allocation/leak trace was not available for this ad-hoc process.
5. **WARN — Finder placement:** the Service is declared and works live; whether
   macOS also shows it directly in Finder's Quick Actions section depends on
   the user's Services configuration.

These warnings do not invalidate the verified corrections, but items 1 and 2
block a publish decision. Final gate: **WARN; publication remains blocked.**

