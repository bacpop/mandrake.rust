//! CPU implementation of Mandrake's stochastic cluster embedding.

use anyhow::{Context, Result, bail};
use ndarray::Array2;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::api::{EmbeddingInput, EmbeddingProgress, WtsneOptions};
use crate::progress::PhaseProgress;

type Xoshiro = Xoshiro256PlusPlus;

const DIMENSIONS: usize = 2;
const PROBABILITY_TOLERANCE: f64 = 1e-5;
const PERPLEXITY_STEPS: usize = 100;
const UPDATE_STREAM_DOMAIN: u64 = 0x5550_4441_5445_5354;
const INITIAL_STREAM_DOMAIN: u64 = 0x5343_455f_494e_4954;
const STREAM_MULTIPLIER: u64 = 0x9e37_79b9_7f4a_7c15;

fn seeded_rng(seed: u64, domain: u64, index: u64) -> Xoshiro {
    let stream_seed = seed
        .wrapping_add(domain)
        .wrapping_add(index.wrapping_mul(STREAM_MULTIPLIER));
    Xoshiro::seed_from_u64(stream_seed)
}

fn next_unit(rng: &mut Xoshiro) -> f64 {
    rng.next_u64() as f64 / 18_446_744_073_709_551_616.0
}

fn update_rng_streams(seed: u64, threads: usize) -> Vec<Xoshiro> {
    let mut root = seeded_rng(seed, UPDATE_STREAM_DOMAIN, 0);
    let mut streams = Vec::with_capacity(threads);
    for _ in 0..threads {
        streams.push(root.clone());
        root.jump();
    }
    streams
}

/// Run one embedding calculation to its configured update target.
pub fn wtsne(input: EmbeddingInput, options: &WtsneOptions) -> Result<Array2<f64>> {
    let mut operation = EmbeddingOperationInner::new(input, options)?;
    operation.advance(usize::MAX);
    Ok(operation.into_embedding())
}

/// Private state behind the public cooperative operation facade.
pub(crate) struct EmbeddingOperationInner {
    embedding: AtomicEmbedding,
    current_embedding: Array2<f64>,
    edge_table: AliasTable,
    node_table: AliasTable,
    rows: Vec<u64>,
    columns: Vec<u64>,
    rng_states: Vec<Xoshiro>,
    n_nodes: usize,
    max_updates: usize,
    completed_updates: usize,
    threads: usize,
    repulsion_samples: usize,
    learning_rate: f64,
    initial_exaggeration: bool,
    eq: f64,
    optimization_started: bool,
    optimization_progress: PhaseProgress,
    #[cfg(not(target_arch = "wasm32"))]
    pool: rayon::ThreadPool,
}

impl EmbeddingOperationInner {
    pub(crate) fn new(input: EmbeddingInput, options: &WtsneOptions) -> Result<Self> {
        Self::new_with_seed(input, options, None)
    }

