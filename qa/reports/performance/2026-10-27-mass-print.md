# Mass Print performance audit — 2026-10-27

## Result

**PASS with WARN items.** The audited native macOS application builds, launches, accepts Finder Service input, remains responsive with 3,000 synthetic documents, sorts and reverse-sorts correctly, and preserves cancellation/order in a batch workflow backed by an in-process test sink. No real print job was submitted and no release was published.

The two confirmed user-facing bottlenecks were fixed:

1. SwiftUI treated every externally opened file as a separate scene event. A 3,000-file handoff remained near 90% CPU, exceeded a 30-second accessibility timeout, and reached a sampled 1.7 GiB physical footprint. The app now advertises one scene for external events and coalesces URL callbacks into one queue mutation. The identical workload settles below 1% CPU by about 3.7 seconds, peaks near 144 MiB RSS during the observed window, and is visibly interactive with all 3,000 items.
2. Natural filename sorting normalized and tokenized both names for every comparison. Caching each name's natural-sort key once per sort reduced the 3,000-document median from 8.583 ms to 2.741 ms ascending (68.1%) and from 8.592 ms to 2.669 ms descending (68.9%).

## Environment

- Hardware: Apple M2, 8 GiB RAM, arm64
- OS: macOS 27.0 (26A428)
- Xcode: 27.1 (27A9275), Swift 6.4
- Rust: 1.90.0
- App: Mass Print 0.2.0, universal arm64/x86_64, ad-hoc signed
- Repository state before audit: `main...origin/main`, clean
- Workload: 3,000 synthetic one-line UTF-8 text documents with accented/numeric filenames; 25 synthetic text documents for safe PDF preparation; three synthetic PDFs for fake-spooler cancellation

## Measurements

| Area | Baseline | After | Status |
|---|---:|---:|---|
| Cold launch to first window | 1,867 ms | 2,033 ms | WARN — single cold samples, within high launch variability |
| Warm launch to first window | 441 ms median (runs 2–5) | 395 ms median (runs 2–5) | PASS |
| Empty idle CPU | Usually 0%, spikes through 6.5% in 10 samples | 0.0% in 9/10 samples; one 0.1% | PASS |
| Empty idle RSS, tenth sample | 45,536 KiB | 30,848 KiB | PASS |
| 3,000-file external handoff | Accessibility timeout >30 s; ~90% CPU; sampled 1.7 GiB footprint | CPU <1% by ~3.7 s and 0% by ~4.9 s; observed RSS peak ~144 MiB | PASS |
| 3,000-file add through C/JSON bridge | 312.729 ms median | 207.823 ms median | INFO — no add-path code change; filesystem/cache sensitive |
| 3,000-item snapshot through C/JSON bridge | 14.040 ms median | 9.928 ms median | INFO — now off the main thread; raw time is cache sensitive |
| Alphabetical sort | 8.583 ms median | 2.741 ms median | PASS — 68.1% faster |
| Reverse alphabetical sort | 8.592 ms median | 2.669 ms median | PASS — 68.9% faster |
| Engine reorder | 0.024 ms median | 0.015 ms median | PASS — both negligible; snapshot now off-main |
| Prepare 25 text documents as PDFs | 2,930 ms | 2,303 ms | WARN — one run each; unchanged per-file `cupsfilter` process cost |
| Idle leak scan | 414 leaks / 19,872 bytes | 416 leaks / 19,936 bytes | WARN — effectively flat; restricted process prevented complete attribution |

Medians use five release-mode iterations. Launch measurements include `open` and polling until the first application window exists. CPU and RSS use `ps`; stacks use `sample`; heap evidence uses `leaks`. Raw evidence and exact commands are in [`evidence/2026-10-27-mass-print-baseline.txt`](evidence/2026-10-27-mass-print-baseline.txt) and [`evidence/2026-10-27-mass-print-after.txt`](evidence/2026-10-27-mass-print-after.txt).

## Implemented improvements

- Added SwiftUI external-event routing and a 50 ms thread-safe file ingress coalescer, turning thousands of URL callbacks into one queue update.
- Moved add, sort, reorder, clear, remove, print, and snapshot bridge work onto an ordered background dispatch queue. Cancellation uses a separate queue so it cannot wait behind a long batch.
- Added explicit `Sendable` conformances to bridge model values; the final Swift build is warning-free.
- Cached natural filename keys for alphabetical and reverse-alphabetical sorting while retaining stable insertion-order ties.
- Added ignored, repeatable release-mode probes for the 3,000-document C/JSON bridge and 25-document safe preparation workload.
- Added a fake-spooler batch regression test: the first synthetic PDF is accepted, cancellation is raised, and the remaining PDFs are canceled in source order without invoking `lp`.
- Made native bundle signing reliable on File Provider-backed storage by staging/signing in a local temporary directory, retaining the verified bundle in a local cache, and exposing the documented repository build path as a symlink. Three consecutive builds and delayed strict signature checks passed; packaging resolves the symlink before archiving.

## Functional and runtime verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace`: PASS — 27 regular tests; two manual performance probes ignored in the normal suite
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- `./scripts/build-native-app.sh` three times consecutively with delayed signature checks: PASS
- `codesign --verify --deep --strict build/native/Mass Print.app`: PASS
- Universal executable: PASS — arm64 and x86_64
- Native UI launch and exact bundle inspection: PASS
- 3,000-document visible count: PASS
- Alphabetical and reverse-alphabetical visible order: PASS
- Finder Service via `NSPerformService`: PASS; visible queue count increased by one
- Safe batch order/cancellation with fake destination: PASS; zero real submissions

## Remaining WARN items and limits

- Text/image preparation still launches `cupsfilter` once per document. The 25-document run took 2.3–2.9 seconds. Parallelizing this may improve throughput but needs a bounded concurrency design and broader format/resource tests; it was not justified by the current evidence alone.
- Accessibility-driven drag synthesis did not trigger SwiftUI's drop destination, so a real mouse drag was not automatically proven. The stable-ID reorder engine path and large-list reorder latency are covered; manual drag QA remains advisable.
- `leaks` reported that the ad-hoc signed process was not fully debuggable. Counts were flat and sampled roots were framework/XPC allocations, but this is not equivalent to a full Instruments Allocations/Leaks trace.
- No real printer or CUPS queue was used. Printer discovery was exercised, preparation used synthetic files, and submission used only the in-process fake destination as required.

## Gate

No release, tag, push, pull request, installation, or real print job was performed.
