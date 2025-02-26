# secsgml-rust

SEC SGML Parser implemented in Rust.

commands:
cargo run --release --bin benchmark_sgml ../sgml_samples ../benchmark_results.txt
cargo run --release -- ../296920000049.sgml ../output

Neat: We appear to have sped up from 3s (python) to 1.7s

Let's setup bindings now