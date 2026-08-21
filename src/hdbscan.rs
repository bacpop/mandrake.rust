//! HDBSCAN labelling for the final two-dimensional embedding.

use anyhow::{Result, bail};
use hdbscan::{DistanceMetric, Hdbscan, HdbscanHyperParams, NnAlgorithm};

const MIN_CLUSTER_SIZE: usize = 2;
const MIN_SAMPLES: usize = 2;
const CLUSTER_SELECTION_EPSILON: f64 = 0.02;

/// Cluster a row-major two-dimensional embedding with the browser preset.
///
/// The centering and half-range scaling mirror the Python plotting helper. The
/// returned labels are in sample order; `-1` denotes HDBSCAN noise.
pub(crate) fn cluster_embedding(embedding: &[f64]) -> Result<Vec<i32>> {
    if !embedding.len().is_multiple_of(2) {
        bail!("embedding must contain an even number of coordinates");
    }
    let sample_count = embedding.len() / 2;
    if sample_count < 2 {
        bail!("HDBSCAN requires at least two samples");
    }

    let mut points = Vec::with_capacity(sample_count);
    for index in (0..embedding.len()).step_by(2) {
        let x = embedding[index];
        let y = embedding[index + 1];
        if !x.is_finite() || !y.is_finite() {
            bail!("embedding contains a non-finite coordinate");
        }
        points.push(vec![x, y]);
    }

    for dimension in 0..2 {
        let mean = points.iter().map(|point| point[dimension]).sum::<f64>() / sample_count as f64;
        for point in &mut points {
            point[dimension] -= mean;
        }
        let (minimum, maximum) = points.iter().map(|point| point[dimension]).fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
        );
        let scale = 0.5 * (maximum - minimum);
        if !scale.is_finite() || scale == 0.0 {
            bail!("embedding dimension has zero range");
        }
        for point in &mut points {
            point[dimension] /= scale;
        }
    }

    let hyper_params = HdbscanHyperParams::builder()
        .min_cluster_size(MIN_CLUSTER_SIZE)
        .min_samples(MIN_SAMPLES)
        .epsilon(CLUSTER_SELECTION_EPSILON)
        .allow_single_cluster(true)
        .dist_metric(DistanceMetric::Euclidean)
        .nn_algorithm(NnAlgorithm::Auto)
        .build();

    Hdbscan::new(&points, hyper_params)
        .cluster()
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::cluster_embedding;
    use std::collections::HashSet;

    #[test]
    fn separates_compact_groups_and_marks_outlier_noise() {
        let embedding = [
            1.5, 2.2, 1.0, 1.1, 1.2, 1.4, 0.8, 1.0, 1.1, 1.0, 3.7, 4.0, 3.9, 3.9, 3.6, 4.1, 3.8,
            3.9, 4.0, 4.1, 10.0, 10.0,
        ];
        let labels = cluster_embedding(&embedding).expect("fixture should cluster");

        assert_eq!(labels, [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, -1]);
        assert_eq!(labels[..5].iter().collect::<HashSet<_>>().len(), 1);
        assert_eq!(labels[5..10].iter().collect::<HashSet<_>>().len(), 1);
        assert_ne!(labels[0], labels[5]);
        assert_eq!(labels[10], -1);
    }

    #[test]
    fn rejects_malformed_or_degenerate_embeddings() {
        assert!(cluster_embedding(&[0.0]).is_err());
        assert!(cluster_embedding(&[0.0, 0.0]).is_err());
        assert!(cluster_embedding(&[0.0, 0.0, 1.0, f64::NAN]).is_err());
        assert!(cluster_embedding(&[0.0, 0.0, 0.0, 0.0]).is_err());
    }
}
