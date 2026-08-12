//! Public stochastic cluster embedding configuration and operation types.

use anyhow::{Result, bail};
use ndarray::Array2;

/// Owned numeric input for one embedding operation.
///
/// Sample labels intentionally remain outside this type. `rows`, `columns`,
/// and `distances` are zero-based COO vectors and are moved without copying.
#[derive(Clone, Debug)]
pub struct EmbeddingInput {
    pub(crate) rows: Vec<u64>,
    pub(crate) columns: Vec<u64>,
    pub(crate) distances: Vec<f64>,
    pub(crate) n_nodes: usize,
    pub(crate) weights: Vec<f64>,
}

impl EmbeddingInput {
    /// Create an owned embedding input.
    ///
    /// When `weights` is `None`, one uniform weight is created per node. This
    /// constructor performs only constant-time structural checks; release
    /// embedding assumes supplied numerical data is trusted.
    pub fn new(
        rows: Vec<u64>,
        columns: Vec<u64>,
        distances: Vec<f64>,
        n_nodes: usize,
        weights: Option<Vec<f64>>,
    ) -> Result<Self> {
        if rows.len() != columns.len() || rows.len() != distances.len() {
            bail!("COO rows, columns, and distances must have the same length");
        }
        if n_nodes < 2 {
            bail!("at least two nodes are required");
        }
        let weights = weights.unwrap_or_else(|| vec![1.0; n_nodes]);
        if weights.len() != n_nodes {
            bail!("node weights must have the declared node count");
        }
        Ok(Self {
            rows,
            columns,
            distances,
            n_nodes,
            weights,
        })
    }

    /// Number of nodes represented by this input.
    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }
}

/// Progress reported after advancing an embedding operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbeddingProgress {
    completed_updates: usize,
    max_updates: usize,
    eq: f64,
}

impl EmbeddingProgress {
    pub(crate) fn new(completed_updates: usize, max_updates: usize, eq: f64) -> Self {
        Self {
            completed_updates,
            max_updates,
            eq,
        }
    }

    /// Number of completed stochastic update attempts.
    pub fn completed_updates(&self) -> usize {
        self.completed_updates
    }

    /// Configured target number of stochastic update attempts.
    pub fn max_updates(&self) -> usize {
        self.max_updates
    }

    /// Current SCE convergence statistic.
    pub fn eq(&self) -> f64 {
        self.eq
    }

    /// Whether the operation has reached or exceeded its configured update target.
    pub fn is_complete(&self) -> bool {
        self.completed_updates >= self.max_updates
    }
}

/// A caller-owned, cooperatively stepped embedding calculation.
///
/// Use [`Self::advance`] to perform a caller-selected number of rounds and
/// [`Self::embedding`] to borrow the latest completed embedding state. Public
/// operations seed their private random streams from system time.
pub struct EmbeddingOperation {
    pub(crate) inner: crate::sce::EmbeddingOperationInner,
}

impl EmbeddingOperation {
    /// Construct and initialise an embedding operation.
    pub fn new(input: EmbeddingInput, options: &WtsneOptions) -> Result<Self> {
        crate::sce::EmbeddingOperationInner::new(input, options).map(|inner| Self { inner })
    }

    /// Advance by up to `round_budget` complete parallel update rounds.
    ///
    /// A zero budget is a no-op poll. Each non-zero round performs one update
    /// per configured native thread; the final round may carry the operation
    /// up to `threads - 1` updates beyond its target. Advancing after
    /// completion is idempotent.
    pub fn advance(&mut self, round_budget: usize) -> EmbeddingProgress {
        self.inner.advance(round_budget)
    }

    /// Borrow the current embedding, including the initial zero-update state.
    pub fn embedding(&self) -> &Array2<f64> {
        self.inner.embedding()
    }

    /// Consume this operation and return its latest embedding without copying.
    ///
    /// A warning is logged when the operation has not completed.
    pub fn into_embedding(self) -> Array2<f64> {
        self.inner.into_embedding()
    }
}

/// Configuration for [`crate::wtsne`] and [`EmbeddingOperation`].
#[derive(Clone, Debug)]
pub struct WtsneOptions {
    /// Target entropy for conditional probability preprocessing. Values at or
    /// below zero use the input values as raw similarities (`1 - distance`).
    pub perplexity: f64,
    /// Target number of stochastic update attempts.
    pub max_updates: usize,
    /// Number of randomly sampled repulsion pairs per update attempt.
    pub repulsion_samples: usize,
    /// Initial learning rate.
    pub learning_rate: f64,
    /// Use four-times stronger attraction during the first tenth of the run.
    pub initial_exaggeration: bool,
    /// Number of native Rayon threads used for each parallel update round.
    /// Wasm accepts this value but executes sequentially.
    pub threads: usize,
    /// Suppress native phase progress bars.
    pub quiet: bool,
}

impl Default for WtsneOptions {
    fn default() -> Self {
        Self {
            perplexity: 15.0,
            max_updates: 100_000,
            repulsion_samples: 5,
            learning_rate: 1.0,
            initial_exaggeration: false,
            threads: 1,
            quiet: false,
        }
    }
}