    fn new_with_seed(
        input: EmbeddingInput,
        options: &WtsneOptions,
        configured_seed: Option<u64>,
    ) -> Result<Self> {
        validate_options(options)?;
        if input.rows.is_empty() {
            bail!("at least one COO edge is required");
        }
        let seed = configured_seed.unwrap_or_else(system_seed);

        #[cfg(not(target_arch = "wasm32"))]
        let threads = options.threads;
        #[cfg(target_arch = "wasm32")]
        let threads = 1;

        #[cfg(not(target_arch = "wasm32"))]
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(options.threads)
            .build()
            .context("building Rayon thread pool")?;

        let row_edges = make_row_edges(&input.rows, input.n_nodes);
        log::info!(
            "calculating conditional probabilities for {} edges",
            input.distances.len()
        );
        #[cfg(not(target_arch = "wasm32"))]
        let probabilities = pool.install(|| {
            conditional_probabilities(
                &row_edges,
                &input.distances,
                options.perplexity,
                options.quiet,
            )
        })?;
        #[cfg(target_arch = "wasm32")]
        let probabilities = conditional_probabilities(
            &row_edges,
            &input.distances,
            options.perplexity,
            options.quiet,
        )?;
        log::info!("conditional probabilities calculated");
        let edge_table = AliasTable::new(&probabilities).context("building edge sampler")?;
        let node_table = AliasTable::new(&input.weights).context("building node sampler")?;

        log::info!("initializing embedding for {} nodes", input.n_nodes);
        #[cfg(not(target_arch = "wasm32"))]
        let initial = initial_embedding(input.n_nodes, seed, &pool);
        #[cfg(target_arch = "wasm32")]
        let initial = initial_embedding(input.n_nodes, seed);

        let current_embedding =
            Array2::from_shape_vec((input.n_nodes, DIMENSIONS), initial.clone())
                .expect("initial embedding has the expected shape");
        log::info!("embedding initialized");
        let embedding = AtomicEmbedding::new(initial);
        let rng_states = update_rng_streams(seed, threads);

        Ok(Self {
            embedding,
            current_embedding,
            edge_table,
            node_table,
            rows: input.rows,
            columns: input.columns,
            rng_states,
            n_nodes: input.n_nodes,
            max_updates: options.max_updates,
            completed_updates: 0,
            threads,
            repulsion_samples: options.repulsion_samples,
            learning_rate: options.learning_rate,
            initial_exaggeration: options.initial_exaggeration,
            eq: 1.0,
            optimization_started: false,
            optimization_progress: PhaseProgress::new(
                options.max_updates as u64,
                options.quiet,
                "Optimizing",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            pool,
        })
    }

    pub(crate) fn advance(&mut self, round_budget: usize) -> EmbeddingProgress {
        if round_budget == 0 || self.is_complete() {
            return self.progress();
        }
        let remaining = self.max_updates.saturating_sub(self.completed_updates);
        let rounds_needed = remaining.div_ceil(self.threads);
        let rounds = round_budget.min(rounds_needed);
        if !self.optimization_started {
            log::info!(
                "starting embedding optimisation for up to {} updates",
                self.max_updates
            );
            self.optimization_started = true;
        }
        for _ in 0..rounds {
            self.advance_round();
        }
        if self.is_complete() {
            self.optimization_progress.finish(Some(format!(
                "completed {} updates (target {})",
                self.completed_updates, self.max_updates
            )));
            log::info!(
                "embedding optimisation complete after {} updates",
                self.completed_updates
            );
        }
        self.progress()
    }

    pub(crate) fn embedding(&self) -> &Array2<f64> {
        &self.current_embedding
    }

    pub(crate) fn into_embedding(self) -> Array2<f64> {
        if !self.is_complete() {
            log::warn!(
                "consuming incomplete embedding operation after {}/{} updates",
                self.completed_updates,
                self.max_updates
            );
        }
        self.current_embedding
    }

    fn progress(&self) -> EmbeddingProgress {
        EmbeddingProgress::new(self.completed_updates, self.max_updates, self.eq)
    }

    fn is_complete(&self) -> bool {
        self.completed_updates >= self.max_updates
    }

    fn advance_round(&mut self) {
        let update = self.completed_updates;
        let eta = (self.learning_rate * (1.0 - update as f64 / self.max_updates as f64))
            .max(self.learning_rate * 1e-4);
        let attraction_coefficient = if self.initial_exaggeration && update < self.max_updates / 10
        {
            8.0
        } else {
            2.0
        };
        let repulsion_coefficient = 2.0 / (self.eq * self.repulsion_samples as f64);
        let context = UpdateContext {
            embedding: &self.embedding,
            edge_table: &self.edge_table,
            node_table: &self.node_table,
            rows: &self.rows,
            columns: &self.columns,
            eta,
            attraction_coefficient,
            repulsion_coefficient,
            repulsion_samples: self.repulsion_samples,
        };
        #[cfg(not(target_arch = "wasm32"))]
        let (qsum, qcount, _) = {
            let pool = &self.pool;
            let rng_states = &mut self.rng_states;
            pool.install(|| run_updates(rng_states, &context))
        };
        #[cfg(target_arch = "wasm32")]
        let (qsum, qcount, _) = run_updates(&mut self.rng_states, &context);
        let n_squared = self.n_nodes as f64 * (self.n_nodes - 1) as f64;
        self.eq = (self.eq * n_squared + qsum) / (n_squared + qcount as f64);
        self.completed_updates = self.completed_updates.saturating_add(self.threads);
        self.optimization_progress
            .set(self.completed_updates.min(self.max_updates) as u64, || {
                format!("Eq={:.6}", self.eq)
            });
        self.embedding.copy_into(&mut self.current_embedding);
    }
}

fn system_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn validate_options(options: &WtsneOptions) -> Result<()> {
    if !options.perplexity.is_finite() {
        bail!("perplexity must be finite");
    }
    if options.max_updates == 0 {
        bail!("max_updates must be greater than zero");
    }
    if options.repulsion_samples == 0 {
        bail!("repulsion_samples must be greater than zero");
    }
    if options.threads == 0 {
        bail!("threads must be greater than zero");
    }
    if !options.learning_rate.is_finite() || options.learning_rate <= 0.0 {
        bail!("learning_rate must be finite and positive");
    }
    Ok(())
}

fn make_row_edges(rows: &[u64], n_nodes: usize) -> Vec<Vec<usize>> {
    let mut row_edges = vec![Vec::new(); n_nodes];
    for (edge, &source) in rows.iter().enumerate() {
        row_edges[source as usize].push(edge);
    }
    row_edges
}

fn conditional_probabilities(
    rows: &[Vec<usize>],
    distances: &[f64],
    perplexity: f64,
    quiet: bool,
) -> Result<Vec<f64>> {
    let mut probabilities = vec![0.0; distances.len()];
    let progress = PhaseProgress::new(distances.len() as u64, quiet, "Probabilities");
    if perplexity <= 0.0 {
        probabilities
            .par_iter_mut()
            .zip(distances.par_iter())
            .for_each(|(probability, &distance)| {
                *probability = 1.0 - distance;
                progress.inc(1);
            });
    } else {
        let desired_entropy = perplexity.ln();
        let row_probabilities: Vec<Vec<f64>> = rows
            .par_iter()
            .map(|row| {
                let mut beta = 1.0;
                let mut beta_min = -f64::MAX;
                let mut beta_max = f64::MAX;
                let mut row_probabilities = vec![0.0; row.len()];

                for _ in 0..PERPLEXITY_STEPS {
                    let mut sum = 0.0;
                    for (slot, &edge) in row.iter().enumerate() {
                        let probability = (-distances[edge] * beta).exp();
                        row_probabilities[slot] = probability;
                        sum += probability;
                    }
                    if sum == 0.0 || !sum.is_finite() {
                        let uniform = 1.0 / row.len() as f64;
                        row_probabilities.fill(uniform);
                        break;
                    }
                    let mut weighted_distance = 0.0;
                    for (slot, &edge) in row.iter().enumerate() {
                        row_probabilities[slot] /= sum;
                        weighted_distance += distances[edge] * row_probabilities[slot];
                    }
                    let entropy = sum.ln() + beta * weighted_distance;
                    let difference = entropy - desired_entropy;
                    if difference.abs() <= PROBABILITY_TOLERANCE {
                        break;
                    }
                    if difference > 0.0 {
                        beta_min = beta;
                        beta = if beta_max == f64::MAX {
                            beta * 2.0
                        } else {
                            (beta + beta_max) * 0.5
                        };
                    } else {
                        beta_max = beta;
                        beta = if beta_min == -f64::MAX {
                            beta * 0.5
                        } else {
                            (beta + beta_min) * 0.5
                        };
                    }
                }

                progress.inc(row.len() as u64);
                row_probabilities
            })
            .collect();
        for (row, row_values) in rows.iter().zip(row_probabilities) {
            for (slot, &edge) in row.iter().enumerate() {
                probabilities[edge] = row_values[slot];
            }
        }
    }

    normalize(&mut probabilities, "edge probabilities")?;
    Ok(probabilities)
}

fn normalize(values: &mut [f64], name: &str) -> Result<()> {
    let sum: f64 = values.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        bail!("{name} must have a positive finite total");
    }
    for value in values {
        *value /= sum;
    }
    Ok(())
}

