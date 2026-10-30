# Mass Print final performance fixes — 2026-10-30

## Outcome

**PASS with one external release WARN.** The remaining reproducible P3
preparation bottleneck and the incomplete leak attribution are resolved. A
newly discovered broken build-artifact link was also fixed. No real print job
was submitted.

## Bounded document preparation

Native text and image preparation now runs in windows of at most four workers.
Each window is fully collected before its results are submitted to CUPS in
source order. Cancellation prevents the next window from starting and marks
all remaining documents canceled. LibreOffice formats remain isolated in
single-document windows to avoid profile and converter contention.

| Scenario | Baseline | After | Change |
| --- | ---: | ---: | ---: |
| 25 synthetic text documents, median of five | 2,115.209 ms | 1,007.387 ms | −52.4% |
| Versus original 2026-10-27 baseline | 2,930.465 ms | 1,007.387 ms | −65.6% |

All runs used the same release-mode fixture and the in-process fake destination.
The after probe exercises the production batch preparation path. Submitted
real jobs: **0**.

Regression tests verify that:

- deliberately out-of-order preparation completion still submits in source
  order;
- cancellation after the first submission starts no work beyond the active
  four-document window;
- rich-document conversion is never prepared concurrently;
- accepted, failed, and canceled results retain document order.

## Leak verification

The earlier ad-hoc bundle could not be fully inspected by `leaks`. For this
verification, the exact 0.2.1 application was copied to a temporary directory,
development-signed with `get-task-allow`, launched under `leaks --atExit` with
malloc stack logging, and quit normally through the application menu.

- Physical footprint: 44.8 MiB
- Peak physical footprint: 46.7 MiB
- Allocations: 84,521 nodes / 12,781 KiB
- Leaks: **0**
- Leaked bytes: **0**

The temporary debug-signed copy and entitlement file were moved to Trash after
the run. Production signing behavior was not changed by this diagnostic.

## Build artifact durability

The prior signing workaround exposed `build/native/Mass Print.app` as a symlink
into the purgeable temporary directory. macOS removed that target, leaving a
broken build path. The cache now lives under the persistent Darwin user cache;
the temporary staging directory remains short-lived. A fresh build, strict
signature verification, packaging, ZIP/DMG executable identity comparison, and
packaged-app launch all pass.

## Version and release safety

- Cargo workspace, application plist, build number, and changelog now agree on
  version **0.2.1**.
- CI and the release workflow run an explicit version/tag consistency check.
- Local development builds remain ad-hoc signed.
- The publication workflow now requires Developer ID signing, hardened runtime,
  notarization, stapling, and Gatekeeper acceptance. Missing credentials fail
  closed before the GitHub release step.

Actual Developer ID/notarization execution remains **BLOCKED externally** until
the repository receives the required certificate and Apple notary secrets. No
fallback to an unnotarized public artifact exists.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace`: PASS — 30 passed, 2 ignored manual probes
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- Version check locally and as tag `v0.2.1`: PASS
- Workflow YAML parsing and all Zsh syntax checks: PASS
- Universal native build and strict code-sign verification: PASS
- ZIP/DMG executable matches verified build: PASS
- Packaged application launch: PASS — one visible window
- Real print jobs: **0**
- Raw evidence: [`evidence/2026-10-30-mass-print-final-fixes.txt`](evidence/2026-10-30-mass-print-final-fixes.txt)

