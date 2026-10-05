# User-workload performance

## Scope and reproducibility

Performance work starts after the real local CLI journey passed 23 commands,
including pack/export/import, namespace collisions, copy/link, replacement and
removal. Full local Worker/account acceptance was still being repaired when this
baseline was taken; these numbers do not claim that cloud writes were complete.

The goal is responsive command startup and bounded pack/import/update cost on
normal skills and modest tails, not a higher synthetic requests-per-second score.
`scripts/bench.mjs` times the actual existing executable from spawn through exit,
with piped output. It never builds Rust or starts another Worker. Run it only in
an agreed quiet CPU slot, not alongside Cargo or the full integration harness.

```powershell
node scripts/bench.mjs --phase baseline --cli .cache/target/debug/mskill.exe
# Optional: exactly six read-only requests, after all local timings.
node scripts/bench.mjs --phase baseline --remote https://skills-staging.moesegfault.dev
# After an implementation change and integration-owned build:
node scripts/bench.mjs --phase optimized --cli .cache/target/debug/mskill.exe
```

Each phase deletes only its checked `.temp/performance/<phase>` directory. Raw
sample results, executable SHA-256, fixtures and generated archives remain there.
These ignored fixtures are not release artifacts. Reproduce the summary using
the command and fixture construction in the committed script.

### Environment and method (2026-10-05)

- Windows 10.0.26200 x64, Intel Core i9-12900H, 20 logical CPUs, 34,087,665,664
  bytes reported RAM; Node v26.10.0.
- Existing **debug** CLI, 13,888,512 bytes. Baseline executable SHA-256:
  `c62788718b36cc4dc731419ddb8f6dae0a3b6ffef7dd73c1000d5dabfdd594f5`.
  These are not claims about optimized release binaries or Linux/macOS.
- One unrecorded warm-up, 15 startup samples and 5 samples per archive operation.
  `performance.now()` measures wall time, including process startup and JSON.
- Warm OS cache; no eviction, privileged tracing, cold-cache or power-loss claim.
  Fresh import targets are distinct empty homes. No-op import targets already
  contain identical bytes. Fixture generation and independent hash verification
  are outside the timed interval.
- Every command must exit successfully and emit no ANSI into the pipe. Repeated
  packs must have identical archive SHA-256; installed files are independently
  hashed against generated source files. This avoids mistaking skipped work or
  corrupt output for a performance improvement.
- Report sample median, range and median absolute deviation (MAD). A maximum of
  five observations is **not p99**. Sequential pre/post experiments are useful
  for large directional effects but do not eliminate machine-state drift.

Fixtures remain below 8 MiB each and below 1,000 files:

| Fixture | Files | Expanded bytes | Baseline ZIP bytes |
| --- | ---: | ---: | ---: |
| Typical text references | 25 | 98,397 | 4,562 |
| Many small references | 768 | 1,570,915 | 135,188 |
| Large repeated reference | 2 | 6,291,571 | 15,628 |
| Large deterministic pseudorandom reference | 2 | 6,291,575 | 6,292,776 |

The pseudorandom fixture uses deterministic xorshift32 data, not cryptographic
randomness; its purpose is difficult-to-compress bytes. The large fixtures have
one `SKILL.md` and one resource. The many-file fixture models filesystem overhead
without testing the maximum product entry limit.

## Baseline measurements

Raw results: `.temp/performance/baseline/results.json`.

| Actual operation | Samples | Median ms | Observed min–max ms | MAD ms |
| --- | ---: | ---: | ---: | ---: |
| `--help` | 15 | 21.6 | 20.8–22.2 | 0.3 |
| Empty library `list` | 15 | 22.4 | 21.9–25.1 | 0.4 |
| Typical `pack` | 5 | 44.5 | 41.3–46.7 | 0.8 |
| Typical fresh archive `add` | 5 | 56.5 | 52.5–59.4 | 2.2 |
| Typical unchanged archive `add` | 5 | 33.3 | 32.8–34.3 | 0.5 |
| Many files `pack` | 5 | 394.0 | 389.3–423.5 | 3.9 |
| Many files fresh archive `add` | 5 | 524.2 | 502.2–578.7 | 22.0 |
| Many files unchanged archive `add` | 5 | 163.1 | 153.8–166.6 | 3.5 |
| Compressible large `pack` | 5 | 239.3 | 213.0–248.5 | 9.2 |
| Compressible large fresh archive `add` | 5 | 208.6 | 206.4–280.8 | 2.2 |
| Compressible large unchanged archive `add` | 5 | 162.3 | 150.6–164.0 | 1.7 |
| Compressible large directory `add` | 5 | 365.4 | 358.1–372.3 | 4.8 |
| Incompressible large `pack` | 5 | 900.9 | 893.0–952.3 | 7.9 |
| Incompressible large fresh archive `add` | 5 | 179.6 | 171.3–181.8 | 2.1 |
| Incompressible large unchanged archive `add` | 5 | 150.6 | 146.2–159.1 | 4.4 |
| One-skill library `list` | 15 | 25.2 | 22.1–27.8 | 1.0 |