struct AliasTable {
    probabilities: Vec<f64>,
    aliases: Vec<usize>,
}

impl AliasTable {
    fn new(values: &[f64]) -> Result<Self> {
        if values.is_empty() {
            bail!("probability table cannot be empty");
        }
        let total: f64 = values.iter().sum();
        if !total.is_finite() || total <= 0.0 {
            bail!("probability table must have a positive finite total");
        }

        let n = values.len();
        let mut probabilities = vec![0.0; n];
        let mut aliases: Vec<usize> = (0..n).collect();
        let mut small = Vec::new();
        let mut large = Vec::new();
        for (index, value) in values.iter().enumerate() {
            probabilities[index] = value * n as f64 / total;
            if probabilities[index] < 1.0 {
                small.push(index);
            } else {
                large.push(index);
            }
        }
        while let (Some(s), Some(l)) = (small.pop(), large.pop()) {
            aliases[s] = l;
            probabilities[l] = probabilities[l] + probabilities[s] - 1.0;
            if probabilities[l] < 1.0 {
                small.push(l);
            } else {
                large.push(l);
            }
        }
        for index in small.into_iter().chain(large) {
            probabilities[index] = 1.0;
            aliases[index] = index;
        }
        Ok(Self {
            probabilities,
            aliases,
        })
    }

