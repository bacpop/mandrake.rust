# Use a cooperative embedding operation

Mandrake will replace retained animation frames with a caller-owned embedding
operation that advances by a caller-selected iteration budget and can be polled
between advances. This keeps the public API usable on
`wasm32-unknown-unknown` without requiring threads, an async executor, or a
runtime-specific scheduling model. Each operation retains only its current
embedding, updated after every completed iteration; progress is returned
separately so polling does not copy the embedding or accumulate history.
The initialized iteration-0 embedding is available immediately on creation.
Advancing an operation after completion is idempotent and preserves that final
embedding.
Dropping the caller-owned operation is cancellation, because no work proceeds
between calls to advance.
Construction performs all validation and setup; a successfully created
operation advances without recoverable errors.
Construction takes ownership of the sparse input and weights needed for later
advances, avoiding an implicit copy of potentially large inputs.
`wtsne` remains as a blocking convenience wrapper that advances an operation
to completion and returns only the final embedding.
Advance status reports completed and maximum iterations plus the `Eq`
convergence statistic; it does not expose worker-update counts.
A zero iteration budget is a valid no-op poll, and `Eq` is diagnostic rather
than a convergence-based termination condition.
The configured maximum iteration count is the only terminal limit; an
oversized advance performs just the remaining iterations before completion.
On `wasm32-unknown-unknown`, the operation uses Rayon's sequential global
fallback instead of an explicit thread pool; native configured pools remain
until the separate worker-model cleanup.
The library does not render progress: the terminal-specific progress option is
removed, while the native CLI retains its progress display by rendering
reported operation progress itself.
This is a clean API break: `FrameSchedule`, `SceFrame`, and `SceResults` are
removed; the blocking `wtsne` wrapper consumes `EmbeddingInput` and returns the
final `Array2`.
The current embedding is borrowed for polling and transferred only through a
separate consuming final-embedding accessor.
Each advance returns a single `EmbeddingProgress` value; its completion state
is queried directly instead of matching a separate status enum.
An operation may be consumed before completion to transfer its partial current
embedding, with a warning log to make early extraction visible.
The Rust operation remains synchronous; callers provide any async or UI
scheduling around `advance`, without an executor or `Future` in this phase.
