# mandrake.rust
rust port of the mandrake (stochastic cluster embedding) algorithm

## macOS visualization build

The default build embeds Python for the `mandrake plot` visualization command.
On arm64 macOS, use the supplied Python 3.12 mamba environment so PyO3 links
against the matching `libpython` and the plotting dependencies are available:

```sh
mamba env create -f environment.yml
mamba run -n mandrake.rust_py312 cargo build
mamba run -n mandrake.rust_py312 cargo test
```

An activated environment can be used instead:

```sh
mamba activate mandrake.rust_py312
cargo build
cargo test
```

The executable is intended to run with that environment available. A
Python-free build remains available with `cargo build --no-default-features`.
