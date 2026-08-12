use anyhow::{Context, Result, anyhow, bail};
use needletail::parse_fastx_file;
use sketchlib::distances;
use sketchlib::distances::distance_matrix::DistVec;
use sketchlib::hashing::HashType;
use sketchlib::io::NeedletailIterator;
use sketchlib::sketch::multisketch::MultiSketch;
use sketchlib::sketch::{SketchingOpts, sketch_data};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{
    DistanceOptions, SparseDistances, Sparsification, flatten_rows, select_distance_row,
    validate_distance_options, with_pool,
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

/// Calculate core or accessory distances from an existing sketchlib database.
///
/// Sketchlib 0.4.1 exposes a sparse kNN implementation but no streaming
/// threshold operation, so threshold sparsification is rejected here.
pub fn sketch_distances<P: AsRef<Path>>(
    prefix: P,
    distance_options: &DistanceOptions,
    use_accessory: bool,
) -> Result<SparseDistances> {
    validate_distance_options(distance_options)?;
    if matches!(
        distance_options.sparsification,
        Sparsification::Threshold(_)
    ) {
        bail!("threshold sparsification is not supported for sketch inputs; use --knn");
    }
    let prefix = normalize_sketch_prefix(prefix.as_ref());
    let sketches = MultiSketch::load(
        prefix
            .to_str()
            .ok_or_else(|| anyhow!("sketch prefix is not valid UTF-8"))?,
    )
    .with_context(|| format!("loading sketch database {}", prefix.display()))?;
    sparse_sketch_distances(&sketches, distance_options, use_accessory)
}

/// Calculate sketch distances from one FASTA/FASTQ path per sample.
pub fn sketch_distances_from_fasta_list<P: AsRef<Path>>(
    files: &[P],
    distance_options: &DistanceOptions,
    use_accessory: bool,
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
    let sketches = MultiSketch::from_sketches(
        &mut all_sketches,
        sketch_options.sketch_size,
        &sketch_options.kmer_sizes,
        HashType::DNA,
    );
    sparse_sketch_distances(&sketches, distance_options, use_accessory)
}

fn sparse_sketch_distances(
    sketches: &MultiSketch,
    distance_options: &DistanceOptions,
    use_accessory: bool,
) -> Result<SparseDistances> {
    let progress = super::distance_progress(distance_options, 1);
    // sketchlib's streaming accessory implementation uses a scoped producer
    // and a blocking writer loop; leave one coordinator lane available when
    // the caller requests a single thread.
    let pool_threads = if use_accessory {
        distance_options.threads.max(2)
    } else {
        distance_options.threads
    };
    let result = with_pool(pool_threads, || {
        sparse_sketch_distances_inner(sketches, distance_options, use_accessory)
    })?;
    progress.inc(1);
    progress.finish(None);
    result
}

fn sparse_sketch_distances_inner(
    sketches: &MultiSketch,
    distance_options: &DistanceOptions,
    use_accessory: bool,
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
        if use_accessory {
            bail!("accessory sketch distances require at least two k-mer lengths");
        }
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

    if use_accessory {
        return sparse_sketch_accessory(sketches, names, k, distance_options);
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

fn sparse_sketch_accessory(
    sketches: &MultiSketch,
    names: Vec<String>,
    k: usize,
    distance_options: &DistanceOptions,
) -> Result<SparseDistances> {
    let n = names.len();
    let dist_type = distances::set_k(sketches, None, false).context("selecting sketch distance")?;
    let mut writer = SketchAccessoryWriter::new(names.clone(), k);
    distances::self_dists_all_stream(
        &mut writer,
        sketches,
        n,
        dist_type,
        true,
        None,
        0.0,
        distance_options.threads,
    )
    .map_err(|error| anyhow!(error))
    .context("streaming sketch accessory distances")?;
    let candidates = writer.finish();
    let rows = candidates
        .into_iter()
        .enumerate()
        .map(|(row, candidate)| select_distance_row(row, candidate, Sparsification::Knn(k)))
        .collect::<Vec<_>>();
    let (rows, columns, distances) = flatten_rows(rows);
    SparseDistances::new(names, rows, columns, distances)
}

struct SketchAccessoryWriter {
    names: HashMap<String, usize>,
    candidates: Vec<Vec<(usize, f64)>>,
    k: usize,
    partial: String,
}

impl SketchAccessoryWriter {
    fn new(names: Vec<String>, k: usize) -> Self {
        let count = names.len();
        Self {
            names: names
                .into_iter()
                .enumerate()
                .map(|(index, name)| (name, index))
                .collect(),
            candidates: vec![Vec::new(); count],
            k,
            partial: String::new(),
        }
    }

    fn add(&mut self, row: usize, column: usize, distance: f64) {
        let candidates = &mut self.candidates[row];
        candidates.push((column, distance));
        if candidates.len() > self.k {
            let (max_index, _) = candidates
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| {
                    left.1
                        .total_cmp(&right.1)
                        .then_with(|| left.0.cmp(&right.0))
                })
                .expect("candidate list is non-empty");
            candidates.swap_remove(max_index);
        }
    }

    fn parse_line(&mut self, line: &str) -> io::Result<()> {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sketchlib accessory distance line has the wrong number of fields",
            ));
        }
        let row = *self.names.get(fields[0]).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "unknown sketch sample name")
        })?;
        let column = *self.names.get(fields[1]).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "unknown sketch sample name")
        })?;
        let distance = fields[3].parse::<f64>().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid sketch distance: {error}"),
            )
        })?;
        self.add(row, column, distance);
        self.add(column, row, distance);
        Ok(())
    }

    fn finish(mut self) -> Vec<Vec<(usize, f64)>> {
        if !self.partial.is_empty() {
            let line = std::mem::take(&mut self.partial);
            self.parse_line(&line)
                .expect("sketchlib stream ended with an invalid distance line");
        }
        self.candidates
    }
}

impl Write for SketchAccessoryWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.partial.push_str(&String::from_utf8_lossy(buffer));
        while let Some(end) = self.partial.find('\n') {
            let line = self.partial[..end].to_string();
            self.partial.drain(..=end);
            self.parse_line(line.trim_end_matches('\r'))?;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
