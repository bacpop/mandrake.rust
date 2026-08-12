use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Parser};
use mandrake::{
    DistanceOptions, EmbeddingInput, EmbeddingOperation, SketchOptions, SparseDistances,
    Sparsification, WtsneOptions, accessory_distances, pair_snp_distances, sketch_distances,
    sketch_distances_from_fasta_list,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "mandrake",
    about = "Embed sparse genomic distances with Mandrake"
)]
#[command(group(
    ArgGroup::new("input")
        .required(true)
        .args(["alignment", "accessory", "sketches"])
))]
#[command(group(
    ArgGroup::new("sparsification")
        .required(true)
        .args(["knn", "threshold"])
))]
struct Args {
    /// Output prefix for embedding and names files.
    #[arg(short, long, required = true, value_name = "PREFIX")]
    output: PathBuf,
    /// Multiple-sequence FASTA alignment (plain or compressed).
    #[arg(long, value_name = "FILE")]
    alignment: Option<PathBuf>,
    /// Binary Roary-style accessory table (plain or .bz2).
    #[arg(long, value_name = "FILE")]
    accessory: Option<PathBuf>,
    /// Sketch prefix (.skm/.skd) or a text file containing one FASTA path per line.
    #[arg(long, value_name = "PREFIX_OR_LIST")]
    sketches: Option<PathBuf>,
    /// Number of neighbours to retain per sample
    #[arg(short, long, visible_alias = "kNN", value_name = "N")]
    knn: Option<usize>,
    /// Strict normalized distance threshold (not supported for sketches).
    #[arg(long, value_name = "DISTANCE")]
    threshold: Option<f64>,
    /// Conditional-probability perplexity; non-positive values use raw similarities.
    #[arg(long, default_value_t = 30.0)]
    perplexity: f64,
    /// Target number of stochastic update attempts.
    #[arg(long, default_value_t = 1_000_000)]
    max_updates: usize,
    /// Repulsion samples per update attempt.
    #[arg(long, default_value_t = 5)]
    repulsion_samples: usize,
    /// Initial learning rate.
    #[arg(long, default_value_t = 1.0)]
    learning_rate: f64,
    /// Apply initial attraction exaggeration.
    #[arg(long)]
    initial_exaggeration: bool,
    /// Number of native optimisation threads.
    #[arg(long, default_value_t = 1)]
    threads: usize,
    /// Disable the progress bar.
    #[arg(long, visible_alias = "no-progress")]
    quiet: bool,
    /// K-mer size used when --sketches points to a FASTA list.
    #[arg(long, default_value_t = 21)]
    sketch_kmer: usize,
    /// Sketch bins used when --sketches points to a FASTA list.
    #[arg(long, default_value_t = 1_000)]
    sketch_size: u64,
}

const CLI_ADVANCE_CHUNK: usize = 1_000;

pub fn run() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let sparsification = parse_sparsification(&args)?;
    let distance_options = DistanceOptions {
        sparsification,
        threads: args.threads,
        quiet: args.quiet,
    };
    let distances = build_distances(&args, &distance_options)?;
    let (names, rows, columns, values) = distances.into_parts();
    let input = EmbeddingInput::new(rows, columns, values, names.len(), None)?;
    let options = WtsneOptions {
        perplexity: args.perplexity,
        max_updates: args.max_updates,
        repulsion_samples: args.repulsion_samples,
        learning_rate: args.learning_rate,
        initial_exaggeration: args.initial_exaggeration,
        threads: args.threads,
        quiet: args.quiet,
    };
    let mut operation = EmbeddingOperation::new(input, &options).context("running wtsne")?;
    loop {
        let status = operation.advance(CLI_ADVANCE_CHUNK);
        if status.is_complete() {
            break;
        }
    }
    write_outputs(&args.output, &names, operation.embedding())?;
    Ok(())
}

fn parse_sparsification(args: &Args) -> Result<Sparsification> {
    match (args.knn, args.threshold) {
        (Some(k), None) if k > 0 => Ok(Sparsification::Knn(k)),
        (None, Some(threshold)) if threshold.is_finite() && threshold > 0.0 && threshold <= 1.0 => {
            Ok(Sparsification::Threshold(threshold))
        }
        (Some(_), None) => bail!("--knn must be greater than zero"),
        (None, Some(_)) => bail!("--threshold must be finite and in (0, 1]"),
        _ => bail!("specify exactly one of --knn or --threshold"),
    }
}

fn build_distances(args: &Args, options: &DistanceOptions) -> Result<SparseDistances> {
    match (&args.alignment, &args.accessory, &args.sketches) {
        (Some(path), None, None) => pair_snp_distances(path, options),
        (None, Some(path), None) => accessory_distances(path, options),
        (None, None, Some(path)) => {
            if matches!(options.sparsification, Sparsification::Threshold(_)) {
                bail!("threshold sparsification is not supported for sketch inputs; use --knn")
            }
            let sketch_options = SketchOptions {
                kmer_sizes: vec![args.sketch_kmer],
                sketch_size: args.sketch_size,
            };
            if sketch_prefix_exists(path) {
                sketch_distances(path, options)
            } else {
                let files = read_fasta_list(path)?;
                sketch_distances_from_fasta_list(&files, options, &sketch_options)
            }
        }
        _ => bail!("specify exactly one input source"),
    }
}

fn sketch_prefix_exists(path: &Path) -> bool {
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
        prefix = PathBuf::from(value);
    }
    let mut metadata = prefix.clone();
    metadata.set_extension("skm");
    let mut data = prefix;
    data.set_extension("skd");
    metadata.is_file() && data.is_file()
}

fn read_fasta_list(path: &Path) -> Result<Vec<PathBuf>> {
    let file =
        File::open(path).with_context(|| format!("opening FASTA list {}", path.display()))?;
    let mut files = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.context("reading FASTA list")?;
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            files.push(PathBuf::from(line));
        }
    }
    if files.len() < 2 {
        bail!("FASTA list must contain at least two paths");
    }
    Ok(files)
}

fn write_outputs(output: &Path, names: &[String], embedding: &ndarray::Array2<f64>) -> Result<()> {
    let embedding_path = PathBuf::from(format!("{}.embedding.txt", output.display()));
    let names_path = PathBuf::from(format!("{}.names.txt", output.display()));
    let mut embedding_file = File::create(&embedding_path)
        .with_context(|| format!("creating {}", embedding_path.display()))?;
    for row in embedding.rows() {
        writeln!(embedding_file, "{:.17e}\t{:.17e}", row[0], row[1])?;
    }
    let mut names_file =
        File::create(&names_path).with_context(|| format!("creating {}", names_path.display()))?;
    for name in names {
        writeln!(names_file, "{name}")?;
    }
    Ok(())
}
