use secsgml::{parse_sgml_into_memory, parse_sgml_submission, MetadataValue};
use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <path_to_sgml_file> <output_directory>", args[0]);
        std::process::exit(1);
    }

    let filepath = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);

    println!("Parsing SGML file: {}", filepath.display());
    println!("Output directory: {}", output_dir.display());

    // Parse and write to disk
    match parse_sgml_submission(None, Some(filepath), output_dir) {
        Ok(()) => {
            println!(
                "Successfully wrote SGML submission to {}",
                output_dir.display()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("Error parsing SGML: {}", e);
            Err(e.into())
        }
    }
}
