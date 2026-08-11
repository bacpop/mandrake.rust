//! Sparse distance construction for supported Mandrake input formats.

use anyhow::{Result, bail};

mod accessory;
mod alignment;
#[allow(dead_code)]
mod bitvecs;
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
mod sketch;

#[cfg(feature = "native-inputs")]
pub use accessory::accessory_distances;
pub use accessory::accessory_distances_from_reader;
#[cfg(feature = "native-inputs")]
pub use alignment::pair_snp_distances;
pub use alignment::pair_snp_distances_from_reader;
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
pub use sketch::{SketchOptions, sketch_distances, sketch_distances_from_fasta_list};

/// A sparse distance matrix in coordinate (COO) form.
///
/// `rows`, `columns`, and `distances` have equal lengths. Indices are
/// zero-based and correspond to entries in `names`.
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
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                rows.iter()
                    .chain(&columns)
                    .all(|&index| index < names.len() as u64)
            );
            debug_assert!(
                distances
                    .iter()
                    .all(|&distance| distance.is_finite() && (0.0..=1.0).contains(&distance))
            );
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
    /// Keep the `k` nearest neighbours per sample. Zero means all other
    /// samples, matching the legacy command-line convention.
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

pub(crate) fn select_alignment_row(
    row: usize,
    mut comparisons: Vec<(usize, usize)>,
    alignment_len: usize,
    sparsification: Sparsification,
) -> Vec<(u64, u64, f64)> {
    let cutoff = match sparsification {
        Sparsification::Knn(k) => {
            comparisons.sort_unstable_by_key(|&(column, distance)| (distance, column));
            let neighbour_count = if k == 0 {
                comparisons.len() - 1
            } else {
                k.min(comparisons.len() - 1)
            };
            comparisons[neighbour_count].1
        }
        Sparsification::Threshold(threshold) => {
            let raw = (threshold * alignment_len as f64).floor() as usize;
            raw.saturating_add(1)
        }
    };
    comparisons
        .into_iter()
        .filter(|&(_, distance)| distance <= cutoff)
        .map(|(column, distance)| {
            (
                row as u64,
                column as u64,
                distance as f64 / alignment_len as f64,
            )
        })
        .collect()
}

pub(crate) fn select_distance_row(
    row: usize,
    mut candidates: Vec<(usize, f64)>,
    sparsification: Sparsification,
) -> Vec<(u64, u64, f64)> {
    match sparsification {
        Sparsification::Knn(k) => {
            candidates.sort_unstable_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            });
            let count = if k == 0 {
                candidates.len()
            } else {
                k.min(candidates.len())
            };
            candidates.truncate(count);
        }
        Sparsification::Threshold(threshold) => {
            candidates.retain(|&(_, distance)| distance < threshold);
        }
    }
    candidates
        .into_iter()
        .map(|(column, distance)| (row as u64, column as u64, distance))
        .collect()
}

pub(crate) fn flatten_rows(rows: Vec<Vec<(u64, u64, f64)>>) -> (Vec<u64>, Vec<u64>, Vec<f64>) {
    let total = rows.iter().map(Vec::len).sum();
    let mut row_indices = Vec::with_capacity(total);
    let mut column_indices = Vec::with_capacity(total);
    let mut distances = Vec::with_capacity(total);
    for row in rows {
        for (source, column, distance) in row {
            row_indices.push(source);
            column_indices.push(column);
            distances.push(distance);
        }
    }
    (row_indices, column_indices, distances)
}

pub(crate) fn jaccard_distance(left: &[u8], right: &[u8]) -> f64 {
    let mut intersection = 0usize;
    let mut union = 0usize;
    for (&left, &right) in left.iter().zip(right) {
        if left == 1 && right == 1 {
            intersection += 1;
        }
        if left == 1 || right == 1 {
            union += 1;
        }
    }
    if union == 0 {
        0.0
    } else {
        1.0 - intersection as f64 / union as f64
    }
}
