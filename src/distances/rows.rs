use anyhow::{Result, bail};
use rayon::prelude::*;
use std::collections::BinaryHeap;

use super::{DistanceOptions, SparseDistances, Sparsification, distance_progress, with_pool};

/// Build a labeled COO distance value from a source-specific pair-distance
/// function. Row construction owns sparsification so source adapters do not
/// materialize full candidate rows.
pub(crate) fn build_sparse_distances<F>(
    names: Vec<String>,
    options: &DistanceOptions,
    pair_distance: F,
) -> Result<SparseDistances>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    if names.len() < 2 {
        bail!("at least two sample names are required");
    }

    let n = names.len();
    log::info!("constructing sparse distances for {n} samples");
    let progress = distance_progress(options, n);
    let retained_rows = with_pool(options.threads, || {
        (0..n)
            .into_par_iter()
            .map(|row| {
                let retained = build_row(row, n, options.sparsification, &pair_distance);
                progress.inc(1);
                retained
            })
            .collect::<Vec<_>>()
    })?;
    progress.finish(None);

    let total = retained_rows.iter().map(Vec::len).sum();
    let mut rows = Vec::with_capacity(total);
    let mut columns = Vec::with_capacity(total);
    let mut distances = Vec::with_capacity(total);
    for (row, retained) in retained_rows.into_iter().enumerate() {
        for (column, distance) in retained {
            rows.push(row as u64);
            columns.push(column as u64);
            distances.push(distance);
        }
    }
    let result = SparseDistances::new(names, rows, columns, distances)?;
    log::info!(
        "constructed sparse distances for {} samples with {} edges",
        result.n_samples(),
        result.len()
    );
    Ok(result)
}

/// Sequential row accumulator used by the cooperative wasm distance phase.
///
/// The source-specific pair-distance closure is supplied for each bounded
/// advance, so this type owns only the sparse rows accumulated so far. Native
/// callers continue to use [`build_sparse_distances`] and its private Rayon
/// pool.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub(crate) struct DistanceRowBuilder {
    names: Vec<String>,
    sparsification: Sparsification,
    next_row: usize,
    retained_rows: Vec<Vec<(usize, f64)>>,
}

#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
impl DistanceRowBuilder {
    pub(crate) fn new(names: Vec<String>, sparsification: Sparsification) -> Result<Self> {
        if names.len() < 2 {
            bail!("at least two sample names are required");
        }
        let n = names.len();
        Ok(Self {
            names,
            sparsification,
            next_row: 0,
            retained_rows: vec![Vec::new(); n],
        })
    }

    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }

    pub(crate) fn completed_rows(&self) -> usize {
        self.next_row
    }

    pub(crate) fn total_rows(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn advance<F>(&mut self, row_budget: usize, pair_distance: F)
    where
        F: Fn(usize, usize) -> f64,
    {
        let end = self
            .next_row
            .saturating_add(row_budget)
            .min(self.names.len());
        while self.next_row < end {
            let row = self.next_row;
            self.retained_rows[row] =
                build_row(row, self.names.len(), self.sparsification, &pair_distance);
            self.next_row += 1;
        }
    }

    pub(crate) fn finish(self) -> Result<SparseDistances> {
        if self.next_row != self.names.len() {
            bail!(
                "distance rows are incomplete: {} of {} rows are ready",
                self.next_row,
                self.names.len()
            );
        }

        let total = self.retained_rows.iter().map(Vec::len).sum();
        let mut rows = Vec::with_capacity(total);
        let mut columns = Vec::with_capacity(total);
        let mut distances = Vec::with_capacity(total);
        for (row, retained) in self.retained_rows.into_iter().enumerate() {
            for (column, distance) in retained {
                rows.push(row as u64);
                columns.push(column as u64);
                distances.push(distance);
            }
        }
        SparseDistances::new(self.names, rows, columns, distances)
    }
}

fn build_row<F>(
    row: usize,
    n: usize,
    sparsification: Sparsification,
    pair_distance: &F,
) -> Vec<(usize, f64)>
where
    F: Fn(usize, usize) -> f64,
{
    match sparsification {
        Sparsification::Knn(0) => (0..n)
            .filter(|&column| column != row)
            .map(|column| (column, pair_distance(row, column)))
            .collect(),
        Sparsification::Knn(k) => {
            let mut nearest = TopK::new(k.min(n.saturating_sub(1)));
            for column in 0..n {
                if column != row {
                    nearest.push(Candidate {
                        column,
                        distance: pair_distance(row, column),
                    });
                }
            }
            nearest.into_vec()
        }
        Sparsification::Threshold(threshold) => (0..n)
            .filter(|&column| column != row)
            .filter_map(|column| {
                let distance = pair_distance(row, column);
                (distance < threshold).then_some((column, distance))
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    column: usize,
    distance: f64,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.column == other.column && self.distance.total_cmp(&other.distance).is_eq()
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.column.cmp(&other.column))
    }
}

struct TopK {
    limit: usize,
    heap: BinaryHeap<Candidate>,
}

impl TopK {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            heap: BinaryHeap::with_capacity(limit),
        }
    }

    fn push(&mut self, candidate: Candidate) {
        if self.limit == 0 {
            return;
        }
        if self.heap.len() < self.limit {
            self.heap.push(candidate);
        } else if self.heap.peek().is_some_and(|&worst| candidate < worst) {
            self.heap.pop();
            self.heap.push(candidate);
        }
    }

    fn into_vec(self) -> Vec<(usize, f64)> {
        self.heap
            .into_iter()
            .map(|candidate| (candidate.column, candidate.distance))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Candidate, DistanceRowBuilder, TopK};
    use crate::Sparsification;

    #[test]
    fn bounded_heap_never_exceeds_its_limit() {
        let mut nearest = TopK::new(3);
        for column in 0..100 {
            nearest.push(Candidate {
                column,
                distance: (100 - column) as f64,
            });
            assert!(nearest.heap.len() <= 3);
        }
    }

    #[test]
    fn cooperative_rows_are_bounded_and_finish_in_order() {
        let mut builder = DistanceRowBuilder::new(
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            Sparsification::Knn(1),
        )
        .unwrap();
        builder.advance(0, |left, right| (left.abs_diff(right)) as f64);
        assert_eq!(builder.completed_rows(), 0);
        builder.advance(1, |left, right| (left.abs_diff(right)) as f64);
        assert_eq!(builder.completed_rows(), 1);
        builder.advance(10, |left, right| (left.abs_diff(right)) as f64);
        assert_eq!(builder.completed_rows(), 3);

        let distances = builder.finish().unwrap();
        assert_eq!(distances.names(), ["a", "b", "c"]);
        assert_eq!(distances.len(), 3);
    }
}
