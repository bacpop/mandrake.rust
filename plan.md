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

Replace retained `SceResults` animation frames with a wasm32-compatible async
embedding interface that callers can poll for the current embedding state.

Before code changes, this task updates this plan. Before finishing, it records
completed milestones, design decisions, verification, blockers, and the next
task here.

### Checklist

- [x] Update the plan with the wasm/async scope and remove superseded follow-up work
- [x] Resolve the public async API, polling contract, and lifecycle semantics
- [x] Define wasm32-compatible execution and scheduling constraints
- [ ] Remove retained animation-frame types and implementation
- [ ] Implement the async, pollable embedding API
- [ ] Add focused lifecycle, polling, and final-result coverage
- [ ] Verify native and `wasm32-unknown-unknown` builds, focused tests, formatting, lint, documentation, and diff checks
- [ ] Update this plan at completion with status, decisions, blockers, log, and next task

### Current Status

Working on:

The wasm/async API design is complete and documented. No implementation changes
were made during this grilling phase; the next session begins implementation.

Last verified:

2026-08-07: `cargo test --all-targets --offline` (28 tests),
`cargo build --all-targets --offline`, `cargo doc --no-deps --offline`,
`cargo fmt --all`, `cargo clippy --all-targets --offline -- -D warnings`, and
`git diff --check` pass. The online Cargo attempt was blocked by unavailable
crates.io DNS; all required dependencies were available in the local cache.

Current blocker:

2026-08-11: `cargo check --target wasm32-unknown-unknown --offline` reaches
native compression dependencies and fails while compiling `lzma-sys` and
`bzip2-sys`, whose C sources require `stdlib.h`. The target is installed. The
wasm-facing crate boundary has been designed but is not yet implemented; the
planned feature split addresses this blocker.

---

# Design Notes

## Parallelism

- Prefer Rayon.
- Atomic floating-point updates are required for embedding writes; use a CAS
  loop over `AtomicU64` bit patterns and retain the C++ optimistic clash retry.

## RNG

- Use `rand_xoshiro::Xoshiro256PlusPlus` through its `RngCore` and
  `SeedableRng` interfaces.
- Derive independent domain-separated `seed_from_u64` streams from the
  configured seed for each logical worker and each initial embedding point.
- Preserve same-seed reproducibility, but intentionally do not preserve the
  previous manual-generator embedding values; this is a clean seeded-output
  compatibility break chosen for the phase.

## Progress

The library returns `EmbeddingProgress` from each synchronous advance. The
native CLI renders that status with indicatif; the portable core owns no
terminal or UI renderer.

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
- `WtsneOptions` carries perplexity, iteration count, repulsion samples,
  learning rate, initial exaggeration, worker count, and seed. Retained frame
  schedules and library-owned progress rendering are removed.
- Tests target public behavior: validation, normalization, finite output,
  reproducibility for one worker, and successful parallel execution.
- No Python bindings or CUDA are part of this milestone.
- Distance constructors return `SparseDistances` and accept
  `Sparsification::Knn` or `Sparsification::Threshold` where the source
  supports that mode.

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
- The clean RNG migration derives worker seeds from
  `seed + WORKER_STREAM_DOMAIN + index * STREAM_MULTIPLIER` and initial-point
  seeds from the analogous `INITIAL_STREAM_DOMAIN` stream. Unit sampling uses
  the full-width `next_u64` output over `[0, 1)`. This retains deterministic
  same-seed runs while intentionally changing fixed-seed embedding values from
  the former hand-written xoshiro128+ implementation.

## Wasm/async API transition

- Replace the retained-frame result model with a caller-owned, cooperatively
  stepped embedding operation. The caller advances bounded work and may poll
  the current state between steps; this avoids requiring threads, an executor,
  or a particular async runtime on `wasm32-unknown-unknown`.
- Each advance accepts a caller-selected iteration budget, so UI callers can
  choose short work units while native callers can choose larger batches.
- `advance` returns lightweight progress/completion metadata. The operation
  retains one current `Array2` embedding, replacing it after every completed
  iteration; callers retrieve that state through a separate accessor. It never
  retains a historical frame series.
- The initialized iteration-0 embedding is available through that accessor as
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
- Each advance reports completed iterations, configured maximum iterations, and
  the `Eq` convergence statistic. Worker-update counts are removed from the
  public API because workers are not part of the intended long-term model.
- A zero iteration budget is a valid no-op poll. `Eq` remains diagnostic and
  must not cause convergence-based early termination.
- The configured maximum iteration count is the sole terminal limit. An
  oversized budget performs the remaining iterations and then returns
  completion.
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
- On `wasm32-unknown-unknown`, use Rayon's sequential global fallback rather
  than an explicit `ThreadPoolBuilder`, which Rayon documents as unsupported
  on that target. Native builds retain configured pools until the separate
  worker-model cleanup.
- Remove the terminal-specific `WtsneOptions::progress` setting. The library
  reports operation progress only; the native CLI retains its progress display
  by rendering that reported status itself.
- Make a clean public API break: remove `FrameSchedule`, `SceFrame`, and
  `SceResults`. The retained blocking `wtsne` wrapper consumes `EmbeddingInput`
  and returns `Result<Array2<f64>>`.
- For this phase, the CLI advances a fixed internal iteration chunk and renders
  progress after each chunk. Interval-driven rendering from a separate thread
  is deferred as a non-priority follow-up.
- Define that chunk as `const CLI_ADVANCE_CHUNK: usize = 1_000` in the CLI.
- Advancing with different budget partitions should ideally preserve fixed-seed
  results, but this is secondary to performance and must not justify added
  copying, synchronization, or validation overhead.
- Regression coverage checks budget-partition invariance only in the
  deterministic single-worker path. Multi-worker equivalence is not tested and
  is deferred with the worker-model removal.
- Retain `WtsneOptions::workers` temporarily for native implementation behavior;
  remove it later with the worker-model cleanup. WASM uses sequential
  execution, and workers do not appear in public progress/status values.
- Keep the Rust operation synchronous (`advance(&mut self, budget)`); callers
  provide any async/UI scheduling around it. No executor or `Future` is part of
  this phase.
- `EmbeddingOperation::embedding()` borrows the retained current `Array2`; a
  separate `into_embedding()` consumes the operation and transfers the final
  array. Polling does not clone the embedding.
- `advance` returns one `EmbeddingProgress` struct carrying completed and
  maximum iteration counts plus `Eq`; completion is queried through
  `is_complete()` rather than a separate status enum.
- `into_embedding()` may consume an incomplete operation and return its latest
  state without copying, but emits a `WARN` log unless the operation has
  completed.

---

# Open Questions

- SIMD opportunities? Ideally compiler optimised rather than 'hand-optimised'.

---

# Next Task

Implement the confirmed wasm/async API: introduce owned `EmbeddingInput` and
cooperative `EmbeddingOperation`, remove retained frame results, split and
feature-gate distance inputs, preserve native CLI progress rendering, and add
focused lifecycle tests.

# Further tasks

Tasks for later implementation steps:

- Parallelism tasks:
  - A single --threads argument which works everywhere. Default to one
    thread unless otherwise requested.
  - Parallelise distances and conditional probabilities in line with
    this guidance
  - Add indicatif progress bars to these two parallel iterators (but not
    the main sce algorithm, which already works).
  - Remove the worker concept in the main SCE algorithm, and instead
    just allow parallel updates in the main loop. If this simplifies
    atomic updates let me know in the plan.

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
