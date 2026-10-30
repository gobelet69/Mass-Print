# Mass Print release guard check — 2026-10-30

## Final status: WARN

All repository-controlled gates pass for the 0.2.1 candidate. Publication is
still **BLOCKED externally** because this machine/repository does not have a
Developer ID Application distribution certificate and Apple notarization
credentials configured. The release workflow now fails closed before the
GitHub release step rather than publishing an ad-hoc artifact.

No tag, push, GitHub release, website update, or real print job was performed.

## Release manifest

| Field | Value |
| --- | --- |
| Repository | `gobelet69/Mass-Print` |
| Candidate version | `0.2.1` |
| Audited source commit | `40f1d89faa6bb0766155f6e15a17b2f188aab06c` |
| Platform | macOS 13+, universal `x86_64 arm64` |
| Bundle identifier | `com.massprint.app` |
| Local signing | Ad-hoc; strict code-sign verification passes |
| Publication signing | BLOCKED — Developer ID/notary credentials absent |
| Website synchronization | Not requested; not performed |

Local candidate artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| `Mass.Print_0.2.1_universal.app.zip` | `f75c261c0511bdd68e3209755fff1a233fe99e53799172e1afee531698d6cbeb` |
| `Mass.Print_0.2.1_universal.dmg` | `10709b12ad53ee051eff1bdfc68be5c14ad68f106b7d6de16ae239939a8a667d` |
| App executable in build, ZIP, and DMG | `b5eb0d488b549303632dae49bad9ef9bb83a690733a5f26c938ece321670ce03` |

The ZIP and mounted DMG contain the same executable as the verified build. The
ZIP application launched with one visible window and zero active print jobs.

## Closed findings

| Priority | Finding | Resolution | Evidence |
| --- | --- | --- | --- |
| P1 | 3,000-file Finder handoff exhausted CPU/memory and blocked the UI | Coalesced external ingress and background engine queue | Real 3,000-file runtime test remains responsive |
| P1 | Build bundle could fail signing on File Provider metadata | Local staging, xattr cleanup, strict signing verification | Repeated universal builds pass |
| P1 | Build path later became a broken symlink into purgeable temporary storage | Final bundle cache moved to persistent Darwin user cache | Fresh build/package and packaged launch pass |
| P1 | Release workflow could publish an ad-hoc unnotarized artifact | Developer ID/notarization/stapling/Gatekeeper steps added; missing secrets fail closed | Local distribution gate rejects the ad-hoc candidate before publication |
| P2 | Large natural sorts repeated normalization/tokenization | Cached per-document natural sort keys | A–Z/Z–A remain about 69–70% below baseline |
| P2 | Main-thread bridge work and delayed cancellation | Ordered background engine queue and independent cancellation queue | Large-list and cancellation tests pass |
| P2 | Drag, sort, progress, count, and compact-layout defects | Native visual/state corrections | Before/after runtime capture matrix passes |
| P2 | Version 0.2.0 collided with the existing `v0.2.0` tag | Candidate bumped to 0.2.1; Cargo/plist/build/changelog/tag check aligned | Local and simulated `v0.2.1` checks pass |
| P3 | One sequential `cupsfilter` process dominated preparation time | Bounded four-document preparation windows; ordered submission; LibreOffice isolation | 2,115.209 ms → 1,007.387 ms median; −52.4% |
| P3 | Leak attribution was incomplete for the restricted ad-hoc process | Temporary development-signed `leaks --atExit` run with normal quit | 0 leaks / 0 leaked bytes |

## Verification matrix

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --workspace` | PASS — 30 passed, 2 ignored manual probes |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Version consistency, including simulated `v0.2.1` | PASS |
| Zsh script syntax | PASS |
| GitHub Actions YAML parsing | PASS |
| Native universal build | PASS |
| Strict deep signature verification | PASS |
| Application plist version/build `0.2.1` | PASS |
| Built application launch | PASS — one visible window |
| ZIP and DMG executable provenance | PASS — exact SHA-256 match |
| Packaged application launch | PASS — one visible window |
| Finder Service declaration/live behavior | PASS — prior visual audit evidence |
| Performance and visual reports | PASS |
| Idle leak trace | PASS — 0 leaks |
| Real print jobs submitted | PASS — 0 |
| Developer ID signature | BLOCKED — credential not available |
| Apple notarization and stapling | BLOCKED — credential not available |
| Gatekeeper acceptance of local ad-hoc artifact | BLOCKED as expected |

## Remaining external action

Before a future publication, configure these GitHub Actions secrets with the
project's Apple distribution credentials:

- `MACOS_CERTIFICATE_P12`
- `MACOS_CERTIFICATE_PASSWORD`
- `MACOS_SIGNING_IDENTITY`
- `APPLE_NOTARY_ID`
- `APPLE_NOTARY_PASSWORD`
- `APPLE_TEAM_ID`

Then run release guard again against the exact signed/notarized 0.2.1 artifact.
Only an accepted notarization response, successful stapling, Gatekeeper PASS,
matching hashes, and a clean `v0.2.1` tag can promote this gate from WARN to
PASS. The current workflow cannot silently bypass these checks.

