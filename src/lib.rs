//! CPU implementation of Mandrake's stochastic cluster embedding.

pub mod api;
pub mod distances;
mod sce;

pub use api::{EmbeddingInput, EmbeddingOperation, EmbeddingProgress, WtsneOptions};
#[cfg(all(feature = "native-inputs", feature = "sketchlib"))]
pub use distances::{SketchOptions, sketch_distances, sketch_distances_from_fasta_list};
pub use distances::{
    SparseDistances, Sparsification, accessory_distances_from_reader,
    pair_snp_distances_from_reader,
};
#[cfg(feature = "native-inputs")]
pub use distances::{accessory_distances, pair_snp_distances};
pub use sce::wtsne;
