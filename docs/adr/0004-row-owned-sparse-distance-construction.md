# Use row-owned bounded sparse-distance construction

Mandrake will construct sparse distances through a private row-oriented module
that accepts a source-specific pair-distance function. Each positive-k kNN row
owns an `O(k)` priority queue and retains exactly `k` non-self neighbours, while
threshold rows discard rejected edges as distances are generated and zero-k
rows directly retain every non-self edge; retained COO order is unspecified
and kNN output is directed.

Independent rows calculate both directions of each symmetric distance rather
than sharing candidate state. This deliberately trades duplicate computation
and legacy alignment self-edge/tie-expansion behaviour for bounded auxiliary
memory, lock-free Rayon parallelism, one implementation of sparsification, and
an unavoidable single final pass that assembles the flat COO vectors.
