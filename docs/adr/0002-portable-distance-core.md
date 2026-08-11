# Keep the distance core portable

Mandrake will split distance construction into portable reader-based alignment
and accessory constructors, plus native-only path loaders for file opening and
compression. Sketchlib-backed constructors will be separately feature-gated:
the wasm application supplies sketch functionality outside Mandrake, while the
core retains sparse-distance construction and embedding.

The positive `native-inputs` and `sketchlib` features remain enabled by default
for native compatibility. Portable wasm builds use `--no-default-features`,
because Cargo features are additive and a `wasm` feature cannot disable default
native dependencies.

The path-oriented `mandrake` CLI requires both features and is native-only;
the feature-free wasm build exposes only the portable library core.

Portable reader-based constructors accept already-decompressed bytes.
Compression detection and decoding remain the native path-loader's
responsibility or are supplied by the caller.

`SparseDistances` remains a labeled distance-input value, while the optimiser
consumes a separate owned `EmbeddingInput` containing only COO vectors and
weights. This keeps sample labels outside the long-lived embedding operation.
`EmbeddingInput` construction accepts optional owned weights; absent weights
select a newly created uniform-weight vector.
It takes owned COO vectors and an explicit node count directly, so it does not
consume labels or copy supplied numerical input.
