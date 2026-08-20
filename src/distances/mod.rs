//! Sparse distance construction for supported Mandrake input formats.

use crate::progress::PhaseProgress;
use anyhow::{Result, bail};

mod accessory;
mod alignment;
#[allow(dead_code)]
mod bitvecs;
mod rows;
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
mod sketch;

#[cfg(feature = "native-inputs")]
pub use accessory::accessory_distances;
pub use accessory::accessory_distances_from_reader;
#[cfg(target_family = "wasm")]
pub(crate) use accessory::{jaccard_distance, read_accessory_table};
#[cfg(feature = "native-inputs")]
pub use alignment::pair_snp_distances;
pub use alignment::pair_snp_distances_from_reader;
#[cfg(target_family = "wasm")]
pub(crate) use alignment::read_alignment;
#[cfg(target_family = "wasm")]
pub(crate) use bitvecs::SampleBases;
#[cfg(target_family = "wasm")]
pub(crate) use rows::DistanceRowBuilder;
pub(crate) use rows::build_sparse_distances;
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
pub use sketch::{SketchOptions, sketch_distances, sketch_distances_from_fasta_list};

/// A sparse distance matrix in coordinate (COO) form.
///
/// `rows`, `columns`, and `distances` have equal lengths. Indices are
/// zero-based and correspond to entries in `names`. COO edge order is
/// intentionally unspecified: SCE rebuilds row membership from `rows` and
/// does not require sorted or symmetric input.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseDistances {
    /// Sample names in row/column order.
    pub names: Vec<String>,
    /// Zero-based COO row indices.
    pub rows: Vec<u64>,
    /// Zero-based COO column indices.
    pub columns: Vec<u64>,
    /// Normalized distances corresponding to `rows` and `columns`.
    pub distances: Vec<f64>,
}

/// Common configuration for every distance constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct DistanceOptions {
    /// Which source-specific edges to retain.
    pub sparsification: Sparsification,
    /// Number of native Rayon threads used for row-wise construction.
    /// Wasm accepts this value but executes sequentially.
    pub threads: usize,
    /// Suppress native distance-construction progress bars.
    pub quiet: bool,
}

impl Default for DistanceOptions {
    fn default() -> Self {
        Self {
            sparsification: Sparsification::Knn(0),
            threads: 1,
            quiet: false,
        }
    }
}

impl SparseDistances {
    /// Construct a sparse matrix after constant-time structural validation.
    pub fn new(
        names: Vec<String>,
        rows: Vec<u64>,
        columns: Vec<u64>,
        distances: Vec<f64>,
    ) -> Result<Self> {
        if rows.len() != columns.len() || rows.len() != distances.len() {
            bail!("COO rows, columns, and distances must have the same length");
        }
        if names.len() < 2 {
            bail!("at least two sample names are required");
        }
        Ok(Self {
            names,
            rows,
            columns,
            distances,
        })
    }

    /// Sample names in row/column order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Zero-based COO row indices.
    pub fn rows(&self) -> &[u64] {
        &self.rows
    }

    /// Zero-based COO column indices.
    pub fn columns(&self) -> &[u64] {
        &self.columns
    }

    /// Normalized distances corresponding to [`Self::rows`] and columns.
    pub fn distances(&self) -> &[f64] {
        &self.distances
    }

    /// Number of samples represented by this matrix.
    pub fn n_samples(&self) -> usize {
        self.names.len()
    }

    /// Number of retained COO edges.
    pub fn len(&self) -> usize {
        self.distances.len()
    }

    /// Whether no COO edges are retained.
    pub fn is_empty(&self) -> bool {
        self.distances.is_empty()
    }

    /// Consume the matrix into labels and COO vectors without copying.
    pub fn into_parts(self) -> (Vec<String>, Vec<u64>, Vec<u64>, Vec<f64>) {
        (self.names, self.rows, self.columns, self.distances)
    }
}

/// Selects which edges are retained by a distance constructor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sparsification {
    /// Keep exactly `min(k, n - 1)` nearest non-self neighbours per sample.
    /// Equal distances are resolved by ascending column index. Zero means all
    /// other samples.
    Knn(usize),
    /// Keep edges whose source-specific distance is below the threshold.
    Threshold(f64),
}

pub(crate) fn validate_sparsification(sparsification: Sparsification) -> Result<()> {
    if let Sparsification::Threshold(threshold) = sparsification
        && (!threshold.is_finite() || !(0.0..=1.0).contains(&threshold))
    {
        bail!("distance threshold must be finite and in [0, 1]");
    }
    Ok(())
}

pub(crate) fn validate_distance_options(options: &DistanceOptions) -> Result<()> {
    if options.threads == 0 {
        bail!("threads must be greater than zero");
    }
    validate_sparsification(options.sparsification)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn with_pool<T, F>(threads: usize, operation: F) -> Result<T>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(anyhow::Error::from)?;
    Ok(pool.install(operation))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn with_pool<T, F>(_: usize, operation: F) -> Result<T>
where
    F: FnOnce() -> T,
{
    Ok(operation())
}

pub(crate) fn distance_progress(options: &DistanceOptions, length: usize) -> PhaseProgress {
    PhaseProgress::new(length as u64, options.quiet, "Distances")
}
