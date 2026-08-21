//! Browser sketch-database loading and sparse kNN construction.

use anyhow::{Result, bail};
use sketchlib::distances::{
    self,
    distance_matrix::{DistType, DistVec},
};
use sketchlib::sketch::multisketch::MultiSketch;
use sketchlib_wasm as sketchlib;

use super::{DistanceOptions, SparseDistances, Sparsification, validate_distance_options};

/// Distance choices supported for browser sketch databases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SketchDistanceKind {
    Core,
    Jaccard(usize),
}

/// Inspect a current-format `.skm` file without loading `.skd` bins.
pub(crate) fn sketch_kmer_lengths(metadata: &[u8]) -> Result<Vec<usize>> {
    let sketches = MultiSketch::load_metadata_bytes(metadata)?;
    reject_legacy(&sketches)?;
    Ok(sketches.kmer_lengths().to_vec())
}

/// Calculate sparse distances from paired current-format `.skm`/`.skd` bytes.
pub(crate) fn sketch_distances_from_bytes(
    metadata: &[u8],
    data: &[u8],
    distance_options: &DistanceOptions,
    distance_kind: SketchDistanceKind,
) -> Result<SparseDistances> {
    validate_distance_options(distance_options)?;
    if matches!(
        distance_options.sparsification,
        Sparsification::Threshold(_)
    ) {
        bail!("threshold sparsification is not supported for sketch inputs; use kNN");
    }

    let sketches = MultiSketch::load_bytes(metadata, data)?;
    reject_legacy(&sketches)?;
    if matches!(distance_kind, SketchDistanceKind::Core) && sketches.kmer_lengths().len() < 2 {
        bail!("core distance requires at least two k-mer lengths; choose Jaccard");
    }
    let n = sketches.number_samples_loaded();
    if n < 2 {
        bail!("at least two sketch samples are required");
    }
    let requested_k = match distance_options.sparsification {
        Sparsification::Knn(k) => k,
        Sparsification::Threshold(_) => unreachable!(),
    };
    let k = if requested_k == 0 {
        n - 1
    } else {
        requested_k.min(n - 1)
    };
    let dist_type = match distance_kind {
        SketchDistanceKind::Core => DistType::CoreAcc,
        SketchDistanceKind::Jaccard(kmer) => {
            let k_index = sketches.get_k_idx(kmer).ok_or_else(|| {
                anyhow::anyhow!("k-mer size {kmer} is not present in the sketch database")
            })?;
            DistType::Jaccard(k_index, kmer as f64, false)
        }
    };

    let sparse =
        distances::self_dists_knn_generic::<16>(&sketches, n, k, dist_type, true, None, 0.0);
    let names = (0..n)
        .map(|index| sketches.sketch_name(index).to_string())
        .collect::<Vec<_>>();
    match sparse.dists_as_ref() {
        DistVec::Jaccard(values) => sparse_values(
            names,
            k,
            values.iter().map(|value| (value.0, value.1 as f64)),
        ),
        DistVec::CoreAcc(values) => sparse_values(
            names,
            k,
            values.iter().map(|value| (value.0, value.1 as f64)),
        ),
    }
}

fn reject_legacy(sketches: &MultiSketch) -> Result<()> {
    if sketches.is_legacy_format() {
        bail!("legacy sketch databases use 14-bit bins; only new BB=16 files are supported")
    }
    Ok(())
}

fn sparse_values(
    names: Vec<String>,
    k: usize,
    values: impl Iterator<Item = (usize, f64)>,
) -> Result<SparseDistances> {
    let values = values.collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(values.len());
    let mut columns = Vec::with_capacity(values.len());
    let mut distances = Vec::with_capacity(values.len());
    for (edge, (column, distance)) in values.into_iter().enumerate() {
        rows.push((edge / k) as u64);
        columns.push(column as u64);
        distances.push(distance);
    }
    SparseDistances::new(names, rows, columns, distances)
}
