use anyhow::{Context, Result, anyhow, bail};
use needletail::parse_fastx_file;
use sketchlib::distances;
use sketchlib::distances::distance_matrix::DistVec;
use sketchlib::hashing::HashType;
use sketchlib::io::NeedletailIterator;
use sketchlib::sketch::multisketch::MultiSketch;
use sketchlib::sketch::{SketchingOpts, sketch_data};
use sketchlib_native as sketchlib;
use std::path::{Path, PathBuf};

use super::{
    DistanceOptions, SparseDistances, Sparsification, validate_distance_options, with_pool,
};

/// Options used when sketching a FASTA list in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchOptions {
    /// K-mer sizes used for the generated sketch.
    pub kmer_sizes: Vec<usize>,
    /// Number of sketch bins.
    pub sketch_size: u64,
}

impl Default for SketchOptions {
    fn default() -> Self {
        Self {
            kmer_sizes: vec![21],
            sketch_size: 1_000,
        }
    }
}

/// Calculate core distances from an existing sketchlib database.
///
/// Sketchlib 0.4.1 exposes a sparse kNN implementation but no streaming
/// threshold operation, so threshold sparsification is rejected here.
pub fn sketch_distances<P: AsRef<Path>>(
    prefix: P,
    distance_options: &DistanceOptions,
) -> Result<SparseDistances> {
    validate_distance_options(distance_options)?;
    if matches!(
        distance_options.sparsification,
        Sparsification::Threshold(_)
    ) {
        bail!("threshold sparsification is not supported for sketch inputs; use --knn");
    }
    let prefix = normalize_sketch_prefix(prefix.as_ref());
    log::info!("loading sketch database {}", prefix.display());
    let sketches = MultiSketch::load(
        prefix
            .to_str()
            .ok_or_else(|| anyhow!("sketch prefix is not valid UTF-8"))?,
    )
    .with_context(|| format!("loading sketch database {}", prefix.display()))?;
    sparse_sketch_distances(&sketches, distance_options)
}

/// Calculate sketch distances from one FASTA/FASTQ path per sample.
pub fn sketch_distances_from_fasta_list<P: AsRef<Path>>(
    files: &[P],
    distance_options: &DistanceOptions,
    sketch_options: &SketchOptions,
) -> Result<SparseDistances> {
    if files.is_empty() {
        bail!("at least two FASTA files are required");
    }
    validate_distance_options(distance_options)?;
    if sketch_options.kmer_sizes.is_empty() || sketch_options.kmer_sizes.contains(&0) {
        bail!("sketch k-mer sizes must be positive");
    }
    if sketch_options.sketch_size == 0 {
        bail!("sketch size must be positive");
    }
    log::info!("loading {} FASTA inputs for sketch distances", files.len());
    let mut all_sketches = Vec::with_capacity(files.len());
    for path in files {
        let path = path.as_ref();
        let reader = parse_fastx_file(path)
            .map_err(|error| anyhow!(error))
            .with_context(|| format!("opening FASTA input {}", path.display()))?;
        let mut records = vec![NeedletailIterator::new(reader)];
        let mut opts = SketchingOpts::default();
        opts.name = sample_name(path);
        opts.k_vals = sketch_options.kmer_sizes.clone();
        opts.sketch_size = sketch_options.sketch_size;
        let mut sketches = sketch_data(&mut records, opts);
        all_sketches.append(&mut sketches);
    }
    if all_sketches.len() < 2 {
        bail!("at least two FASTA samples are required");
    }
    log::info!(
        "loaded {} FASTA samples for sketch distances",
        all_sketches.len()
    );
    let sketches = MultiSketch::from_sketches(
        &mut all_sketches,
        sketch_options.sketch_size,
        &sketch_options.kmer_sizes,
        HashType::DNA,
    );
    sparse_sketch_distances(&sketches, distance_options)
}

fn sparse_sketch_distances(
    sketches: &MultiSketch,
    distance_options: &DistanceOptions,
) -> Result<SparseDistances> {
    log::info!(
        "constructing sketch distances for {} samples",
        sketches.number_samples_loaded()
    );
    let progress = super::distance_progress(distance_options, 1);
    let result = with_pool(distance_options.threads, || {
        sparse_sketch_distances_inner(sketches, distance_options)
    })?;
    progress.inc(1);
    progress.finish(None);
    let result = result?;
    log::info!(
        "constructed sketch distances for {} samples with {} edges",
        result.n_samples(),
        result.len()
    );
    Ok(result)
}

fn sparse_sketch_distances_inner(
    sketches: &MultiSketch,
    distance_options: &DistanceOptions,
) -> Result<SparseDistances> {
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
    let names = (0..n)
        .map(|index| sketches.sketch_name(index).to_string())
        .collect::<Vec<_>>();

    if sketches.kmer_lengths().len() == 1 {
        let dist_type = distances::set_k(sketches, Some(sketches.kmer_lengths()[0]), false)
            .context("selecting sketch distance")?;
        let sparse = distances::self_dists_knn(sketches, n, k, dist_type, true, None, 0.0);
        let DistVec::Jaccard(values) = sparse.dists_as_ref() else {
            bail!("sketchlib returned an unexpected distance type");
        };
        return sparse_values(
            names,
            k,
            values.iter().map(|value| (value.0, value.1 as f64)),
        );
    }

    let dist_type = distances::set_k(sketches, None, false).context("selecting sketch distance")?;
    let sparse = distances::self_dists_knn(sketches, n, k, dist_type, true, None, 0.0);
    let DistVec::CoreAcc(values) = sparse.dists_as_ref() else {
        bail!("sketchlib returned an unexpected distance type");
    };
    sparse_values(
        names,
        k,
        values.iter().map(|value| (value.0, value.1 as f64)),
    )
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

fn normalize_sketch_prefix(path: &Path) -> PathBuf {
    let mut prefix = path.to_path_buf();
    if prefix
        .extension()
        .is_some_and(|extension| extension == "skm" || extension == "skd")
    {
        prefix.set_extension("");
        let mut value = prefix.to_string_lossy().to_string();
        while value.ends_with('.') {
            value.pop();
        }
        PathBuf::from(value)
    } else {
        prefix
    }
}

fn sample_name(path: &Path) -> String {
    let value = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("sample")
        .to_string();
    [".bz2", ".gz", ".xz", ".fasta", ".fastq", ".fa", ".fq"]
        .iter()
        .find_map(|suffix| value.strip_suffix(suffix))
        .unwrap_or(&value)
        .to_string()
}
