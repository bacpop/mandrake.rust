fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
    if std::env::var_os("CARGO_FEATURE_PYO3").is_some() {
        pyo3_build_config::add_libpython_rpath_link_args();
    }
}
