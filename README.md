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
tables, runs the Rust wasm core locally, plots the final embedding, and
downloads the embedding and names files. The page accepts one regular input or
a paired sketch database by click or drag-and-drop and detects alignment (`.fa`,
`.fasta`, `.fq`,
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
The drop zone also accepts a paired current-format sketchlib database: add one
`.skm` metadata file and its matching `.skd` data file, together or separately.
These files use sketchlib's new 16-bit-bin format; legacy 14-bit databases are
rejected. Core distances are available when the database stores at least two
k-mer lengths, while Jaccard distances expose a selector for the stored k-mer.

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

The browser build requires the Rust `wasm32-unknown-unknown` target,
`wasm-pack`, and the checked-out `sketchlib.rust` submodule:

```sh
git submodule update --init --recursive
```

The deterministic wasm HDBSCAN oracle can be run after building a Node-target
package:

```sh
wasm-pack build --target nodejs --no-default-features --out-dir /tmp/mandrake-wasm-node
node scripts/hdbscan_oracle.mjs /tmp/mandrake-wasm-node
```

The paired-sketch wasm smoke can be run with a Node-target package as well:

```sh
cargo build --lib --target wasm32-unknown-unknown --no-default-features --features wasm-sketchlib
wasm-bindgen target/wasm32-unknown-unknown/debug/mandrake.wasm --target nodejs --out-dir /tmp/mandrake-wasm-sketch
node tests/sketch_wasm_smoke.mjs /tmp/mandrake-wasm-sketch
```

## Citation

See: https://royalsocietypublishing.org/doi/10.1098/rstb.2021.0237
