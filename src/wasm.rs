//! WebAssembly boundary for the browser Mandrake tool.

use crate::{
    DistanceOptions, EmbeddingInput, EmbeddingOperation, SparseDistances, Sparsification,
    WtsneOptions,
    distances::{
        DistanceRowBuilder, SampleBases, jaccard_distance, read_accessory_table, read_alignment,
    },
};
use needletail::parse_fastx_reader;
use roaring::RoaringBitmap;
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

enum PendingDistance {
    Alignment {
        sequences: Vec<SampleBases>,
        alignment_len: usize,
        rows: DistanceRowBuilder,
    },
    Accessory {
        profiles: Vec<RoaringBitmap>,
        rows: DistanceRowBuilder,
    },
}

impl PendingDistance {
    fn from_alignment(bytes: &[u8], sparsification: Sparsification) -> Result<Self, JsValue> {
        let mut reader = parse_fastx_reader(Cursor::new(bytes)).map_err(js_error)?;
        let (names, sequences, alignment_len) = read_alignment(&mut *reader).map_err(js_error)?;
        let rows = DistanceRowBuilder::new(names, sparsification).map_err(js_error)?;
        Ok(Self::Alignment {
            sequences,
            alignment_len,
            rows,
        })
    }

    fn from_accessory(bytes: &[u8], sparsification: Sparsification) -> Result<Self, JsValue> {
        let (names, profiles) = read_accessory_table(Cursor::new(bytes)).map_err(js_error)?;
        let rows = DistanceRowBuilder::new(names, sparsification).map_err(js_error)?;
        Ok(Self::Accessory { profiles, rows })
    }

    fn rows(&self) -> &DistanceRowBuilder {
        match self {
            Self::Alignment { rows, .. } | Self::Accessory { rows, .. } => rows,
        }
    }

    fn advance(&mut self, row_budget: usize) {
        match self {
            Self::Alignment {
                sequences,
                alignment_len,
                rows,
            } => rows.advance(row_budget, |left, right| {
                let matches = sequences[left].matching_sites(&sequences[right]).len() as usize;
                let gaps = sequences[left].either_gap_sites(&sequences[right]).len() as usize;
                let comparable = alignment_len.saturating_sub(gaps);
                let mismatches = comparable.saturating_sub(matches);
                mismatches as f64 / *alignment_len as f64
            }),
            Self::Accessory { profiles, rows } => rows.advance(row_budget, |left, right| {
                jaccard_distance(&profiles[left], &profiles[right])
            }),
        }
    }

    fn into_distances(self) -> Result<SparseDistances, JsValue> {
        match self {
            Self::Alignment { rows, .. } | Self::Accessory { rows, .. } => {
                rows.finish().map_err(js_error)
            }
        }
    }
}

enum OperationState {
    Distances(PendingDistance),
    Transitioning,
    Embedding(EmbeddingOperation),
}

fn pending_operation(
    pending: PendingDistance,
    options: WtsneOptions,
) -> Result<MandrakeOperation, JsValue> {
    let names = pending.rows().names().to_vec();
    Ok(MandrakeOperation {
        names,
        wtsne_options: Some(options),
        state: OperationState::Distances(pending),
    })
}

fn operation_from_distances(
    distances: SparseDistances,
    options: WtsneOptions,
) -> Result<EmbeddingOperation, JsValue> {
    let (names, rows, columns, values) = distances.into_parts();
    let input = EmbeddingInput::new(rows, columns, values, names.len(), None).map_err(js_error)?;
    EmbeddingOperation::new(input, &options).map_err(js_error)
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
    let pending = PendingDistance::from_alignment(bytes, distance_options.sparsification)?;
    pending_operation(pending, wtsne_options)
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
    let pending = PendingDistance::from_accessory(bytes, distance_options.sparsification)?;
    pending_operation(pending, wtsne_options)
}

#[wasm_bindgen]
pub struct MandrakeOperation {
    names: Vec<String>,
    wtsne_options: Option<WtsneOptions>,
    state: OperationState,
}

