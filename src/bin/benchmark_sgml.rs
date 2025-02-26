use sgml_parser::benchmark::benchmark_directory;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <sgml_directory> <output_file>", args[0]);
        process::exit(1);
    }

    let dir_path = &args[1];
    let output_file = &args[2];

    println!("Benchmarking SGML files in {} ...", dir_path);

    if let Err(e) = benchmark_directory(dir_path, output_file) {
        eprintln!("Error: {:?}", e);
        process::exit(1);
    }

    println!("Benchmark complete! Results written to {}", output_file);
}
