use mandrake::{
    DistanceOptions, SketchOptions, Sparsification, accessory_distances,
    accessory_distances_from_reader, pair_snp_distances, pair_snp_distances_from_reader,
    sketch_distances, sketch_distances_from_fasta_list,
};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::Command;

fn temp_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mandrake-distance-test-{}-{suffix}",
        std::process::id()
    ))
}

fn distance_options(sparsification: Sparsification) -> DistanceOptions {
    DistanceOptions {
        sparsification,
        quiet: true,
        ..DistanceOptions::default()
    }
}

fn edge_set(distances: &mandrake::SparseDistances) -> BTreeSet<(u64, u64)> {
    distances
        .rows()
        .iter()
        .copied()
        .zip(distances.columns().iter().copied())
        .collect()
}

#[test]
fn pair_snp_distances_are_normalized_and_exclude_self_edges() {
    let path = temp_path("alignment.fasta.bz2");
    let file = File::create(&path).unwrap();
    let mut encoder = bzip2::write::BzEncoder::new(file, bzip2::Compression::fast());
    write!(encoder, ">a\nACGT\n>b\nACGA\n>c\nACGN\n").unwrap();
    encoder.finish().unwrap();

    let options = distance_options(Sparsification::Knn(1));
    let distances = pair_snp_distances(&path, &options).unwrap();
    assert_eq!(distances.n_samples(), 3);
    assert!(
        distances
            .rows()
            .iter()
            .zip(distances.columns())
            .all(|(row, column)| row != column)
    );
    assert!(
        distances
            .distances()
            .iter()
            .all(|distance| (0.0..=1.0).contains(distance))
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn pair_snp_knn_is_exact_and_breaks_ties_by_column() {
    let distances = pair_snp_distances_from_reader(
        Cursor::new(b">a\nAAAA\n>b\nAAAT\n>c\nAAAC\n"),
        &distance_options(Sparsification::Knn(1)),
    )
    .unwrap();

    assert_eq!(distances.len(), 3);
    assert_eq!(
        edge_set(&distances),
        BTreeSet::from([(0, 1), (1, 0), (2, 0)])
    );
}

#[test]
fn pair_snp_zero_knn_keeps_all_non_self_edges() {
    let distances = pair_snp_distances_from_reader(
        Cursor::new(b">a\nAAAA\n>b\nAAAT\n>c\nAAAC\n"),
        &distance_options(Sparsification::Knn(0)),
    )
    .unwrap();

    assert_eq!(distances.len(), 6);
    assert!(
        distances
            .rows()
            .iter()
            .zip(distances.columns())
            .all(|(row, column)| row != column)
    );
}

#[test]
fn pair_snp_threshold_is_strict_at_the_boundary() {
    let distances = pair_snp_distances_from_reader(
        Cursor::new(b">a\nAAAA\n>b\nAAAT\n>c\nAATT\n"),
        &distance_options(Sparsification::Threshold(0.25)),
    )
    .unwrap();

    assert!(distances.is_empty());
}

#[test]
fn reader_based_distance_constructors_accept_decompressed_bytes() {
    let alignment = pair_snp_distances_from_reader(
        Cursor::new(b">a\nACGT\n>b\nACGA\n"),
        &distance_options(Sparsification::Knn(1)),
    )
    .unwrap();
    assert_eq!(alignment.n_samples(), 2);

    let accessory = accessory_distances_from_reader(
        Cursor::new(b"Gene\ta\tb\ng1\t1\t0\ng2\t1\t1\n"),
        &distance_options(Sparsification::Knn(1)),
    )
    .unwrap();
    assert_eq!(accessory.n_samples(), 2);
}

#[test]
fn accessory_distances_parse_binary_table_and_use_strict_thresholds() {
    let path = temp_path("accessory.tsv");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "Gene\ta\tb\tc").unwrap();
    writeln!(file, "g1\t1\t1\t0").unwrap();
    writeln!(file, "g2\t1\t0\t0").unwrap();
    drop(file);

    let options = distance_options(Sparsification::Threshold(0.5));
    let distances = accessory_distances(&path, &options).unwrap();
    assert_eq!(distances.names(), ["a", "b", "c"]);
    assert!(distances.distances().iter().all(|&distance| distance < 0.5));
    assert!(
        distances
            .rows()
            .iter()
            .zip(distances.columns())
            .all(|(row, column)| row != column)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn accessory_knn_is_exact_and_directed() {
    let distances = accessory_distances_from_reader(
        Cursor::new(b"Gene\ta\tb\tc\ng1\t1\t1\t1\ng2\t1\t0\t0\ng3\t0\t1\t0\ng4\t0\t0\t1\n"),
        &distance_options(Sparsification::Knn(1)),
    )
    .unwrap();

    assert_eq!(distances.len(), 3);
    assert_eq!(
        edge_set(&distances),
        BTreeSet::from([(0, 1), (1, 0), (2, 0)])
    );
}

#[test]
fn accessory_threshold_excludes_equal_distances() {
    let distances = accessory_distances_from_reader(
        Cursor::new(b"Gene\ta\tb\tc\ng1\t1\t1\t0\ng2\t1\t0\t0\n"),
        &distance_options(Sparsification::Threshold(0.5)),
    )
    .unwrap();

    assert!(distances.is_empty());
}

#[test]
fn sketch_fixture_loads_as_sparse_distances() {
    let prefix = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sketches.skm");
    let options = distance_options(Sparsification::Knn(2));
    let distances = sketch_distances(&prefix, &options).unwrap();
    assert!(distances.n_samples() > 2);
    assert_eq!(distances.len(), distances.n_samples() * 2);
    assert!(
        distances
            .distances()
            .iter()
            .all(|distance| (0.0..=1.0).contains(distance))
    );
}

#[test]
fn sketch_fasta_list_supports_core_distances() {
    let first = temp_path("first.fasta");
    let second = temp_path("second.fasta");
    std::fs::write(&first, b">first\nACGTACGTACGTACGTACGTACGT\n").unwrap();
    std::fs::write(&second, b">second\nACGTACGTACGTACGTACGTTCGT\n").unwrap();
    let options = SketchOptions {
        kmer_sizes: vec![3, 5],
        sketch_size: 64,
    };
    let core = sketch_distances_from_fasta_list(
        &[first.clone(), second.clone()],
        &distance_options(Sparsification::Knn(1)),
        &options,
    )
    .unwrap();
    assert_eq!(core.n_samples(), 2);
    assert_eq!(core.len(), 2);
    std::fs::remove_file(first).unwrap();
    std::fs::remove_file(second).unwrap();
}

#[test]
fn cli_writes_embedding_and_names_for_accessory_input() {
    let input = temp_path("cli-accessory.tsv");
    let output = temp_path("cli-output");
    let mut file = File::create(&input).unwrap();
    writeln!(file, "Gene\ta\tb\tc").unwrap();
    writeln!(file, "g1\t1\t1\t0").unwrap();
    writeln!(file, "g2\t1\t0\t0").unwrap();
    drop(file);

    let status = Command::new(env!("CARGO_BIN_EXE_mandrake"))
        .args([
            "--accessory",
            input.to_str().unwrap(),
            "--knn",
            "1",
            "--max-updates",
            "1",
            "--no-progress",
            "--output",
            output.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let embedding = PathBuf::from(format!("{}.embedding.txt", output.display()));
    let names = PathBuf::from(format!("{}.names.txt", output.display()));
    assert_eq!(
        std::fs::read_to_string(&embedding).unwrap().lines().count(),
        3
    );
    assert_eq!(std::fs::read_to_string(&names).unwrap().lines().count(), 3);
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(embedding).unwrap();
    std::fs::remove_file(names).unwrap();
}
