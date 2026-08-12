use anyhow::{Context, Result, anyhow, bail};
use roaring::RoaringBitmap;
#[cfg(feature = "native-inputs")]
use std::fs::File;
use std::io::Read;
#[cfg(feature = "native-inputs")]
use std::path::Path;

use super::{DistanceOptions, SparseDistances, build_sparse_distances, validate_distance_options};

/// Calculate Jaccard distances from an already-decompressed binary Roary-style
/// `.Rtab` reader.
pub fn accessory_distances_from_reader<R: Read>(
    reader: R,
    options: &DistanceOptions,
) -> Result<SparseDistances> {
    validate_distance_options(options)?;
    let (names, profiles) = read_accessory_table(reader)?;
    build_sparse_distances(names, options, move |left, right| {
        jaccard_distance(&profiles[left], &profiles[right])
    })
}

/// Calculate Jaccard distances from a native `.Rtab` path.
#[cfg(feature = "native-inputs")]
pub fn accessory_distances<P: AsRef<Path>>(
    path: P,
    options: &DistanceOptions,
) -> Result<SparseDistances> {
    let path = path.as_ref();
    let file =
        File::open(path).with_context(|| format!("opening accessory table {}", path.display()))?;
    if path.extension().is_some_and(|extension| extension == "bz2") {
        accessory_distances_from_reader(bzip2::read::BzDecoder::new(file), options)
    } else {
        accessory_distances_from_reader(file, options)
    }
}

fn read_accessory_table<R: Read>(reader: R) -> Result<(Vec<String>, Vec<RoaringBitmap>)> {
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
    let mut profiles = vec![RoaringBitmap::new(); names.len()];
    for (gene, record) in csv.records().enumerate() {
        let record = record.context("reading accessory table row")?;
        if record.len() != names.len() + 1 {
            bail!("accessory table row has the wrong number of columns");
        }
        for (column, value) in record.iter().skip(1).enumerate() {
            let value = match value {
                "0" => false,
                "1" => true,
                _ => bail!("accessory values must be binary 0/1"),
            };
            if value {
                profiles[column].insert(gene as u32);
            }
        }
    }
    Ok((names, profiles))
}

fn jaccard_distance(left: &RoaringBitmap, right: &RoaringBitmap) -> f64 {
    let union = left.union_len(right);
    if union == 0 {
        0.0
    } else {
        1.0 - left.intersection_len(right) as f64 / union as f64
    }
}
