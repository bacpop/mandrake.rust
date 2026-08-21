//! CPU implementation of Mandrake's stochastic cluster embedding.

pub mod api;
pub mod distances;
#[allow(dead_code)]
mod hdbscan;
mod progress;
mod sce;
#[cfg(target_family = "wasm")]
mod wasm;

pub use api::{EmbeddingInput, EmbeddingOperation, EmbeddingProgress, WtsneOptions};
pub use distances::{
    DistanceOptions, SparseDistances, Sparsification, accessory_distances_from_reader,
    pair_snp_distances_from_reader,
};
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
pub use distances::{SketchOptions, sketch_distances, sketch_distances_from_fasta_list};
#[cfg(feature = "native-inputs")]
pub use distances::{accessory_distances, pair_snp_distances};
pub use sce::wtsne;
