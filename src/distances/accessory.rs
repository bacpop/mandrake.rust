use anyhow::{Context, Result, anyhow, bail};
use rayon::prelude::*;
#[cfg(feature = "native-inputs")]
use std::fs::File;
use std::io::Read;
#[cfg(feature = "native-inputs")]
use std::path::Path;

use super::{
    SparseDistances, Sparsification, flatten_rows, jaccard_distance, select_distance_row,
    validate_sparsification,
};

/// Calculate Jaccard distances from an already-decompressed binary Roary-style
/// `.Rtab` reader.
pub fn accessory_distances_from_reader<R: Read>(
    reader: R,
    sparsification: Sparsification,
) -> Result<SparseDistances> {
    validate_sparsification(sparsification)?;
    let (names, columns) = read_accessory_table(reader)?;
    let n = names.len();
    let rows = (0..n)
        .into_par_iter()
        .map(|i| {
            let mut candidates = Vec::with_capacity(n.saturating_sub(1));
            for j in 0..n {
                if i != j {
                    candidates.push((j, jaccard_distance(&columns[i], &columns[j])));
                }
            }
            select_distance_row(i, candidates, sparsification)
        })
        .collect();
    let (rows, columns, distances) = flatten_rows(rows);
    SparseDistances::new(names, rows, columns, distances)
}

/// Calculate Jaccard distances from a native `.Rtab` path.
#[cfg(feature = "native-inputs")]
pub fn accessory_distances<P: AsRef<Path>>(
    path: P,
    sparsification: Sparsification,
) -> Result<SparseDistances> {
    let path = path.as_ref();
    let file =
        File::open(path).with_context(|| format!("opening accessory table {}", path.display()))?;
    if path.extension().is_some_and(|extension| extension == "bz2") {
        accessory_distances_from_reader(bzip2::read::BzDecoder::new(file), sparsification)
    } else {
        accessory_distances_from_reader(file, sparsification)
    }
}

fn read_accessory_table<R: Read>(reader: R) -> Result<(Vec<String>, Vec<Vec<u8>>)> {
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(reader);
    let header = csv
        .records()
        .next()
        .ok_or_else(|| anyhow!("accessory table has no header"))?
        .context("reading accessory table header")?;
    if header.len() < 3 || header.get(0) != Some("Gene") {
        bail!("accessory table must start with Gene and at least two samples");
    }
    let names = header
        .iter()
        .skip(1)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut columns = vec![Vec::new(); names.len()];
    for record in csv.records() {
        let record = record.context("reading accessory table row")?;
        if record.len() != names.len() + 1 {
            bail!("accessory table row has the wrong number of columns");
        }
        for (column, value) in record.iter().skip(1).enumerate() {
            let value = match value {
                "0" => 0,
                "1" => 1,
                _ => bail!("accessory values must be binary 0/1"),
            };
            columns[column].push(value);
        }
    }
    Ok((names, columns))
}
