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
plain or gzip-compressed FASTA/FASTQ alignments and Roary-style accessory
tables, runs the feature-free Rust wasm core locally, plots the final
embedding, and downloads the embedding and names files. The page accepts one
input by click or drag-and-drop and detects alignment (`.fa`, `.fasta`, `.fq`,
`.fastq`, and related FASTA/FASTQ suffixes) versus accessory (`.rtab`/`.tsv`),
with an optional `.gz` suffix, from the file name. Gzip data is read and
decompressed inside the worker as the parser consumes it. Distance
construction and optimization each have their own progress bar; the Plotly
WebGL view updates with the latest embedding and supports hover, zoom, and
pan. An optional labels file uses the same unheadered
`sample-name<TAB>label` format as the Python plotting CLI and must cover every
sample exactly once.
The `Run HDBSCAN after embedding` option applies a fixed, deterministic preset to
the final two-dimensional embedding. The result reports the number of non-noise
clusters, can switch between manual and HDBSCAN colours, renders noise separately,
and offers a `<prefix>.embedding_hdbscan_clusters.csv` download.

```sh
cd www
npm install
npm run serve
```

To run the committed Chromium browser checks, install the external Playwright
binary once and let the test runner start the dev server:

```sh
npm run playwright:install
npm run test:e2e
```

The browser build requires the Rust `wasm32-unknown-unknown` target and
`wasm-pack`. Sketch databases and sketch generation are planned for a later
phase.

The deterministic wasm HDBSCAN oracle can be run after building a Node-target
package:

```sh
wasm-pack build --target nodejs --no-default-features --out-dir /tmp/mandrake-wasm-node
node scripts/hdbscan_oracle.mjs /tmp/mandrake-wasm-node
```

## Citation

See: https://royalsocietypublishing.org/doi/10.1098/rstb.2021.0237