### Mechanism and narrow optimization proposal

Startup is already approximately 22 ms in this debug setup. Help parses before
Tokio initialization and local commands do not construct an HTTP client. There
is no measured reason to redesign CLI startup or introduce a daemon.

Before optimization, the source call path completely inspected/decompressed the
same immutable archive **three times** on archive `add`, **four times** on
directory `add`, and **twice** on directory `pack`:

```text
archive add:  pack_or_read.inspect -> add.inspect -> install_bytes.inspect -> extraction
directory add: pack.inspect -> pack_or_read.inspect -> add.inspect -> install_bytes.inspect -> extraction
directory pack: pack.inspect -> pack_or_read.inspect
```

The unchanged-import path performs no extraction but still costs 151–163 ms for
the larger fixtures versus 22 ms process startup. This discriminates redundant
validation from extraction I/O as a worthwhile cost in no-op updates. Fresh
many-file import is slower still, consistent with staging 768 separate files;
that comparison is not a syscall profile or proof that filesystem writes are
the only bottleneck. Incompressible packing is the largest recorded operation,
but these debug numbers do not justify changing the portable compression contract.

Proposal: a private validated archive value carries the immutable bytes and
`ArchiveInspection` result through the local call chain. Public `pack`,
`pack_or_read` and `install_bytes` keep their contracts; untrusted archive bytes
still receive one complete shared-protocol check before any committed mutation.
Do not skip validation solely because caller-supplied hashes match, merge unsafe
extraction with validation, expose an unchecked public API, or change canonical
ZIP compression/timestamps/permissions to improve a benchmark. Core owns this
implementation and integration owns the rebuild; performance owns the comparable
rerun. The full real local journey must remain green.

## Implemented change and comparable rerun

Core implemented private `ValidatedArchive<B>` containing immutable bytes and
the shared `ArchiveInspection`. Its constructor performs full validation once;
the private install path accepts that value rather than inspecting its bytes
again. Public `install_bytes` still invokes the constructor for untrusted input,
including identical bytes. The writer, compression level, ordering, timestamps,
permissions, hash calculation, rollback and extraction code were not changed.
Archive `add` now checks once rather than three times; directory `add` once
rather than four; CLI directory `pack` once rather than twice. Changed installs
still decompress again when materializing the validated tree.

The integration-owned combined native rebuild also contained the Windows
credential-vault correction; the timed local commands do not log in or access
the vault. The optimized executable was 13,895,680 bytes, SHA-256
`7a34905b431887049fea5894ce5b68d00f5570d52739676cc93016648d2f3579`.
Integration reported **16 native unit tests, one documentation test and the
23-command actual local journey passing** on that rebuilt binary. New core
test instrumentation also checks the one-validation paths and rejects corrupt
bytes at the unchanged public boundary.

Same quiet-slot machine, same command construction and fixture bytes, same
one warm-up and sample counts. No cloud requests were repeated. Raw rerun:
`.temp/performance/optimized/results.json`. The script independently verified
extracted file hashes and no-op archive modification timestamps. A deep equality
comparison of all fixture names, file counts, expanded sizes, compressed sizes
and archive SHA-256 values against the baseline **passed**. Portable archive
bytes were therefore preserved for every measured fixture.

