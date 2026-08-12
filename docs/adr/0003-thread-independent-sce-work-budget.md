# Thread-independent SCE work budget

`max_updates` is the target for stochastic SCE update attempts, and progress
reports completed attempts. Native parallel rounds perform exactly the
configured `threads` attempts while wasm remains sequential; a run may finish
up to `threads - 1` attempts above the target rather than forcing a final
partial round. This keeps requested optimiser work comparable across thread
counts without imposing cross-thread reproducibility or allocating per-thread
embedding buffers. A small set of persistent Xoshiro executor streams is
initialized once through disjoint `jump()` calls, rather than reseeding each
update or replaying jumps to catch up. `EmbeddingOperation::advance` receives a
parallel-round budget, so every non-zero round can execute a full thread batch;
only zero is a no-op poll. The public API calls the total-work fields
`max_updates` and `completed_updates`, making their difference from round
budgets explicit. Each native operation owns its configured Rayon pool, which
also runs construction-time parallel preprocessing; no process-global pool is
configured so repeated operations remain valid. Public distance constructors
receive the same explicit setting and use a per-call configured pool.