#[wasm_bindgen]
impl MandrakeOperation {
    /// Parse an alignment and prepare a cooperatively stepped distance build.
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

    /// Parse a Roary-style table and prepare a cooperatively stepped distance build.
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

    /// Advance a bounded number of sparse-distance rows.
    #[wasm_bindgen(js_name = advanceDistances)]
    pub fn advance_distances(
        &mut self,
        row_budget: u32,
    ) -> Result<MandrakeDistanceProgress, JsValue> {
        match &mut self.state {
            OperationState::Distances(pending) => {
                pending.advance(row_budget as usize);
                let rows = pending.rows();
                Ok(MandrakeDistanceProgress {
                    completed: rows.completed_rows() as u32,
                    maximum: rows.total_rows() as u32,
                })
            }
            OperationState::Embedding(_) => Ok(MandrakeDistanceProgress {
                completed: self.names.len() as u32,
                maximum: self.names.len() as u32,
            }),
            OperationState::Transitioning => Err(JsValue::from_str(
                "Mandrake operation is transitioning to embedding",
            )),
        }
    }

    /// Finalize sparse distances and initialize the cooperative embedding.
    #[wasm_bindgen(js_name = beginEmbedding)]
    pub fn begin_embedding(&mut self) -> Result<(), JsValue> {
        if matches!(self.state, OperationState::Embedding(_)) {
            return Ok(());
        }

        let state = std::mem::replace(&mut self.state, OperationState::Transitioning);
        let OperationState::Distances(pending) = state else {
            return Err(JsValue::from_str(
                "Mandrake operation has no pending distances",
            ));
        };
        if pending.rows().completed_rows() != pending.rows().total_rows() {
            self.state = OperationState::Distances(pending);
            return Err(JsValue::from_str("distance construction is not complete"));
        }

        let distances = pending.into_distances()?;
        let options = self
            .wtsne_options
            .take()
            .ok_or_else(|| JsValue::from_str("embedding options are unavailable"))?;
        let operation = operation_from_distances(distances, options)?;
        self.state = OperationState::Embedding(operation);
        Ok(())
    }

    /// Advance a bounded number of cooperative optimization rounds.
    pub fn advance(&mut self, round_budget: u32) -> Result<MandrakeProgress, JsValue> {
        let OperationState::Embedding(operation) = &mut self.state else {
            return Err(JsValue::from_str(
                "beginEmbedding must complete before advancing optimization",
            ));
        };
        let progress = operation.advance(round_budget as usize);
        Ok(MandrakeProgress {
            completed: progress.completed_updates() as u32,
            maximum: progress.max_updates() as u32,
            eq: progress.eq(),
        })
    }

    /// Return the current row-major two-dimensional embedding.
    pub fn embedding(&self) -> Result<Vec<f64>, JsValue> {
        let OperationState::Embedding(operation) = &self.state else {
            return Err(JsValue::from_str("embedding has not been initialized"));
        };
        Ok(operation.embedding().iter().copied().collect())
    }

    /// Return one sample name per line in embedding order.
    pub fn names(&self) -> String {
        self.names.join("\n")
    }

    pub fn sample_count(&self) -> u32 {
        self.names.len() as u32
    }

    pub fn is_complete(&self) -> bool {
        matches!(&self.state, OperationState::Embedding(operation) if operation.is_complete())
    }
}

#[wasm_bindgen]
pub struct MandrakeDistanceProgress {
    completed: u32,
    maximum: u32,
}

#[wasm_bindgen]
impl MandrakeDistanceProgress {
    #[wasm_bindgen(getter)]
    pub fn completed(&self) -> u32 {
        self.completed
    }

    #[wasm_bindgen(getter)]
    pub fn maximum(&self) -> u32 {
        self.maximum
    }

    #[wasm_bindgen(getter)]
    pub fn complete(&self) -> bool {
        self.completed >= self.maximum
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
