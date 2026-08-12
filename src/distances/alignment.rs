use anyhow::{Context, Result, anyhow, bail};
#[cfg(feature = "native-inputs")]
use needletail::parse_fastx_file;
use needletail::{FastxReader, parse_fastx_reader};
use rayon::prelude::*;
use std::io::Read;
#[cfg(feature = "native-inputs")]
use std::path::Path;

use super::bitvecs::SampleBases;
use super::{
    DistanceOptions, SparseDistances, flatten_rows, select_alignment_row,
    validate_distance_options, with_pool,
};

/// Calculate normalized pair-SNP distances from an already-decompressed FASTA
/// or FASTQ reader.
pub fn pair_snp_distances_from_reader<R: Read + Send>(
    reader: R,
    options: &DistanceOptions,
) -> Result<SparseDistances> {
    validate_distance_options(options)?;
    let mut reader = parse_fastx_reader(reader).map_err(|error| anyhow!(error))?;
    let (names, sequences, alignment_len) = read_alignment(&mut *reader)?;
    pair_snp_distances_from_alignment(names, sequences, alignment_len, options)
}

/// Calculate normalized pair-SNP distances from a native path.
#[cfg(feature = "native-inputs")]
pub fn pair_snp_distances<P: AsRef<Path>>(
    path: P,
    options: &DistanceOptions,
) -> Result<SparseDistances> {
    let path = path.as_ref();
    let mut reader = parse_fastx_file(path)
        .map_err(|error| anyhow!(error))
        .with_context(|| format!("opening alignment {}", path.display()))?;
    validate_distance_options(options)?;
    let (names, sequences, alignment_len) = read_alignment(&mut *reader)?;
    pair_snp_distances_from_alignment(names, sequences, alignment_len, options)
}

fn pair_snp_distances_from_alignment(
    names: Vec<String>,
    sequences: Vec<SampleBases>,
    alignment_len: usize,
    options: &DistanceOptions,
) -> Result<SparseDistances> {
    let n = names.len();
    let progress = super::distance_progress(options, n);
    let rows = with_pool(options.threads, || {
        (0..n)
            .into_par_iter()
            .map(|i| {
                let mut comparisons = Vec::with_capacity(n);
                for j in 0..n {
                    let matches = sequences[i].matching_sites(&sequences[j]).len() as usize;
                    let gaps = sequences[i].either_gap_sites(&sequences[j]).len() as usize;
                    let comparable = alignment_len.saturating_sub(gaps);
                    let mismatches = comparable.saturating_sub(matches);
                    comparisons.push((j, mismatches));
                }
                progress.inc(1);
                select_alignment_row(i, comparisons, alignment_len, options.sparsification)
            })
            .collect::<Vec<_>>()
    })?;
    progress.finish(None);
    let (rows, columns, distances) = flatten_rows(rows);
    SparseDistances::new(names, rows, columns, distances)
}

fn read_alignment(reader: &mut dyn FastxReader) -> Result<(Vec<String>, Vec<SampleBases>, usize)> {
    let mut names = Vec::new();
    let mut sequences = Vec::new();
    let mut alignment_len = None;
    while let Some(record) = reader.next() {
        let record = record
            .map_err(|error| anyhow!(error))
            .context("reading alignment record")?;
        let sequence = record.seq().to_vec();
        if sequence.is_empty() {
            bail!("alignment contains an empty sequence");
        }
        if let Some(expected) = alignment_len {
            if sequence.len() != expected {
                bail!("alignment sequences have variable lengths");
            }
        } else {
            alignment_len = Some(sequence.len());
        }
        names.push(String::from_utf8_lossy(record.id()).into_owned());
        sequences.push(SampleBases::from_sequence_at(&sequence, 0));
    }
    let alignment_len = alignment_len.ok_or_else(|| anyhow!("alignment contains no sequences"))?;
    if names.len() < 2 {
        bail!("alignment must contain at least two sequences");
    }
    Ok((names, sequences, alignment_len))
}