    fn draw(&self, rng: &mut Xoshiro) -> usize {
        let column = (next_unit(rng) * self.probabilities.len() as f64) as usize;
        if next_unit(rng) < self.probabilities[column] {
            column
        } else {
            self.aliases[column]
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn initial_embedding(n_nodes: usize, seed: u64, pool: &rayon::ThreadPool) -> Vec<f64> {
    let mut values = vec![0.0; n_nodes * DIMENSIONS];
    pool.install(|| initialise_embedding(&mut values, seed));
    values
}

#[cfg(target_arch = "wasm32")]
fn initial_embedding(n_nodes: usize, seed: u64) -> Vec<f64> {
    let mut values = vec![0.0; n_nodes * DIMENSIONS];
    initialise_embedding(&mut values, seed);
    values
}

fn initialise_embedding(values: &mut [f64], seed: u64) {
    values
        .par_chunks_exact_mut(DIMENSIONS)
        .enumerate()
        .for_each(|(index, point)| {
            let mut rng = seeded_rng(seed, INITIAL_STREAM_DOMAIN, index as u64);
            loop {
                let x = 2.0 * next_unit(&mut rng) - 1.0;
                let y = 2.0 * next_unit(&mut rng) - 1.0;
                let norm = x * x + y * y;
                if norm > 0.0 && norm <= 1.0 {
                    point[0] = x * 1e-4;
                    point[1] = y * 1e-4;
                    break;
                }
            }
        });
}

struct UpdateContext<'a> {
    embedding: &'a AtomicEmbedding,
    edge_table: &'a AliasTable,
    node_table: &'a AliasTable,
    rows: &'a [u64],
    columns: &'a [u64],
    eta: f64,
    attraction_coefficient: f64,
    repulsion_coefficient: f64,
    repulsion_samples: usize,
}

fn run_updates(rng_states: &mut [Xoshiro], context: &UpdateContext<'_>) -> (f64, u64, u64) {
    rng_states
        .par_iter_mut()
        .map(|rng| run_update(rng, context))
        .reduce(
            || (0.0, 0_u64, 0_u64),
            |left, right| (left.0 + right.0, left.1 + right.1, left.2 + right.2),
        )
}

fn run_update(rng: &mut Xoshiro, context: &UpdateContext<'_>) -> (f64, u64, u64) {
    let edge = context.edge_table.draw(rng);
    let attraction_i = context.rows[edge] as usize;
    let attraction_j = context.columns[edge] as usize;
    let mut qsum = 0.0;
    let mut qcount = 0_u64;
    let mut clashes = 0_u64;

    for sample in 0..=context.repulsion_samples {
        let (k, l) = if sample == 0 {
            (attraction_i, attraction_j)
        } else {
            (context.node_table.draw(rng), context.node_table.draw(rng))
        };
        if k == l {
            continue;
        }

        let k_offset = k * DIMENSIONS;
        let l_offset = l * DIMENSIONS;
        let k_read = [
            context.embedding.load(k_offset),
            context.embedding.load(k_offset + 1),
        ];
        let l_read = [
            context.embedding.load(l_offset),
            context.embedding.load(l_offset + 1),
        ];
        let delta = [k_read[0] - l_read[0], k_read[1] - l_read[1]];
        let dist_squared = delta[0] * delta[0] + delta[1] * delta[1];
        let q = 1.0 / (1.0 + dist_squared);
        let coefficient = if sample == 0 {
            -context.attraction_coefficient * q
        } else {
            context.repulsion_coefficient * q * q
        };
        let gain = [
            context.eta * coefficient * delta[0],
            context.eta * coefficient * delta[1],
        ];
        let changed_k0 = context.embedding.add(k_offset, gain[0], k_read[0]);
        let changed_l0 = context.embedding.add(l_offset, -gain[0], l_read[0]);
        let changed_k1 = context.embedding.add(k_offset + 1, gain[1], k_read[1]);
        let changed_l1 = context.embedding.add(l_offset + 1, -gain[1], l_read[1]);

        if changed_k0 && changed_l0 && changed_k1 && changed_l1 {
            qsum += q;
            qcount += 1;
        } else {
            context.embedding.add_unchecked(k_offset, -gain[0]);
            context.embedding.add_unchecked(l_offset, gain[0]);
            context.embedding.add_unchecked(k_offset + 1, -gain[1]);
            context.embedding.add_unchecked(l_offset + 1, -gain[1]);
            clashes += 1;
        }
    }
    (qsum, qcount, clashes)
}

struct AtomicEmbedding {
    values: Vec<AtomicU64>,
}

impl AtomicEmbedding {
    fn new(values: Vec<f64>) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|value| AtomicU64::new(value.to_bits()))
                .collect(),
        }
    }

    fn load(&self, index: usize) -> f64 {
        f64::from_bits(self.values[index].load(Ordering::SeqCst))
    }

    fn add(&self, index: usize, delta: f64, expected: f64) -> bool {
        let atomic = &self.values[index];
        let expected_result = (expected + delta).to_bits();
        let mut observed = atomic.load(Ordering::SeqCst);
        loop {
            let current = f64::from_bits(observed);
            let next = (current + delta).to_bits();
            match atomic.compare_exchange_weak(observed, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return next == expected_result,
                Err(actual) => observed = actual,
            }
        }
    }

    fn add_unchecked(&self, index: usize, delta: f64) {
        let atomic = &self.values[index];
        let mut observed = atomic.load(Ordering::SeqCst);
        loop {
            let current = f64::from_bits(observed);
            let next = (current + delta).to_bits();
            match atomic.compare_exchange_weak(observed, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return,
                Err(actual) => observed = actual,
            }
        }
    }

    fn copy_into(&self, target: &mut Array2<f64>) {
        for (target, source) in target.iter_mut().zip(&self.values) {
            *target = f64::from_bits(source.load(Ordering::SeqCst));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EmbeddingOperationInner;
    use crate::api::{EmbeddingInput, WtsneOptions};

    fn graph() -> EmbeddingInput {
        EmbeddingInput::new(
            vec![0, 0, 1, 1, 2, 2, 3, 3],
            vec![1, 2, 0, 2, 0, 3, 0, 2],
            vec![0.1, 0.4, 0.1, 0.2, 0.4, 0.3, 0.3, 0.2],
            4,
            Some(vec![1.0, 2.0, 1.0, 1.0]),
        )
        .unwrap()
    }

    fn options() -> WtsneOptions {
        WtsneOptions {
            perplexity: 2.0,
            max_updates: 20,
            repulsion_samples: 1,
            learning_rate: 1.0,
            initial_exaggeration: false,
            threads: 1,
            quiet: true,
        }
    }

    #[test]
    fn configured_seed_keeps_private_single_thread_runs_reproducible() {
        let mut first =
            EmbeddingOperationInner::new_with_seed(graph(), &options(), Some(42)).unwrap();
        first.advance(20);
        let first = first.into_embedding();

        let mut second =
            EmbeddingOperationInner::new_with_seed(graph(), &options(), Some(42)).unwrap();
        second.advance(20);
        let second = second.into_embedding();

        assert_eq!(first, second);
    }

    #[test]
    fn configured_seed_preserves_budget_partitioning() {
        let mut single =
            EmbeddingOperationInner::new_with_seed(graph(), &options(), Some(42)).unwrap();
        single.advance(20);
        let single = single.into_embedding();

        let mut partitioned =
            EmbeddingOperationInner::new_with_seed(graph(), &options(), Some(42)).unwrap();
        partitioned.advance(3);
        partitioned.advance(7);
        partitioned.advance(10);
        let partitioned = partitioned.into_embedding();

        assert_eq!(single, partitioned);
    }
}
