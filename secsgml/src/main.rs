use sgml_parser::{extract_sgml_to_directory, SgmlParserError};
use std::env;
use std::process;

fn main() -> Result<(), SgmlParserError> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input_filepath> <output_directory>", args[0]);
        process::exit(1);
    }

    let filepath = &args[1];
    let output_dir = &args[2];

    extract_sgml_to_directory(None, Some(filepath), output_dir)?;

    println!(
        "Successfully processed {} and saved results to {}",
        filepath, output_dir
    );

    Ok(())
}
