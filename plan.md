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

Port the C++ package `mandrake` to Rust.

Source:
- `src/`
- primarily `src/wtsne_cpu.cpp`

Target:
- `rust/`
- standalone Cargo crate

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

Implement the confirmed worker-model cleanup: replace explicit logical workers
in the main SCE optimiser with parallel update rounds, expose one cross-platform
thread setting, retain atomic updates, add persistent jumped RNG streams,
distance options, and native phase progress bars.

### Checklist

- [x] Record the confirmed worker-model design in the plan and domain docs
- [x] Replace worker configuration/API with `threads`, `max_updates`, and `quiet`
- [x] Replace worker execution with persistent RNG-backed parallel rounds
- [x] Scope Rayon pools for SCE, probabilities, and distance constructors
- [x] Add native phase progress bars and shared `DistanceOptions`
- [x] Update focused API, distance, CLI, and wasm coverage
- [x] Verify native/wasm builds, tests, formatting, lint, docs, and diff checks
- [x] Update this plan with implementation status, decisions, blockers, log, and next task

### Current Status

Working on:

The worker-model cleanup is complete. The public API, native parallel rounds,
distance options, progress phases, and portable wasm build all reflect the
confirmed design.

Last verified:

2026-08-12: `cargo test --all-targets --offline` (24 tests),
`cargo test --no-default-features --lib --offline`,
`cargo build --all-targets --offline`, `cargo doc --no-deps --offline`,
`cargo fmt --all -- --check`, `cargo clippy --all-targets --offline -- -D warnings`,
`cargo clippy --lib --no-default-features --offline -- -D warnings`,
`cargo check --lib --no-default-features --target wasm32-unknown-unknown
--offline`, `cargo run --offline -- --help`, the focused accessory CLI
integration test, and `git diff --check` pass.

Blockers:

None.

---

# Design Notes

## Parallelism

- Prefer Rayon.
- Atomic floating-point updates are required for embedding writes; use a CAS
  loop over `AtomicU64` bit patterns and retain the C++ optimistic clash retry.
- Replace the public `workers` setting with `threads`. `WtsneOptions::threads`
  and CLI `--threads` default to one and govern every native parallel section:
  distance construction, conditional probabilities, embedding initialization,
  and SCE updates. The setting is accepted on wasm for API compatibility, but
  wasm execution remains sequential and values above one have no effect.
- A fixed seed guarantees reproducible embeddings only with `threads = 1` and
  otherwise identical input and options. Multi-threaded SCE updates may vary
  with scheduling, so their verification targets finite output and intended
  behaviour rather than equality across thread counts.
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
- Initialize a root, domain-separated update RNG once from the configured
  seed. Create the small fixed set of executor streams by successive
  `Xoshiro256PlusPlus::jump()` calls, then retain and advance those streams
  normally. Never reseed per update or repeatedly jump from the initial state
  to catch up with progress.
- Retain domain-separated `seed_from_u64` streams for initial embedding points.
- Preserve same-seed reproducibility, but intentionally do not preserve the
  previous manual-generator embedding values; this is a clean seeded-output
  compatibility break chosen for the phase.

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
  learning rate, initial exaggeration, thread count, quiet setting, and seed.
  Retained frame schedules are removed; native progress bars are controlled by
  the quiet setting.
- Tests target public behavior: validation, normalization, finite output,
  reproducibility for one thread, and successful parallel execution.
- No Python bindings or CUDA are part of this milestone.
- Distance constructors return `SparseDistances` and accept `DistanceOptions`,
  whose sparsification field supports `Sparsification::Knn` or
  `Sparsification::Threshold` where the source supports that mode.

## Distance-input phase decisions

- All three sources return a public `SparseDistances` value containing sample
  names and zero-based COO row, column, and normalized-distance vectors.
- Use released crates.io `sketchlib = "0.4.1"` for file-compatible `.skm`/`.skd`
  loading and sparse kNN distances.
- Pair-SNP alignment input retains legacy pairsnp self edges and tie behavior.
  Accessory input retains legacy sklearn behavior (no self edges, exact kNN,
  strict threshold filtering).
- Sketch inputs support kNN only in this phase. Threshold mode is rejected for
  sketches because the released API does not provide a streaming threshold
  operation and dense all-pairs materialization would defeat sparse input.
- The CLI accepts one source at a time, runs `wtsne`, and writes
  `<output>.embedding.txt` plus `<output>.names.txt`; plotting and clustering
  remain outside this phase.
- Use the existing `rust/src/gene.rs` parsing semantics (IUPAC overlap,
  unknown-base matching, and its tested gap handling) rather than duplicating
  the parser.
- For multi-kmer sketch accessory distances, consume sketchlib's streamed
  all-pairs output into bounded per-row top-k candidates, avoiding a dense
  distance matrix while retaining accessory-specific neighbour selection.
- Single-kmer sketch databases use sketchlib's Jaccard sparse path; accessory
  selection is rejected unless at least two k-mer lengths are available.

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
  uses the full-width `next_u64` output over `[0, 1)`. This retains
  deterministic single-thread runs while intentionally changing fixed-seed
  embedding values from the former hand-written xoshiro128+ implementation.

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
- Advancing with different round-budget partitions should ideally preserve
  fixed-seed single-thread results, but this is secondary to performance and
  must not justify added copying, synchronization, or validation overhead.
- Regression coverage checks budget-partition invariance only in the
  deterministic single-thread path. Multi-thread tests assert a completed,
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

---

# Open Questions

- SIMD opportunities? Ideally compiler optimised rather than 'hand-optimised'.

---

# Next Task

Start the planned distance-code efficiency pass: keep row-oriented parallel
construction, replace full per-row sorting with bounded top-k selection, retain
threshold filtering during generation, and assess the shared alignment/CSV
distance seam.

# Further tasks

Tasks for later implementation steps:

- Improvement for dists code (from pairsnp or csv):
  - Refactor: combine dist types (with a trait?), csv just has one bitvec, to keep code paths more similar.
  - Refactor: remove the --accessory option, and all associated helper
    code.
  - Efficiency: parallel iteration should always be over rows to avoid need for a costly flatten, then at the end append all the vecs together
  - Efficiency: the sparsification is inefficient: for knn use a priority queue in each row as distances are produced, for threshold only keep the item if distance is less than threshold when first calculated. When parallelised over rows as the above point, this change will become easier.


- Code and style refactoring. Taking note of house style above, which
  has been ignored.
  - Add a verbose option and logging messages for every step (loading
    files, distances, probabilities, calculating embedding)
  - Remove costly checks, especially on long distance vectors. Checking
    of user input paramaters should be done in CLI as part of clap

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
  native `lzma-sys`/`bzip2-sys` dependencies; implementation of the planned
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
