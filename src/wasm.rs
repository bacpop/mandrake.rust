//! WebAssembly boundary for the browser Mandrake tool.

use crate::{
    DistanceOptions, EmbeddingInput, EmbeddingOperation, SparseDistances, Sparsification,
    WtsneOptions, accessory_distances_from_reader, pair_snp_distances_from_reader,
};
use std::fmt::Display;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

fn js_error(error: impl Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn sparsification(mode: &str, value: f64) -> Result<Sparsification, JsValue> {
    match mode {
        "knn" => {
            if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
                return Err(JsValue::from_str(
                    "kNN must be a finite non-negative integer",
                ));
            }
            if value > usize::MAX as f64 {
                return Err(JsValue::from_str("kNN is too large"));
            }
            Ok(Sparsification::Knn(value as usize))
        }
        "threshold" => {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(JsValue::from_str(
                    "distance threshold must be finite and in [0, 1]",
                ));
            }
            Ok(Sparsification::Threshold(value))
        }
        _ => Err(JsValue::from_str(
            "sparsification must be either 'knn' or 'threshold'",
        )),
    }
}

fn options(
    mode: &str,
    value: f64,
    perplexity: f64,
    max_updates: u32,
    repulsion_samples: u32,
    learning_rate: f64,
    initial_exaggeration: bool,
) -> Result<(DistanceOptions, WtsneOptions), JsValue> {
    let sparsification = sparsification(mode, value)?;
    let max_updates =
        usize::try_from(max_updates).map_err(|_| JsValue::from_str("max-updates is too large"))?;
    let repulsion_samples = usize::try_from(repulsion_samples)
        .map_err(|_| JsValue::from_str("repulsion samples is too large"))?;
    Ok((
        DistanceOptions {
            sparsification,
            threads: 1,
            quiet: true,
        },
        WtsneOptions {
            perplexity,
            max_updates,
            repulsion_samples,
            learning_rate,
            initial_exaggeration,
            threads: 1,
            quiet: true,
        },
    ))
}

fn operation_from_distances(
    distances: SparseDistances,
    options: WtsneOptions,
) -> Result<MandrakeOperation, JsValue> {
    let (names, rows, columns, values) = distances.into_parts();
    let input = EmbeddingInput::new(rows, columns, values, names.len(), None).map_err(js_error)?;
    let operation = EmbeddingOperation::new(input, &options).map_err(js_error)?;
    Ok(MandrakeOperation { names, operation })
}

fn alignment_operation(
    bytes: &[u8],
    mode: &str,
    value: f64,
    perplexity: f64,
    max_updates: u32,
    repulsion_samples: u32,
    learning_rate: f64,
    initial_exaggeration: bool,
) -> Result<MandrakeOperation, JsValue> {
    let (distance_options, wtsne_options) = options(
        mode,
        value,
        perplexity,
        max_updates,
        repulsion_samples,
        learning_rate,
        initial_exaggeration,
    )?;
    let distances =
        pair_snp_distances_from_reader(Cursor::new(bytes), &distance_options).map_err(js_error)?;
    operation_from_distances(distances, wtsne_options)
}

fn accessory_operation(
    bytes: &[u8],
    mode: &str,
    value: f64,
    perplexity: f64,
    max_updates: u32,
    repulsion_samples: u32,
    learning_rate: f64,
    initial_exaggeration: bool,
) -> Result<MandrakeOperation, JsValue> {
    let (distance_options, wtsne_options) = options(
        mode,
        value,
        perplexity,
        max_updates,
        repulsion_samples,
        learning_rate,
        initial_exaggeration,
    )?;
    let distances =
        accessory_distances_from_reader(Cursor::new(bytes), &distance_options).map_err(js_error)?;
    operation_from_distances(distances, wtsne_options)
}

#[wasm_bindgen]
pub struct MandrakeOperation {
    names: Vec<String>,
    operation: EmbeddingOperation,
}

#[wasm_bindgen]
impl MandrakeOperation {
    /// Build an operation from a plain FASTA/FASTQ alignment.
    #[wasm_bindgen(js_name = fromAlignment)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_alignment(
        bytes: &[u8],
        mode: &str,
        value: f64,
        perplexity: f64,
        max_updates: u32,
        repulsion_samples: u32,
        learning_rate: f64,
        initial_exaggeration: bool,
    ) -> Result<MandrakeOperation, JsValue> {
        console_error_panic_hook::set_once();
        alignment_operation(
            bytes,
            mode,
            value,
            perplexity,
            max_updates,
            repulsion_samples,
            learning_rate,
            initial_exaggeration,
        )
    }

    /// Build an operation from a plain Roary-style tab-separated table.
    #[wasm_bindgen(js_name = fromAccessory)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_accessory(
        bytes: &[u8],
        mode: &str,
        value: f64,
        perplexity: f64,
        max_updates: u32,
        repulsion_samples: u32,
        learning_rate: f64,
        initial_exaggeration: bool,
    ) -> Result<MandrakeOperation, JsValue> {
        console_error_panic_hook::set_once();
        accessory_operation(
            bytes,
            mode,
            value,
            perplexity,
            max_updates,
            repulsion_samples,
            learning_rate,
            initial_exaggeration,
        )
    }

    /// Advance a bounded number of cooperative optimisation rounds.
    pub fn advance(&mut self, round_budget: u32) -> MandrakeProgress {
        let progress = self.operation.advance(round_budget as usize);
        MandrakeProgress {
            completed: progress.completed_updates() as u32,
            maximum: progress.max_updates() as u32,
            eq: progress.eq(),
        }
    }

    /// Return the current row-major two-dimensional embedding.
    pub fn embedding(&self) -> Vec<f64> {
        self.operation.embedding().iter().copied().collect()
    }

    /// Return one sample name per line in embedding order.
    pub fn names(&self) -> String {
        self.names.join("\n")
    }

    pub fn sample_count(&self) -> u32 {
        self.names.len() as u32
    }

    pub fn is_complete(&self) -> bool {
        self.operation.is_complete()
    }
}

#[wasm_bindgen]
pub struct MandrakeProgress {
    completed: u32,
    maximum: u32,
    eq: f64,
}

#[wasm_bindgen]
impl MandrakeProgress {
    #[wasm_bindgen(getter)]
    pub fn completed(&self) -> u32 {
        self.completed
    }

    #[wasm_bindgen(getter)]
    pub fn maximum(&self) -> u32 {
        self.maximum
    }

    #[wasm_bindgen(getter)]
    pub fn eq(&self) -> f64 {
        self.eq
    }

    #[wasm_bindgen(getter)]
    pub fn complete(&self) -> bool {
        self.completed >= self.maximum
    }
}
