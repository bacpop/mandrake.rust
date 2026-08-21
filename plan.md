Read AGENTS.md before starting work.

This file is the working plan. Keep it up to date.

At the beginning of each session:
- Read this file completely.
- Verify the Current Task still matches the repository.
- If not, update it before writing code.

During work:
- Mark completed checklist items.
- Record any important design decisions.
- Record blockers immediately.

Before finishing:
- Update Current Status.
- Update Next Task so another session can continue without context.
- Record any deviations from the original plan.

## Definition of Done

- [x] Builds with `cargo build`
- [x] Tests pass
- [x] Public API documented
- [x] plan.md updated

---

# Project

## Goal

Port the C++ package `mandrake` to Rust, then integrate it as a wasm
module in a vue app.

Target:
- `rust/`
- standalone Cargo crate
- `www/` for the vue app.

Out of scope:
- CUDA (`*.cu`, `*.cuh`)
- Python bindings unless explicitly requested

---

# Engineering Principles

Priority:

1. Computational efficiency
2. Memory efficiency
3. Readability
4. Error reporting

Mirror the style of [sketchlib.rust]((https://github.com/bacpop/sketchlib.rust):

- clap
- anyhow
- log/env_logger
- rayon where appropriate
- wasm32 compatibility
- cargo test

---

# Public Interface

Required API:

```rust
pub fn wtsne(input: EmbeddingInput, options: &WtsneOptions) -> Result<Array2<f64>>
```

The cooperative `EmbeddingOperation` API is the canonical interface; `wtsne`
is its blocking convenience wrapper and returns only the final ndarray
embedding.

Expose:

- Rust library
- End-to-end CLI for one distance source and embedding output

---

# Current Task

## Objective

Add a reproducible Playwright test harness for the browser tool. The harness
must start the Vue dev server itself, use repository-relative tracked fixtures,
exercise the gzip accessory path and HDBSCAN result controls, and report page or
console errors without waiting for the slow compressed alignment fixture.

### Checklist

- [x] Add the pinned serial HDBSCAN dependency and pure Rust clustering helper
- [x] Export final-embedding clustering through the feature-free wasm boundary
- [x] Add the browser option, final-label switch, cluster count, and noise styling
- [x] Add Python-compatible cluster CSV generation and download
- [x] Add native, Node-wasm, and Chromium verification for labels and downloads
- [x] Add the local Playwright dependency, config, and reproducible browser tests
- [x] Update plan status, design notes, session log, and next task

### Current Status

Completed:

The HDBSCAN final-embedding phase is complete. This phase adds a committed
Playwright harness around the existing browser workflows; it deliberately tests
the tracked `.Rtab.gz` fixture but does not wait for the slow `.fas.gz` fixture.

Last verified:

2026-08-21: the previous phase's native tests and Clippy, feature-free wasm
checks, production Vue build, deterministic Node wasm oracle, compressed real
fixture, and temporary Chromium HDBSCAN smoke passed; the committed Playwright
suite now passes both focused Chromium tests.

Blockers:

No implementation blockers. The `wasm-bindgen-file-reader` GPL-3.0 licensing
follow-up remains deferred from the gzip phase. Browser binaries remain an
external Playwright cache installed by `npm run playwright:install`.

---

# Design Notes

## Mandrake web tool first pass (completed)

- In scope: a new `www/` Vue 3 package following Sparrowhawk's shell and page
  layout; a wasm-bindgen stateful handle; alignment and Roary-style accessory
  byte uploads; kNN or threshold sparsification; perplexity, max updates,
  repulsion samples, learning rate, and initial exaggeration controls; a
  queued worker that advances the operation; a final SVG scatter plot; a
  progress percentage; and downloads of `<prefix>.embedding.txt` and
  `<prefix>.names.txt`.
- Out of scope for that first pass: existing `.skd`/`.skm` loading, browser sketch
  generation, intermediate-frame rendering, label-file colouring, and HDBSCAN.
  These require separate wasm-compatible sketch/label boundaries or additional
  UI state and become the next implementation phase.
- The page owns presentation state; the worker owns the wasm handle and is the
  only code that advances it. Messages are serialized through an explicit
  promise queue, and progress fields use `completed`/`maximum` rather than a
  truthy `done` count.
- The correctness oracle for this phase is the existing native Rust test suite
  plus the feature-free `wasm32-unknown-unknown` library check. The public
  operation is intentionally time-seeded, so browser and native coordinates
  are checked for shape, finiteness, and completion rather than byte equality;
  a dedicated deterministic Node oracle is deferred with the later browser
  verification phase.

## Mandrake web interface phase

- Distance construction becomes a wasm-only cooperative state machine: input
  constructors parse and retain source data, `advanceDistances` computes bounded
  row batches, and `beginEmbedding` explicitly transitions to SCE. Native
  distance constructors retain their existing parallel implementation and
  sparse-output contract.
- The worker reports distance and embedding progress separately. It emits an
  initial embedding and then only the latest state at 5% update thresholds;
  browser history is not retained and no Rust frame schedule is reintroduced.
- User labels use the Python plotting contract: an optional unheadered
  `sample-name<TAB>label` file must have exact, unique sample-name coverage.
  Labels are aligned to embedding order and rendered with deterministic
  categorical colours.
- The output plot uses Plotly `scattergl` with responsive zoom/pan/reset,
  sample-name hover text, equal axis scaling, and cleanup on component removal.
- `WtsneOptions::default` and the web control default to 1,000,000 updates; the
  web number input steps by 1,000,000. Existing explicit smaller test budgets
  remain unchanged.
- This phase does not add `.skd`/`.skm` loading, sketch generation, HDBSCAN, or
  deterministic coordinate comparison; those remain the following task.

## Browser gzip input phase (completed)

- Scope is gzip only, for both alignment/FASTQ and Roary-style Rtab/TSV input.
- Follow Sparrowhawk's approach: pass the selected `web_sys::File` through the
  worker, use `wasm-bindgen-file-reader` for fixed-size synchronous `Read`
  slices, inspect gzip magic bytes, and chain the prefix into
  `flate2::read::MultiGzDecoder` before the existing parsers.
- Keep the byte-based wasm constructors for existing Node/raw-byte callers and
  share the resulting operation construction with the new `File` constructors.
- Add the dependency as Sparrowhawk does and record its GPL-3.0 licensing as a
  separate follow-up; do not change the repository licence in this phase.
- `needletail` requires `Send` for its parser trait, so the file-reader enum has
  a wasm-only `Send` implementation guarded by the single-worker wasm invariant;
  no browser file object is handed to another thread.

## Browser visual/input refinement phase (completed)

- Add a generic `.gz` token to the primary file input's `accept` list so Firefox
  filters by the final compressed suffix; retain case-insensitive underlying
  suffix detection and parser validation rather than accepting arbitrary gzip
  content.
- Keep the existing approximately 20-frame schedule for smaller embedding
  budgets, but cap its interval at 20,000 updates with
  `min(ceil(maximum / 20), 20_000)` so large runs do not look choppy.
- Import the supplied `mandrake_logo.png` through the Vue asset pipeline and
  use it for the sidebar brand mark, page heading mark, and empty-state mark.
  The images remain decorative because the adjacent text carries the identity.
- Do not add a permanent end-to-end harness in this focused pass. Verification
  uses the existing Chromium smokes plus a temporary visual smoke for asset
  loading, the generic picker token, responsive layout, and live-frame updates;
  Firefox picker selection is manual because no Playwright Firefox binary is
  installed locally.

## Browser HDBSCAN labelling phase

- Pin the pure-Rust `hdbscan` crate at `0.12.0` with its serial feature so the
  same implementation is available to native tests and the feature-free wasm
  build. Use a private helper over row-major two-dimensional embeddings and a
  wasm `clusterEmbedding` export returning signed labels, where `-1` is noise.
- Match the existing Python preset: centre each dimension, divide by half its
  range, use Euclidean distance, `min_cluster_size=2`, `min_samples=2`,
  `epsilon=0.02`, automatic nearest-neighbour selection, and allow a single
  cluster. Reject fewer than two samples, malformed/non-finite coordinates, or
  zero-range dimensions with a recoverable error.
- Add a checkbox to request clustering after final optimisation. The worker
  posts a labelling state, invokes clustering only after the final embedding,
  and preserves the completed embedding if labelling fails.
- Preserve manual labels during live updates. If both sources succeed, expose
  an accessible colour-scheme toggle and default it to manual labels; if only
  HDBSCAN succeeds, select clusters automatically. Plot cluster labels as
  `Cluster N` and render `Noise` in a separate subdued black trace.
- Report the number of distinct non-noise cluster IDs in the result summary,
  explicitly showing that zero clusters were found when every sample is noise.
- Add a `Download clusters` action that writes
  `<prefix>.embedding_hdbscan_clusters.csv` with the Python-compatible
  `id,hdbscan_cluster__autocolour` header, one CSV-escaped row per sample in
  embedding order. Do not expose the button when clustering was not requested
  or failed.
- Keep cluster IDs deterministic for the pinned Rust implementation, but do
  not promise numeric parity with another HDBSCAN implementation. Validate the
  native helper against a separated-cluster/noise fixture and compare the Node
  wasm export to that native result.

## Reproducible Playwright browser phase

- Keep the test runner in `www/` with pinned `@playwright/test` 1.62.1, a
  checked-in `playwright.config.ts`, and `www/e2e/mandrake.spec.ts`. The config starts
  `npm run serve -- --port 8080 --host 127.0.0.1`, so tests do not depend on a
  manually running server or an npx cache import.
- Resolve the accessory fixture from the test file's path to the repository's
  tracked `tests/fixtures/gene_presence_absence.Rtab.gz`; do not embed an
  absolute developer path. The compressed alignment fixture is intentionally
  excluded because its embedding runtime is too long for this smoke suite.
- Use one tracked gzip completion test and one small in-memory accessory test
  for HDBSCAN controls and CSV download. Both collect page errors and console
  errors and fail after the relevant result state; the small test keeps UI
  coverage fast without changing production defaults.
- Playwright browser binaries remain outside the repository and are installed
  explicitly with `npm run playwright:install` (Chromium only).

## Python plotting CLI

- Use a required mutually exclusive `--labels LABELS.tsv` / `--hdbscan` group;
  the input positional argument is a prefix, and every output keeps that same
  prefix.
- User labels are read from an unheadered two-column TSV and aligned by exact
  sample-name identity to `.names.txt`; duplicate, missing, extra, malformed,
  or empty sample IDs are rejected before plotting.
- User labels are ordinary categorical labels (`dbscan=False`); HDBSCAN labels
  retain the plotting helpers' noise-point behavior and also produce the
  existing cluster CSV.
- Verification is intentionally limited to static review and documentation;
  no CLI invocation, help check, or dedicated failure tests are added.

## Distance efficiency pass

- `Sparsification::Knn(k)` will use one uniform exact-k contract for alignment
  and accessory inputs: retain `min(k, n - 1)` non-self neighbours per row,
  with equal distances resolved by ascending column index. `Knn(0)` retains
  every non-self neighbour. This intentionally replaces alignment's legacy
  self-edge and tie-expansion behaviour so row construction can use bounded
  candidate storage and both sources expose the same meaning of kNN.
- `Sparsification::Threshold(t)` will uniformly retain only edges whose
  normalized distance is strictly less than `t`. Filtering occurs as each
  distance is generated rather than after a full row is materialized. Thus a
  threshold of zero retains no edges and a threshold of one excludes edges at
  normalized distance exactly one.
- `SparseDistances` COO edge order is explicitly unspecified. SCE reconstructs
  row membership from the row-index vector and does not require sorted input,
  so bounded kNN heaps may be drained directly without sorting. Reordering can
  still change floating-point accumulation and internally seeded test results.
- Remove the public `WtsneOptions::seed` field and CLI `--seed` option. The
  private optimiser constructor accepts `Option<u64>`: internal deterministic
  tests pass `Some(seed)`, while all public entry points pass `None` and derive
  a seed from system time. Use `web_time::SystemTime` so this works on native
  targets and browser-hosted `wasm32-unknown-unknown`; no public reproducibility
  guarantee remains.
- Retain standalone Roary-style accessory-table input, including CLI
  `--accessory` and its public constructors. Remove sketch `--use-accessory`
  and its streaming accessory-distance implementation; sketch distances become
  core-only. The standalone alignment and accessory paths remain the two
  source adapters that need a shared row-construction seam.
- Add one private deep row-construction module whose interface accepts sample
  names, `DistanceOptions`, and a pair-distance closure. It owns the configured
  pool, row-parallel traversal, progress, exact-k heaps, generation-time
  threshold filtering, and final COO assembly. Alignment and accessory parsing
  remain source-specific adapters. A private closure is sufficient because
  pair-distance calculation is the only varying operation; do not add a public
  or private distance trait solely for this seam.
- Represent each standalone accessory sample as one `RoaringBitmap` containing
  the row indices of genes present in that sample, replacing its dense
  `Vec<u8>`. Compute Jaccard distance from `intersection_len` and `union_len`
  without materializing temporary bitmaps.
- Keep lock-free row ownership by calculating both directed forms of each
  symmetric pair independently. This accepts doubled pair-distance computation
  in exchange for `O(k)` auxiliary storage per active kNN row and no shared heap
  synchronization or per-thread heap sets. kNN output is directed: retaining
  `(i, j)` does not require retaining `(j, i)`.
- Each parallel row returns an unsorted `Vec<(column, distance)>`. After row
  construction, sum retained lengths, allocate the three flat COO vectors once,
  and append each row while generating its repeated row index. This is the one
  unavoidable `O(retained edges)` assembly pass; temporary candidates do not
  store redundant row indices, and assembly needs no sorting, locks, unsafe
  concurrent writes, or repeated flat-vector growth.
- Implementation verification will compare public COO edge sets without
  assuming order and cover exact k, self-edge exclusion, deterministic tie
  selection, directed/non-symmetric kNN, strict threshold boundaries for both
  standalone sources, and the private selector's `heap.len() <= k` invariant.
  Run the existing native, feature-free, and wasm checks. The user will perform
  runtime and peak-RSS benchmarking externally, so no in-repository benchmark
  or performance gate is part of this task's completion criteria.
- ADR 0004 records the deliberate compatibility and compute-for-memory
  trade-offs in the row-owned sparse-distance construction contract.
- Implementation uses a `BinaryHeap` max-heap whose ordering treats larger
  distance and, on ties, larger column index as the worst candidate. Draining
  the heap directly intentionally leaves each row unsorted.
- Public construction resolves a `None` seed once from `web_time::SystemTime`;
  only private unit tests can pass `Some(seed)`. The public options and CLI no
  longer expose seed configuration.
- The sketch distance constructor signatures are now core-only and no longer
  accept an accessory selector; standalone `.Rtab` accessory constructors and
  the CLI `--accessory` path remain available.

## Code and style refactor

- Replace the CLI's optional `env_logger` dependency with `simple_logger`.
  Parse arguments before logger setup and select `Error` for `--quiet`, `Info`
  for `--verbose`/`-v`, and `Warn` by default. `--quiet` and `--verbose` are
  mutually exclusive; retain the `--no-progress` alias. `-vv` is not a second
  verbosity level, and `--verbose` leaves progress bars enabled.
- Emit concise `log::info!` phase messages from the code that owns each phase.
  Path loaders report source loading, reader constructors report parsed sample
  counts, shared distance construction reports start/completion and retained
  edge counts, embedding operations report probability preprocessing,
  initialization, and optimisation, and the CLI reports output writing. Do
  not log individual records, updates, allocations, or helper substeps. The
  CLI emits one final total elapsed-time measurement after outputs are written;
  no phase timings are tracked.
- Move CLI scalar validation into named `clap` value parsers and declarative
  argument constraints. Preserve current CLI semantics for positive integer
  settings, positive finite learning rate, positive kNN, and finite threshold
  in `(0, 1]`; require CLI perplexity in inclusive `[5.0, 100.0]`. The public
  Rust API continues to accept non-positive perplexity for raw-similarity mode
  and retains its own cheap validation for programmatic callers.
- Make `parse_sparsification` an infallible mapping after `clap` validation and
  validate distance options once at each public constructor boundary. Keep
  parser-integrity checks such as alignment lengths and accessory row shape,
  but remove full-vector debug scans from `SparseDistances::new`; its remaining
  checks are constant-time structural checks.
- Do not add tests in this pass. Use the existing test suite, manual CLI
  checks, formatting, lint, documentation, and diff checks as verification.
- Update README and CLI-facing documentation. No ADR or glossary entry is
  needed unless implementation reveals a durable domain trade-off.

## Parallelism

- Prefer Rayon.
- Atomic floating-point updates are required for embedding writes; use a CAS
  loop over `AtomicU64` bit patterns and retain the C++ optimistic clash retry.
- Replace the public `workers` setting with `threads`. `WtsneOptions::threads`
  and CLI `--threads` default to one and govern every native parallel section:
  distance construction, conditional probabilities, embedding initialization,
  and SCE updates. The setting is accepted on wasm for API compatibility, but
  wasm execution remains sequential and values above one have no effect.
- Internal deterministic tests may inject a fixed seed with `threads = 1`.
  Public calls are automatically seeded, and multi-threaded SCE updates may
  also vary with scheduling, so public verification targets finite output and
  intended behaviour rather than coordinate equality.
- `max_updates` is a thread-independent total update-attempt target, not a
  count of parallel rounds. Progress reports completed update attempts. Each
  internal parallel round performs `threads` attempts. A run may finish up to
  `threads - 1` attempts above its requested budget rather than execute a final
  partial round, so thread count changes concurrency and may make total work
  differ slightly while staying within one thread batch of the target.
- `EmbeddingOperation::advance` accepts a round budget, not a literal update
  count: each non-zero round performs one full native thread batch. A zero
  budget remains a no-op poll. This keeps cooperative polling live without
  requiring partial batches.
- Make the unit distinction explicit in the public API: rename the configured
  and reported fields/accessors to `max_updates` and `completed_updates`.
  `advance` is documented as accepting parallel rounds. This is an intentional
  clean API break that prevents callers from treating progress units and
  advance units as interchangeable.
- Each native `EmbeddingOperation` owns a Rayon pool configured by `threads`.
  Run its construction-time conditional-probability and initialization work in
  that pool. Do not configure Rayon globally: repeated operations must remain
  valid. Wasm retains Rayon's sequential fallback.
- Expose an explicit thread setting to every public distance constructor and
  CLI distance-loading path through `DistanceOptions`. It contains
  `sparsification`, `threads`, and `quiet`, defaults to `Knn(0)`, one thread,
  and progress enabled, and is shared by every public distance constructor.
  Each native call creates and uses a per-call configured Rayon pool, so
  distance construction shares the one-setting contract without process-global
  state.
- Public native distance constructors and embedding configuration expose a
  `quiet` option, defaulting to false. When it is false, library callers
  receive one `indicatif` phase bar at a time for distance construction,
  conditional-probability
  preprocessing, and optimisation; when true, those bars are suppressed.
  Structured operation progress remains available independently, and wasm
  remains terminal-bar-free.
- The optimisation bar has length `max_updates`, caps its visual position at
  that target, and reports the actual completed-update count on finish. This
  accommodates a final full thread batch that may exceed the target.
- Keep the existing atomic-CAS embedding updates and optimistic clash retry for
  concurrent SCE updates. Do not introduce per-thread full-embedding delta
  buffers and a reduction step: their `O(nodes * threads)` peak memory cost
  conflicts with Mandrake's memory-efficiency priority and would change the
  in-place update semantics.
- Sketchlib's streaming accessory API needs a coordinator lane while its
  producer is running inside a scoped Rayon pool, so a single-thread accessory
  call uses a two-lane private pool while still passing the requested thread
  count to sketchlib's distance workers.
- The progress implementation is behind a positive `progress` feature enabled
  by the native CLI/default feature set. `quiet` remains in the public options
  on feature-free and wasm builds, where the bar implementation is a no-op.

## RNG

- Use `rand_xoshiro::Xoshiro256PlusPlus` through its `RngCore` and
  `SeedableRng` interfaces.
- Resolve the operation seed once: public construction derives it from system
  time, while a private `Option<u64>` injection point permits deterministic
  tests. Initialize a root, domain-separated update RNG from that seed. Create
  the small fixed set of executor streams by successive
  `Xoshiro256PlusPlus::jump()` calls, then retain and advance those streams
  normally. Never reseed per update or repeatedly jump from the initial state
  to catch up with progress.
- Retain domain-separated `seed_from_u64` streams for initial embedding points.
- Deterministic tests preserve same-seed reproducibility internally, but seed
  selection and reproducibility are not part of the public API contract.

## Progress

The library returns `EmbeddingProgress` from each synchronous advance. Native
library and CLI callers may enable one phase-specific `indicatif` bar at a time
through their `quiet` options; the portable wasm core owns no terminal UI.

## Conditional probabilities

Port implementation from `wtsne.hpp`.
- Preserve a valid uniform row when exponentials underflow for extreme finite
  distances, then normalize the complete edge-probability vector.

## v1 API and scope

- Public input is an owned `EmbeddingInput` containing zero-based COO `I`/`J`
  endpoint vectors, distance values, an explicit node count, and optional
  weights.
- `wtsne(...)` returns `Result<Array2<f64>>`; the embedding has shape
  `(n_nodes, 2)`.
- `WtsneOptions` carries perplexity, target update count, repulsion samples,
  learning rate, initial exaggeration, thread count, and quiet setting. Seed
  selection is private and automatic for public callers.
  Retained frame schedules are removed; native progress bars are controlled by
  the quiet setting.
- Tests target public behavior: validation, normalization, finite output, and
  successful parallel execution. Private optimiser tests may inject a seed for
  deterministic lifecycle assertions.
- No Python bindings or CUDA are part of this milestone.
- Distance constructors return `SparseDistances` and accept `DistanceOptions`,
  whose sparsification field supports `Sparsification::Knn` or
  `Sparsification::Threshold` where the source supports that mode.

## Distance-input phase decisions

- All three sources return a public `SparseDistances` value containing sample
  names and zero-based COO row, column, and normalized-distance vectors.
- Use released crates.io `sketchlib = "0.4.1"` for file-compatible `.skm`/`.skd`
  loading and sparse kNN distances.
- Pair-SNP alignment and standalone accessory input now share exact non-self
  kNN and strict-threshold semantics through the confirmed distance-efficiency
  design. This intentionally supersedes the earlier decision to retain legacy
  pair-SNP self edges and tie expansion.
- Sketch inputs support kNN only in this phase. Threshold mode is rejected for
  sketches because the released API does not provide a streaming threshold
  operation and dense all-pairs materialization would defeat sparse input.
- The CLI accepts one source at a time, runs `wtsne`, and writes
  `<output>.embedding.txt` plus `<output>.names.txt`; plotting and clustering
  remain outside this phase.
- Use the existing `rust/src/gene.rs` parsing semantics (IUPAC overlap,
  unknown-base matching, and its tested gap handling) rather than duplicating
  the parser.
- Sketch distance input is core-only. The former streamed multi-kmer accessory
  mode and its `--use-accessory` switch are removed by the distance-efficiency
  pass.

## Module seams for the refactor

- `api.rs` owns public SCE configuration, input, operation, progress, and
  embedding-accessor types; `lib.rs` remains a thin facade with root
  re-exports.
- `sce.rs` owns the cooperative operation, blocking `wtsne()`, and optimiser
  implementation details, including probabilities, sampling, and atomic
  embedding updates. It retains only the current embedding.
- `distances/` is split into portable reader-based constructors and native
  path/sketch adapters; gene parsing remains private to the distance module.
- `main.rs` becomes a minimal entry point; binary-only argument parsing,
  source dispatch, embedding invocation, and output writing move to `cli.rs`.
- The public interface is tested through root re-exports and CLI behaviour;
  internal seams remain private implementation details.
- The initial embedding uses domain-separated per-point RNG seeds. SCE update
  streams are persistent disjoint Xoshiro streams created once from a root
  stream with `jump()`, never reseeded or replayed to catch up. Unit sampling
  uses the full-width `next_u64` output over `[0, 1)`. Private seed injection
  retains deterministic internal single-thread tests; public operations are
  automatically seeded from system time.

## Wasm/async API transition

- Replace the retained-frame result model with a caller-owned, cooperatively
  stepped embedding operation. The caller advances bounded work and may poll
  the current state between steps; this avoids requiring threads, an executor,
  or a particular async runtime on `wasm32-unknown-unknown`.
- Each advance accepts a caller-selected parallel-round budget, so UI callers can
  choose short work units while native callers can choose larger batches.
- `advance` returns lightweight progress/completion metadata. The operation
  retains one current `Array2` embedding, replacing it after every completed
  update round; callers retrieve that state through a separate accessor. It never
  retains a historical frame series.
- The initialized zero-update embedding is available through that accessor as
  soon as the operation is created.
- Advancing a completed operation is idempotent: it leaves the final embedding
  unchanged and returns completion again rather than reporting an error.
- Dropping a caller-owned operation is cancellation; no explicit cancellation
  state or method is needed because work occurs only during `advance`.
- Construction performs input validation and setup and is fallible. Once an
  operation is created successfully, `advance` is infallible and returns only
  progress/completion metadata.
- Construction takes ownership of the sparse input and weights the operation
  needs for subsequent advances, avoiding an implicit copy of large inputs.
- Retain `wtsne` as a blocking convenience wrapper that constructs an operation,
  advances it to completion, and returns only the final embedding. The
  cooperative operation remains the canonical interface.
- Each advance reports completed updates, configured maximum updates, and the
  `Eq` convergence statistic. Worker-update counts are removed from the public
  API because workers are not part of the intended long-term model.
- A zero round budget is a valid no-op poll. `Eq` remains diagnostic and
  must not cause convergence-based early termination.
- The configured maximum update count is the total-work target. A native run
  completes full thread batches and can exceed it by at most `threads - 1`;
  wasm completes exactly at the target.
- Split distance code into focused files: portable alignment/accessory
  constructors accept caller-provided readers, while path-opening and
  compression wrappers are native-only. Sketchlib-backed constructors are
  separately feature-gated because the target wasm application supplies that
  functionality outside Mandrake.
- Use positive optional `native-inputs` and `sketchlib` Cargo features, enabled
  by default for native compatibility. The portable wasm build selects
  `--no-default-features` rather than relying on an additive `wasm` feature to
  disable native dependencies.
- The `mandrake` CLI is native-only and requires both `native-inputs` and
  `sketchlib`; a `--no-default-features` wasm build exposes the library core,
  not a degraded path-oriented command.
- Portable reader-based distance constructors consume already-decompressed
  bytes. Compression detection and decoding remain native path-loader or
  caller responsibilities.
- `SparseDistances` remains the labeled distance-input value. The numerical,
  owned `EmbeddingInput` is distinct and contains only COO vectors and weights;
  names remain with the caller rather than the long-lived operation.
- `EmbeddingInput` construction accepts `Option<Vec<f64>>` weights. It moves
  supplied weights unchanged; `None` explicitly selects a newly created
  uniform-weight vector.
- `EmbeddingInput::new` takes owned COO vectors, explicit `n_nodes`, and
  optional weights directly. It neither consumes sample labels nor copies
  supplied numerical inputs.
- Input construction performs only constant-time structural validation (for
  example, matching COO lengths and declared vector lengths). It must not scan
  large COO, distance, or weight vectors solely to validate values, preserving
  Mandrake's efficiency priority.
- Per-element validation is limited to debug assertions in distance
  construction. The embedding phase performs only basic non-scanning checks
  and treats its owned numeric input as trusted in release builds.
- On `wasm32-unknown-unknown`, use Rayon's sequential fallback rather than an
  explicit `ThreadPoolBuilder`, which Rayon documents as unsupported on that
  target. Native builds retain configured pools per operation or distance call.
- Native library phases render their own optional progress bars; the CLI passes
  its quiet setting through and renders no duplicate bars.
- Make a clean public API break: remove `FrameSchedule`, `SceFrame`, and
  `SceResults`. The retained blocking `wtsne` wrapper consumes `EmbeddingInput`
  and returns `Result<Array2<f64>>`.
- For this phase, the CLI advances a fixed internal round chunk while the
  library optimization phase renders progress after each round. Interval-driven
  rendering from a separate thread is deferred as a non-priority follow-up.
- Define that chunk as `const CLI_ADVANCE_CHUNK: usize = 1_000` in the CLI.
- Advancing with different round-budget partitions should preserve internally
  seeded single-thread results, but this is secondary to performance and must
  not justify added copying, synchronization, or validation overhead.
- Regression coverage may inject a private seed to check budget-partition
  invariance in the single-thread path. Multi-thread tests assert a completed,
  finite embedding whose completed work is no more than `threads - 1` above
  its requested budget; they do not require coordinate equality or an exact
  final attempt count.
- There is no public worker concept: `threads` configures native rounds, wasm
  executes sequentially, and thread count does not appear in progress values.
- Keep the Rust operation synchronous (`advance(&mut self, budget)`); callers
  provide any async/UI scheduling around it. No executor or `Future` is part of
  this phase.
- `EmbeddingOperation::embedding()` borrows the retained current `Array2`; a
  separate `into_embedding()` consumes the operation and transfers the final
  array. Polling does not clone the embedding.
- `advance` returns one `EmbeddingProgress` struct carrying completed and
  maximum update counts plus `Eq`; completion is queried through
  `is_complete()` rather than a separate status enum.
- `into_embedding()` may consume an incomplete operation and return its latest
  state without copying, but emits a `WARN` log unless the operation has
  completed.

## Mandrake web repair

- The worker loads `@/pkg/mandrake` through one cached dynamic import before
  constructing an operation. This lets webpack emit and initialize the async
  wasm module in the worker instead of calling generated glue with an unset
  wasm handle.
- The webpack public path is root-relative so the worker's split JavaScript
  chunk and wasm binary resolve correctly from scripts under `/js/`.
- Input type is detected case-insensitively from FASTA/FASTQ (`.fa`, `.fasta`,
  `.fas`, `.fna`, `.fq`, `.fnq`, `.fastq`) and accessory (`.rtab`, `.tsv`)
  suffixes. Ambiguous `.txt` files are rejected rather than guessed.
- Parameter help uses a small keyboard-accessible hover/focus tooltip
  component populated from the CLI option descriptions; no UI dependency was
  added for this focused repair.

---

# Next Task

- Add HDBSCAN labelling (https://crates.io/crates/hdbscan) in a wasm-compatible
  boundary and a deterministic Node oracle/browser verification harness.
- Add browser support for existing `.skd`/`.skm` inputs and sketch generation.

---

# Session Log

## 2026-08-07

Completed the Rust `wtsne` v1 library: conditional probabilities, alias
sampling, reproducible xoshiro streams, Rayon workers, atomic embedding
updates, optional indicatif progress, validation, and public API documentation.
Input-index conversion is checked before `usize` conversion for wasm32 safety,
and underflowed probability rows use a uniform fallback. All six integration
tests, build, formatting, clippy, and diff checks pass.

Next session:
- Implement `SceResults`, migrate `wtsne()` to return it, and add schedule and
  accessor tests.

## 2026-08-07 (SceResults phase)

Completed the `SceResults` phase. Chosen interface: `wtsne()` returns a result
object; `FinalOnly` is the default; configured frame counts include initial and
final states; exponential spacing is geometric with exact monotonic clamping.
Added 11 public integration tests; documentation, formatting, Clippy, build,
test, and diff checks pass.

Next session:
- Distance-input phase (completed in the following session-log entry)

Blockers:
- None for the `SceResults` phase.

## 2026-08-07 (distance-input phase)

Completed:
- Started the phase by updating `plan.md` before code changes with the shared
  sparse interface, source compatibility rules, sketch threshold decision,
  CLI outputs, fixture organization, and verification checklist.
- Added `SparseDistances`, alignment and accessory constructors, sketchlib
  database/FASTA-list support, the embedding CLI, organized fixtures, and
  focused source/CLI integration tests.
- Preserved alignment self-edge/tie behavior and accessory no-self/exact-kNN/
  strict-threshold behavior; sketch threshold mode reports a clear error.
- Finished the phase by updating this plan with implementation decisions,
  verification evidence, blockers, and the next task.

Discovered:
- The existing `rust/src/gene.rs` contains the alignment bitmap/parser logic to
  reuse. The placeholder executable was replaced, and the supplied fixtures are
  organized under `rust/tests/fixtures`.

Next session:
- Refactor the crate into separate optimiser, distance, API, and CLI modules;
  then evaluate replacing the manual RNG with the planned xoshiro library.

Blockers:
- None for this phase.

## 2026-08-07 (module/RNG refactor phase)

Completed:
- Started the phase by updating `plan.md` before code changes with the module
  seams, public re-export contract, clean reseeding decision, checklist, and
  verification requirements.
- Moved the public SCE types into `src/api.rs`, optimiser internals into
  `src/sce.rs`, distance code into `src/distances/`, and the executable logic
  into binary-only `src/cli.rs` with a minimal `src/main.rs`.
- Replaced the hand-written xoshiro state machine with
  `rand_xoshiro::Xoshiro256PlusPlus`, preserving deterministic same-seed
  streams and adding changed-seed divergence plus public `api` module tests.
- Ran formatting, 28 tests, Clippy, build, documentation, and diff checks
  successfully using Cargo's offline cache.

Discovered:
- The current optimiser, public result/configuration types, and manual RNG are
  concentrated in `rust/src/lib.rs`; distance construction is already isolated
  enough to become a directory module, while the CLI remains in `main.rs`.

Next session:
- Resolve and implement the wasm-compatible async embedding API that replaces
  retained animation frames.

Blockers:
- Online crates.io resolution was unavailable in the sandbox, so verification
  used `--offline`; no dependency was missing from the local cache.

## 2026-08-11 (wasm/async API scope)

Completed:
- Replaced the completed module/RNG phase as the Current Task with the
  wasm32-compatible async embedding API transition.
- Removed the superseded follow-up from the plan.

Next session:
- Implement the confirmed cooperative operation and portable distance boundary.

Blockers:
- The existing wasm check reaches native compression dependencies; feature
  gating and reader-based distance constructors are required before target
  verification can pass.

## 2026-08-11 (wasm/async API design)

Completed:
- Confirmed the cooperative, synchronous `EmbeddingOperation` contract with
  caller-selected iteration budgets, zero-budget no-op polling, fixed maximum
  iteration completion, idempotent post-completion advances, drop cancellation,
  borrowed current embeddings, and consuming partial/final extraction.
- Confirmed the clean removal of retained frame result types and the blocking
  `wtsne` wrapper returning only `Array2<f64>`.
- Confirmed owned numerical input with optional weights, constant-time
  construction checks, debug-only per-element distance assertions, and no
  multi-worker equivalence tests.
- Confirmed the portable distance boundary, positive native/sketch Cargo
  features, native-only CLI, wasm sequential fallback, and fixed CLI chunk
  constant.
- Recorded the glossary and architectural decisions in `CONTEXT.md` and
  `docs/adr/0001-cooperative-embedding-operation.md` plus
  `docs/adr/0002-portable-distance-core.md`.

Verification:
- `rustup target list --installed` includes `wasm32-unknown-unknown`.
- `cargo check --target wasm32-unknown-unknown --offline` currently fails in
  native compression dependencies; implementation of the planned
  feature boundary is the next step.

Next session:
- Implement the confirmed API and distance-module split, then run focused
  native and wasm verification.

Blockers:
- No unresolved design blockers. The known wasm dependency failure is an
  implementation task addressed by the agreed feature split.

## 2026-08-11 (wasm/async API implementation)

Completed:
- Replaced `FrameSchedule`, `SceFrame`, and `SceResults` with owned
  `EmbeddingInput`, `EmbeddingOperation`, and `EmbeddingProgress`. The
  blocking `wtsne` wrapper now returns only its final `Array2<f64>`.
- Implemented synchronous, budgeted operation advancement with an initial
  pollable embedding, one retained current array, idempotent completion,
  partial-result warning logging, and no retained frame history.
- Split distance construction into portable reader-based alignment/accessory
  modules, native path wrappers, and a separately feature-gated sketch module.
  `native-inputs`, `sketchlib`, and `cli` are positive default features; the
  wasm core uses `--no-default-features`.
- Moved the native CLI to its own chunked progress loop using
  `CLI_ADVANCE_CHUNK = 1_000`; the library no longer owns progress rendering.
- Added operation lifecycle, one-worker budget-partition, partial extraction,
  and decompressed reader-constructor tests. No multi-worker equivalence test
  was added.

Verification:
- `cargo test --all-targets --offline`: 23 tests passed.
- Default and feature-free Clippy runs with `-D warnings`, native build/docs,
  feature-free `wasm32-unknown-unknown` check, formatting, and diff checks
  passed.

Next session:
- Start the worker-model cleanup described in the Current Task's Next Task.

Blockers:
- None.

## 2026-08-12 (worker-model cleanup design)

Completed:
- Confirmed one cross-platform `threads` setting, defaulting to one. It
  configures native distance construction, conditional probabilities,
  initialization, and SCE updates; wasm accepts it but remains sequential.
- Replaced the logical-worker model with parallel update rounds and retained
  atomic-CAS embedding updates rather than allocating per-thread delta buffers.
- Defined `max_updates` as a total-work target and `completed_updates` as the
  reported work. Native runs use complete thread batches and may finish up to
  `threads - 1` updates above the target; `advance` accepts round budgets.
- Confirmed deterministic single-thread behavior only, persistent jumped RNG
  streams, per-operation/per-distance-call native pools, public
  `DistanceOptions`, and default-on native phase bars controlled by `quiet`.
- Updated `CONTEXT.md` and recorded ADR 0003.

Verification:
- Design/documentation session only; no source or test changes were made.

Next session:
- Implement the confirmed worker-model cleanup from the Next Task.

Blockers:
- None.

## 2026-08-12 (worker-model cleanup implementation)

Completed:
- Replaced `workers`/iteration terminology with `threads`, `max_updates`,
  `completed_updates`, and round-budget `advance`; native rounds may overshoot
  the update target by at most `threads - 1`, while wasm remains sequential.
- Replaced logical worker execution with persistent Xoshiro256++ streams
  initialized through disjoint `jump()` calls. Atomic floating-point CAS
  updates and optimistic clash retries remain the shared embedding seam.
- Added private per-operation/per-distance Rayon pools, parallel conditional
  probabilities and initialization, and explicit `DistanceOptions` carrying
  sparsification, threads, and quiet behavior.
- Added default-on native phase bars for distances, probabilities, and
  optimisation, with CLI pass-through and no duplicate CLI renderer. Added
  feature-free no-op progress support for wasm.
- Updated public API, CLI flags, distance constructors, focused tests, and
  documented the sketchlib streaming coordinator-lane exception.

Verification:
- Native all-target tests: 24 passed.
- Feature-free library tests and `wasm32-unknown-unknown` library check passed.
- Native build, docs, formatting, both Clippy configurations, CLI help/smoke
  checks, focused accessory CLI integration, and `git diff --check` passed.
- Final pass after lazy progress-message formatting and accessory-phase bar
  finalization repeated the native and feature-free tests, both Clippy
  configurations, docs, formatting, wasm check, and diff validation
  successfully.

Next session:
- Begin the distance-code efficiency pass listed under Next Task.

Blockers:
- None.

## 2026-08-12 (distance-code efficiency design)

Completed:
- Confirmed uniform exact non-self kNN semantics, deterministic tie selection,
  directed unordered COO output, strict threshold filtering during generation,
  and independent row-owned calculation of both symmetric pair directions.
- Confirmed a private deep row-construction module using a pair-distance
  closure, positive-k `O(k)` priority queues, zero-k direct collection, and one
  output-sized final COO assembly pass.
- Confirmed standalone accessory profiles as one `RoaringBitmap` per sample,
  retained standalone `--accessory` input, and removed sketch
  `--use-accessory` scope.
- Confirmed removal of public/CLI seed settings, private `Option<u64>` seed
  injection for deterministic tests, and automatic native/browser-wasm system
  time seeding through `web_time`.
- Updated `CONTEXT.md`, recorded ADR 0004, reconciled superseded plan language,
  and defined focused correctness and portability verification. Runtime and
  peak-RSS benchmarking remain external to the repository task.

Verification:
- Documentation-only session; `git diff --check` passes. No source or test
  implementation was started.

Next session:
- Run the external runtime and peak-RSS benchmark comparison and review the
  directed COO outputs. No source implementation remains required for this
  phase.

Blockers:
- None. Runtime and peak-RSS benchmarking are intentionally external to this
  repository task.

## 2026-08-12 (distance-code efficiency implementation)

Completed:
- Added the private row-owned sparse-distance module. Positive-k rows use a
  bounded `BinaryHeap`, zero-k rows collect all non-self edges directly, and
  threshold rows filter strictly while distances are generated. Retained rows
  drain unsorted into one output-sized COO assembly pass.
- Migrated alignment and standalone accessory constructors behind the shared
  pair-distance closure seam. Accessory profiles now use `RoaringBitmap` and
  Jaccard intersection/union counts without dense per-gene vectors.
- Removed sketch `--use-accessory` and streamed sketch-accessory helpers;
  sketch constructors and CLI paths are core-only while standalone accessory
  input remains available.
- Removed public and CLI seed settings. Public operations resolve one
  `web_time::SystemTime` seed; private unit tests inject `Option<u64>` for
  reproducibility and budget-partition checks.
- Updated public distance/API documentation and focused edge-set, strict
  threshold, zero-k, directed kNN, and heap-bound tests.

Verification:
- `cargo test --all-targets --offline`: 29 tests passed.
- `cargo test --no-default-features --lib --offline`, native build, docs,
  formatting, both Clippy configurations, feature-free wasm check, CLI help,
  focused accessory CLI integration, and `git diff --check` passed.
- Runtime and peak-RSS benchmarking were not added, as agreed; they remain
  external follow-up work.

Next session:
- Run the external runtime and peak-RSS benchmark comparison and inspect the
  directed COO edge sets. No source implementation remains required for this
  phase.

Blockers:
- None.

## 2026-08-12 (code and style refactor)

Completed:
- Replaced the optional CLI `env_logger` dependency with `simple_logger` and
  added mutually exclusive `--verbose`/`-v` and `--quiet`/`--no-progress`
  behavior at `Info`, `Error`, and default `Warn` levels.
- Added concise library-owned loading, distance, probability, initialization,
  optimisation, output, and total elapsed-time logs without duplicate or
  per-record/per-update messages.
- Moved scalar CLI validation into `clap`, including the CLI-only inclusive
  perplexity range `[5.0, 100.0]`; preserved programmatic raw-similarity mode.
- Removed redundant distance-option validation from private helpers and the
  full-vector debug scans from `SparseDistances::new` while retaining cheap
  structural and parser-integrity checks.
- Updated README and CLI help-facing documentation. No tests were added, as
  agreed.

Verification:
- `cargo test --all-targets --offline`: 29 tests passed.
- `cargo test --no-default-features --lib --offline`, native build, docs,
  formatting, both Clippy configurations, feature-free wasm check, and
  `git diff --check` passed.
- Manual CLI checks confirmed help output, invalid perplexity rejection,
  mutually exclusive logging flags, verbose phase logs and total timing,
  quiet suppression, and rejection of repeated `-v`.

Deviations:
- No new tests or phase timing instrumentation were added, per the confirmed
  scope. Only one total CLI elapsed-time measurement was added.

Next session:
- Run the external runtime and peak-RSS benchmark comparison and inspect the
  directed COO edge sets; no further code is required for this refactor.

Blockers:
- None.

## 2026-08-19 (Python plotting CLI)

Completed:
- Added a required mutually exclusive `--labels LABELS.tsv` / `--hdbscan`
  CLI to `python/plot.py`, with prefix-based embedding and names loading.
- Added exact sample-name matching for unheadered two-column label TSV files,
  HDBSCAN cluster-table generation, and dispatch to the HTML, hex-density, and
  static Matplotlib plotting functions.
- Documented both label modes, input files, and generated outputs in README.

Verification:
- Static compilation with `python3` passed.
- `git diff --check` passed.
- No CLI invocation, `--help` check, or dedicated failure tests were added,
  per the confirmed scope.

Next session:
- If desired, run the CLI in the configured Python environment for runtime
  validation.

Blockers:
- None.

## 2026-08-20 (Mandrake web first pass)

Completed:
- Added a wasm-bindgen `MandrakeOperation`/`MandrakeProgress` boundary that
  reads alignment and accessory bytes, constructs the existing portable sparse
  distances, advances the cooperative embedding, and exposes final names and
  coordinates.
- Added a minimal Sparrowhawk-style Vue package with a 350 px parameter rail,
  alignment/accessory input selection, kNN/threshold controls, all requested
  optimisation parameters, a queued worker/driver, progress percentage, final
  SVG scatter plot, and embedding/name downloads.
- Documented the browser tool and recorded sketch, intermediate-frame,
  labelling, and HDBSCAN work as the next phase.

Verification:
- `cargo test --all-targets --offline`: 29 tests passed.
- `cargo clippy --all-targets --offline -- -D warnings`,
  `cargo fmt --all -- --check`,
  `cargo check --lib --no-default-features --target wasm32-unknown-unknown
  --offline`, and `git diff --check` passed.
- `wasm-pack build --target bundler --no-default-features --release` passed.
- `cd www && npm install` and `npm run build` passed; the generated wasm
  package and production bundle compile without warnings.
- `cd www && npm run serve -- --port 8080` served the landing page successfully
  under the approved local-server check; the server was then stopped.

Next session:
- Add sketch `.skd`/`.skm` support or sketch generation, then consider
  intermediate frames, label colouring, HDBSCAN, and a deterministic Node
  oracle/browser runtime harness.

Blockers:
- None. `npm install` reports transitive audit warnings in the Vue CLI tree;
  dependency upgrades are deferred from this first pass.

## 2026-08-20 (Mandrake web repair)

Completed:
- Replaced the worker's static wasm import with a cached dynamic import and
  made the webpack public path root-relative so async worker assets resolve
  correctly.
- Replaced the source selector and plain file input with an accessible
  click-or-drop zone, case-insensitive suffix detection, detected-type
  feedback, and explicit rejection of unsupported or ambiguous suffixes.
- Added keyboard-accessible hover/focus tooltips for the sparsification and
  optimisation controls using the descriptions in `src/cli.rs`.
- Updated the browser README to describe click/drop input and suffix detection.

Verification:
- `cd www && npm run build` passed; the production output contains the
  Mandrake worker, its split worker chunk, and a wasm binary. Static checks
  confirmed the worker uses `/` as its public path and fetches the emitted
  `.module.wasm` asset.
- `cd www && npm run serve -- --port 8080 --host 127.0.0.1` started
  successfully under the approved local-server check, and the landing page
  responded before the server was stopped.
- `git diff --check` passed.

Limitations:
- No headless browser is installed in this environment, so interactive
  drag/drop, tooltip focus, and a real wasm run were not exercised here; the
  emitted worker/module paths and build contract were checked statically.

Next session:
- Add intermediate-frame plotting and user-label colouring.
- Add browser support for existing `.skd`/`.skm` inputs or sketch generation.
- Add HDBSCAN labelling and the deterministic Node/browser oracle harness.

Blockers:
- None. Transitive npm audit warnings remain deferred.

## 2026-08-20 (Mandrake web interface phase)

Completed:
- Added a wasm-only cooperative distance-row builder and explicit
  `advanceDistances`/`beginEmbedding` transitions while preserving native
  parallel distance construction and sparse-row semantics.
- Extended the queued worker with separate distance/embedding progress and
  transferable latest-state frames at bounded update thresholds.
- Added exact-match named TSV labels, deterministic categorical colours, and a
  Plotly WebGL scatter with hover, zoom, pan, responsive sizing, and cleanup.
- Changed the library and web defaults to 1,000,000 updates with one-million
  numeric steps, updated declarations/docs, and added focused cooperative-row
  and default-value coverage.

Verification:
- `cargo fmt --all -- --check`, `cargo test --all-targets --offline`,
  `cargo clippy --all-targets --offline -- -D warnings`, and the feature-free
  wasm check passed.
- `cd www && npm run build` passed. Plotly adds the expected approximately
  4.7 MiB vendor bundle and npm reports existing transitive audit warnings.
- A direct Node-target wasm run using `tests/fixtures/gene_presence_absence.Rtab`
  completed with 1,837 samples, 3,674 coordinates, and 8 updates.
- Playwright Chromium smoke passed for labels, two Plotly traces, 10 px minimum
  plot text, and a changed zoom range; the same browser path completed the
  supplied 1,837-sample Rtab with no page or console errors.
- `git diff --check` passed.

Limitations:
- The Playwright smoke was run from an installed local CLI after downloading
  its Chromium binary; no permanent e2e harness was added. Deterministic
  coordinate comparison remains deferred with HDBSCAN.

Next session:
- Add `.skd`/`.skm` loading or browser sketch generation, then add HDBSCAN
  labelling and the deterministic Node/browser oracle harness.

Blockers:
- None. Plotly bundle-size and npm audit warnings are recorded for later
  dependency/performance work.

## 2026-08-21 (browser gzip input phase)

Completed:
- Added wasm-only `web_sys::File` constructors for alignment and accessory
  inputs, sharing the existing byte-constructor operation path.
- Added Sparrowhawk-compatible fixed-size file reads, gzip magic detection, and
  `flate2::MultiGzDecoder` streaming into the existing parsers. The worker now
  receives the selected `File` directly, so the page does not materialise an
  input byte buffer before parsing.
- Extended browser suffix detection, picker metadata, README documentation,
  and the generated TypeScript boundary for `.gz` inputs.
- Recorded the wasm-only `Send` invariant required by needletail and the
  GPL-3.0 licensing follow-up for `wasm-bindgen-file-reader`.

Verification:
- `cargo test --all-targets --offline`: 30 tests passed.
- Native and wasm Clippy with `-D warnings`, formatting, feature-free
  `wasm32-unknown-unknown` compilation, `git diff --check`, and the production
  Vue build passed. The build retains existing wasm/vendor bundle-size and npm
  audit warnings.
- Elevated Playwright Chromium smoke completed the replacement
  `tests/fixtures/gene_presence_absence.Rtab.gz` and
  `tests/fixtures/sub5k_hiv_refs_prrt_trim.fas.gz` uploads, and the existing
  raw-input smoke still passed with two Plotly traces, 10 px minimum text, and
  changed zoom range.

Next session:
- Add browser support for existing `.skd`/`.skm` inputs or sketch generation,
  then add HDBSCAN labelling and the deterministic Node/browser oracle harness.

Blockers:
- No implementation blockers. Resolve the GPL-3.0 dependency's project
  licensing/distribution treatment before release.

## 2026-08-21 (browser visual/input refinement)

Completed:
- Added a generic `.gz` token to the browser input picker while preserving the
  existing case-insensitive FASTA/FASTQ and Rtab/TSV suffix validation.
- Capped the worker's live-frame interval at 20,000 updates while retaining the
  existing approximately 20-frame cadence for smaller runs.
- Added the supplied `www/src/assets/mandrake_logo.png` and replaced the
  sidebar, page-heading, and empty-state decorative `M` marks with it.

Verification:
- `cd www && npm run build` passed. Existing wasm/vendor bundle-size and npm
  audit warnings remain unchanged.
- Existing Chromium UI and gzip-fixture smokes passed. A focused elevated
  Chromium smoke loaded all three logo images, confirmed the generic `.gz`
  token, found no desktop or narrow-layout overflow, and observed 47 live
  Plotly updates during a 1,000,000-update run.
- `git diff --check` passed.

Limitations:
- Firefox picker selection was not automated because the Playwright Firefox
  binary is not installed; manually select both `.fas.gz` and `.Rtab.gz` in
  Firefox to complete that browser-specific check.
- No permanent browser test harness was added in this focused pass.

Next session:
- Add browser support for existing `.skd`/`.skm` inputs or sketch generation.

Blockers:
- None. The gzip reader's GPL-3.0 licensing/distribution decision remains a
  release follow-up.

## 2026-08-21 (browser HDBSCAN final-embedding phase)

Completed:
- Pinned `hdbscan = 0.12.0` with its serial-only feature and added a pure Rust
  helper that validates, centres, half-range-scales, and clusters final 2D
  embeddings with the fixed browser preset. Noise is returned as `-1`.
- Added the feature-free wasm `clusterEmbedding` export and queued-worker
  labelling step. Clustering runs only after optimisation, reports a distinct
  progress state, transfers signed labels with the final embedding, and keeps
  the embedding visible when labelling returns an error.
- Added the HDBSCAN option, manual-versus-cluster colour switch, non-noise
  cluster count (including an explicit zero-cluster message), cluster/noise
  Plotly styling, warning state, and CSV download with the exact Python header
  and CSV escaping. Added `scripts/hdbscan_oracle.mjs` for the deterministic
  Node-target wasm fixture.
- Updated the browser README with the option, output, and oracle invocation.

Verification:
- `cargo fmt --all -- --check`, `cargo test --all-targets --offline` (32
  tests), native and feature-free wasm Clippy with `-D warnings`, and the
  feature-free wasm check passed.
- `cd www && npm run build` passed. Existing wasm/vendor bundle-size and npm
  audit warnings remain unchanged.
- `wasm-pack build --target nodejs --no-default-features --out-dir
  /private/tmp/mandrake-wasm-node --dev` followed by
  `node scripts/hdbscan_oracle.mjs /private/tmp/mandrake-wasm-node` passed with
  the native 11-point fixture labels `[0,0,0,0,0,1,1,1,1,1,-1]`.
- A real Node wasm run over `tests/fixtures/gene_presence_absence.Rtab.gz`
  produced 1,837 labels for 3,674 coordinates and completed HDBSCAN labelling.
- Elevated Playwright Chromium smoke passed the HDBSCAN option, final result
  count (including the all-noise message), manual/HDBSCAN colour switch,
  cluster/noise traces, and `hdbscan.embedding_hdbscan_clusters.csv` download
  with six rows and the expected header; no page or console errors occurred.
- `git diff --check` passed.

Limitations and deviations:
- The Rust crate is pinned for reproducible in-browser output, but cluster
  numeric IDs are not promised to match Python's separate implementation.
- Chromium verification used the existing installed CLI and a temporary smoke
  script; the permanent deterministic oracle is Node-based. Firefox picker
  coverage remains manual, as recorded in the preceding phase.

Next session:
- Add browser support for existing `.skd`/`.skm` inputs or sketch generation.

Blockers:
- No implementation blockers. Resolve the `wasm-bindgen-file-reader` GPL-3.0
  licensing/distribution decision before release.

## 2026-08-21 (reproducible Playwright browser harness)

Completed:
- Added pinned local `@playwright/test` 1.62.1, `test:e2e`, and
  `playwright:install` npm
  scripts, plus a checked-in `www/playwright.config.ts` that starts the Vue
  server on localhost and selects Chromium with bounded timeouts and failure
  artifacts.
- Promoted browser coverage to `www/e2e/mandrake.spec.ts`. The suite resolves
  `tests/fixtures/gene_presence_absence.Rtab.gz` from the repository path,
  waits for a final Plotly embedding, checks page/console errors, and exercises
  the HDBSCAN colour switch and cluster CSV download with a small in-memory
  accessory fixture. No `.fas.gz` test was added.
- Documented the install and `npm run test:e2e` commands in README.
- Recorded the reproducible runner and fixture decisions in Design Notes and
  kept the next task at browser `.skd`/`.skm` support or sketch generation.

Verification:
- `npm install` updated the local lockfile with Playwright 1.62.1.
- `npm run playwright:install` downloaded the Chromium and headless-shell
  binaries to Playwright's external cache.
- `npm run test:e2e -- --list` listed both committed Chromium tests.
- `npx tsc --noEmit --target es2022 --module commonjs --moduleResolution node
  --esModuleInterop --skipLibCheck playwright.config.ts e2e/mandrake.spec.ts`
  passed.
- `npm run test:e2e` passed both tests in 34.2 seconds: the tracked gzip
  accessory fixture completed in 24.3 seconds and the HDBSCAN/CSV smoke in
  2.3 seconds.

Limitations and deviations:
- The slow compressed alignment fixture remains intentionally excluded, as
  requested. Playwright browser binaries are not committed and must be
  installed separately.
- Existing npm audit warnings and wasm/vendor bundle-size warnings remain
  unchanged.

Next session:
- Add browser support for existing `.skd`/`.skm` inputs or sketch generation.

Blockers:
- No implementation blockers. Resolve the `wasm-bindgen-file-reader` GPL-3.0
  licensing/distribution decision before release.
