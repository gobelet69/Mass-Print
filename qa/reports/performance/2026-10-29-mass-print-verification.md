# Mass Print performance verification — 2026-10-29

## Outcome

**PASS with WARN limits.** All reproducible P1/P2 performance defects from the
2026-10-27 audit remain fixed in the built and launched application. The final
run used the same synthetic 3,000-document bridge workload, a real 3,000-file
Launch Services handoff, and the safe 25-document preparation probe. No real
print job was submitted.

## Before, audit fix, and final verification

| Metric | Original baseline | Audit after | Final verification | Result |
| --- | ---: | ---: | ---: | --- |
| 3,000-file external handoff | >30 s AX timeout, ~90% CPU, sampled 1.7 GiB | <1% CPU by ~3.7 s, ~144 MiB peak | responsive; 152,128 KiB peak; CPU fell from 52.4% to 4.6% by sample 3 | PASS |
| 3,000-file bridge add | 312.729 ms | 207.823 ms | 163.782 ms median | INFO — cache/filesystem sensitive |
| 3,000-item snapshot | 14.040 ms | 9.928 ms | 9.134 ms median | PASS |
| Sort A–Z | 8.583 ms | 2.741 ms | 2.674 ms median | PASS — 68.8% below baseline |
| Sort Z–A | 8.592 ms | 2.669 ms | 2.575 ms median | PASS — 70.0% below baseline |
| Engine reorder | 0.024 ms | 0.015 ms | 0.013 ms median | PASS |
| Prepare 25 text documents | 2,930.465 ms | 2,302.688 ms | 2,431.140 ms | WARN — 17.0% below baseline; per-file process cost remains |
| Empty idle CPU | usually 0%, spikes to 6.5% | 0.0% in 9/10 samples | 0.0% in 9/10 samples; one 0.1% | PASS |
| Empty idle RSS | 45,536 KiB at sample 10 | 30,848 KiB at sample 10 | settled to 41,472 KiB by sample 20 | PASS — framework reclamation varies |

The final five bridge probe runs are recorded in
[`evidence/2026-10-29-mass-print-verification.txt`](evidence/2026-10-29-mass-print-verification.txt).

## Defects and verification

### P1 — runaway external file handoff

- Reproduction: open the application with 3,000 synthetic files in one Launch
  Services request.
- Original defect: sustained high CPU, an accessibility timeout beyond 30
  seconds, and excessive memory growth.
- Fix: a single external-event scene plus a 50 ms thread-safe ingress coalescer.
- Verification: the real UI reported all 3,000 documents and remained
  responsive. Peak observed RSS was 152,128 KiB and CPU rapidly subsided. PASS.

### P1 — main-thread bridge work and cancellation ordering

- Reproduction: mutate or snapshot a large queue while the application is
  interactive, then cancel a batch.
- Fix: ordered engine work moved to a background queue; cancellation uses a
  separate queue so it is not blocked by the batch.
- Verification: workspace tests, the fake-spooler cancellation regression,
  the bridge probe, and the launched 3,000-document UI all pass. PASS.

### P2 — repeated natural-sort tokenization

- Reproduction: sort 3,000 names containing accents and numeric components in
  both directions.
- Fix: compute and cache one natural-sort key per filename for each sort.
- Verification: final A–Z/Z–A medians are 2.674/2.575 ms, respectively 68.8%
  and 70.0% below the original baseline, with stable order tests passing. PASS.

### P2 — cancellation safety

- Reproduction: cancel a multi-document batch immediately after its first
  accepted item.
- Fix: injectable in-process print destination and ordered cancellation path.
- Verification: the first synthetic item is accepted, later items are canceled
  in source order, and `/usr/bin/lp` is never invoked. PASS.

## Remaining limits

- **P3 / WARN:** text/image preparation still invokes `cupsfilter` once per
  document. Bounded parallelism was not added because preserving resource use,
  result order, and cancellation needs wider format testing than the evidence
  presently justifies.
- **P3 / WARN:** the prior `leaks` comparison was flat, but macOS would not
  provide full attribution for the ad-hoc signed process. This is not a full
  Instruments leak trace.
- **WARN:** a fresh launch series was invalidated by Launch Services error
  `-600` after three runs. It is excluded instead of being presented as valid
  before/after evidence.

## Safety

All fixtures were synthetic and removed after use. The preparation probe used
only the in-process fake destination. Active print jobs after verification:
**0**.