| Actual operation | Baseline median ms | Optimized median ms | Median time reduction | Optimized observed range ms | Optimized MAD ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| `--help` | 21.6 | 21.7 | -0.5% | 21.2–22.3 | 0.2 |
| Empty library `list` | 22.4 | 22.5 | -0.2% | 21.9–32.2 | 0.3 |
| Typical `pack` | 44.5 | 41.0 | 7.8% | 39.9–47.2 | 1.1 |
| Typical fresh archive `add` | 56.5 | 51.0 | 9.6% | 49.8–54.3 | 1.2 |
| Typical unchanged archive `add` | 33.3 | 28.0 | 16.0% | 27.8–29.8 | 0.1 |
| Many files `pack` | 394.0 | 353.6 | 10.3% | 344.6–375.3 | 6.9 |
| Many files fresh archive `add` | 524.2 | 428.0 | 18.4% | 418.9–469.4 | 9.1 |
| Many files unchanged archive `add` | 163.1 | 71.7 | 56.0% | 71.0–78.2 | 0.8 |
| Compressible large `pack` | 239.3 | 179.2 | 25.1% | 172.9–192.6 | 2.5 |
| Compressible large fresh archive `add` | 208.6 | 128.4 | 38.5% | 122.4–130.2 | 1.6 |
| Compressible large unchanged archive `add` | 162.3 | 70.9 | 56.3% | 67.2–84.4 | 1.3 |
| Compressible large directory `add` | 365.4 | 234.3 | 35.9% | 229.9–243.9 | 4.4 |
| Incompressible large `pack` | 900.9 | 872.5 | 3.2% | 850.1–873.1 | 0.6 |
| Incompressible large fresh archive `add` | 179.6 | 156.2 | 13.1% | 151.1–159.2 | 2.2 |
| Incompressible large unchanged archive `add` | 150.6 | 130.6 | 13.3% | 124.8–131.6 | 1.0 |
| One-skill library `list` | 25.2 | 24.2 | 4.0% | 22.6–65.4 | 1.0 |

Archive operations have five samples each; startup/list have fifteen. Percentage
is `(baseline_median - optimized_median) / baseline_median`, not a throughput
improvement or a confidence interval. Startup differences are small enough not
to justify a speedup claim. One populated-list rerun observation was 65.4 ms,
above its baseline maximum 27.8 ms; listing code was not modified and the medians
are similar, but the experiment does **not** establish an improvement in startup
tails. Preserve this negative observation rather than hiding it.

### Interpretation and stopping decision

The narrow change is worthwhile: many-file and compressible unchanged imports
save about **91 ms**, over half their previous process time; fresh imports save
about 96 ms and 80 ms respectively; directory-add saves about 131 ms. These
effects are much larger than their observed within-run dispersion and match
the eliminated repeated-validation mechanism. There is no added network cache,
compression mode, daemon, public unchecked API or retained archive copy. Existing
archive bytes and user behavior remain unchanged in the tested workloads.

The remaining largest operation is debug-build packing of 6 MiB incompressible
data at about 873 ms. Only 3.2% changed there; do not generalize validation reuse
as a major compression optimization. Many-file extraction still costs more than
one-file extraction. Public CLI `update --from` still crosses `pack_or_read`
and `install_bytes` independently and therefore checks twice; cloud hash-match
updates follow a different metadata/network path. No broader update claim is
made from the `add` benchmark.

Stop here for this delivery. The measured simplification is integrated and the
important normal/tail archive workloads are bounded. A separate release-build
profile would be warranted only if users experience packaging delay; this debug
experiment alone does not justify new compression machinery or storage tuning.

### Bounded staging observations

Exactly three sequential requests to each public endpoint, after local timings:

| Endpoint | All end-to-end samples ms |
| --- | --- |
| `/health` | 1128.5, 557.0, 532.2 |
| `/v1/skills` | 293.4, 279.7, 300.6 |

These times drain the response body and include this machine's DNS/TLS/network,
Cloudflare routing and Worker/storage work. The first request establishes a
connection; no separate warm-up is hidden. They cannot establish storage latency,
Worker CPU time, or cloud p99. The separately deployed **3 ms Worker startup**
reported in `docs/delivery.md` is a different metric. Keep streamed R2 downloads;
do not add caching or a load test based on these six observations.

## Measurement rationale

[Google Benchmark's production user guide](https://github.com/google/benchmark/blob/main/docs/user_guide.md)
supports warm-up, repetitions and preserving result statistics instead of one
timing. Its [random-interleaving guidance](https://github.com/google/benchmark/blob/main/docs/random_interleaving.md)
explains how ordering can confound small pre/post comparisons. Here the low-cost
whole-process harness is deliberately not a microbenchmark framework; use a
short interleaved A/B check only if a small change affects the decision.

Kalibera and Jones,
[Rigorous Benchmarking in Reasonable Time (ISMM 2013)](https://kar.kent.ac.uk/33611/),
analyzes multiple sources of experimental nondeterminism and budgeted replication.
The applicable research lesson is to state the experiment level and uncertainty,
not treat five process samples on one build/machine as general confidence bounds.
No speculative new compression or storage research is needed to remove repeated
work on already validated immutable bytes.
