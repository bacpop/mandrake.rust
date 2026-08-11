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
    completed_iterations: usize,
    max_iterations: usize,
    eq: f64,
}

impl EmbeddingProgress {
    pub(crate) fn new(completed_iterations: usize, max_iterations: usize, eq: f64) -> Self {
        Self {
            completed_iterations,
            max_iterations,
            eq,
        }
    }

    /// Number of completed optimisation iterations.
    pub fn completed_iterations(&self) -> usize {
        self.completed_iterations
    }

    /// Configured maximum number of optimisation iterations.
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Current SCE convergence statistic.
    pub fn eq(&self) -> f64 {
        self.eq
    }

    /// Whether the operation has reached its configured iteration limit.
    pub fn is_complete(&self) -> bool {
        self.completed_iterations == self.max_iterations
    }
}

/// A caller-owned, cooperatively stepped embedding calculation.
///
/// Use [`Self::advance`] to perform a caller-selected amount of work and
/// [`Self::embedding`] to borrow the latest completed embedding state.
pub struct EmbeddingOperation {
    pub(crate) inner: crate::sce::EmbeddingOperationInner,
}

impl EmbeddingOperation {
    /// Construct and initialise an embedding operation.
    pub fn new(input: EmbeddingInput, options: &WtsneOptions) -> Result<Self> {
        crate::sce::EmbeddingOperationInner::new(input, options).map(|inner| Self { inner })
    }

    /// Advance by up to `iteration_budget` iterations.
    ///
    /// A zero budget is a no-op poll. An oversized budget performs the
    /// remaining work only. Advancing after completion is idempotent.
    pub fn advance(&mut self, iteration_budget: usize) -> EmbeddingProgress {
        self.inner.advance(iteration_budget)
    }

    /// Borrow the current embedding, including the initial iteration-0 state.
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
    /// Number of optimisation iterations.
    pub max_iterations: usize,
    /// Number of randomly sampled repulsion pairs per worker and iteration.
    pub repulsion_samples: usize,
    /// Initial learning rate.
    pub learning_rate: f64,
    /// Use four-times stronger attraction during the first tenth of the run.
    pub initial_exaggeration: bool,
    /// Number of Rayon workers used on native targets.
    pub workers: usize,
    /// Seed for all random-number streams.
    pub seed: u64,
}

impl Default for WtsneOptions {
    fn default() -> Self {
        Self {
            perplexity: 15.0,
            max_iterations: 100_000,
            repulsion_samples: 5,
            learning_rate: 1.0,
            initial_exaggeration: false,
            workers: 1,
            seed: 1,
        }
    }
}
