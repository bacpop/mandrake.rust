# mandrake.rust
rust port of the mandrake (stochastic cluster embedding) algorithm

This is targeted mainly at producing WASM code, and not all features are reproduced.
We still recommend using the [python/C++ version](https://github.com/bacpop/mandrake).

## Plotting an embedding

The Python plotting CLI reads `<prefix>.embedding.txt` and
`<prefix>.names.txt`, then writes an interactive HTML plot, a PDF density plot,
and a static PNG plot using the same prefix:

```sh
python python/plot.py <prefix> --labels labels.tsv
```

`labels.tsv` must be an unheadered two-column tab-separated file. The first
column is a sample name and the second is its plotting label; every sample name
in `<prefix>.names.txt` must occur exactly once.

Alternatively, generate labels with HDBSCAN:

```sh
python python/plot.py <prefix> --hdbscan
```

The HDBSCAN mode also writes `<prefix>.embedding_hdbscan_clusters.csv`.

## Browser tool

The first browser interface lives in `www/` and follows the worker-driven Vue
layout used by [Sparrowhawk](https://github.com/bacpop/sparrowhawk). It accepts
plain FASTA/FASTQ alignments or Roary-style accessory tables, runs the
feature-free Rust wasm core locally, plots the final embedding, and downloads
the embedding and names files. The page accepts one input by click or
drag-and-drop and detects alignment (`.fa`, `.fasta`, `.fq`, `.fastq`, and
related FASTA/FASTQ suffixes) versus accessory (`.rtab`/`.tsv`) from the file
name. Distance construction and optimization each have their own progress
bar; the Plotly WebGL view updates with the latest embedding and supports
hover, zoom, and pan. An optional labels file uses the same unheadered
`sample-name<TAB>label` format as the Python plotting CLI and must cover every
sample exactly once.

```sh
cd www
npm install
npm run serve
```

The browser build requires the Rust `wasm32-unknown-unknown` target and
`wasm-pack`. Sketch databases, sketch generation, HDBSCAN labelling, and a
deterministic browser coordinate oracle are planned for a later phase.

## Citation

See: https://royalsocietypublishing.org/doi/10.1098/rstb.2021.0237
