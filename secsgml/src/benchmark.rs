use crate::sgml_parser::parse_sgml_submission;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

pub fn benchmark_directory(dir_path: &str, output_file: &str) -> std::io::Result<()> {
    // Create output file
    let mut file = fs::File::create(output_file)?;

    // Write header
    writeln!(file, "filename\ttime_seconds\tstatus")?;

    // Get all files in directory
    let entries = fs::read_dir(dir_path)?;
    let mut total_time = 0.0;
    let mut file_count = 0;
    let mut success_count = 0;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip if not a file or not .sgml
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("sgml") {
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();
        println!("Processing {}...", filename);

        // Parse the file and time it
        let start = Instant::now();

        // Fix: Pass the filepath correctly as the second parameter
        let path_str = path.to_str().unwrap();
        let result = match parse_sgml_submission("", Some(path_str)) {
            Ok(_) => {
                success_count += 1;
                "success".to_string() // Convert &str to String
            }
            Err(e) => {
                eprintln!("  Error: {:?}", e);
                format!("error: {:?}", e)
            }
        };

        let duration = start.elapsed();
        let seconds = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;

        // Write result
        writeln!(file, "{}\t{:.6}\t{}", filename, seconds, result)?;

        total_time += seconds;
        file_count += 1;
        println!("  Time: {:.6} seconds, Status: {}", seconds, result);
    }

    // Write summary
    writeln!(file, "\nTotal files: {}", file_count)?;
    writeln!(file, "Successful files: {}", success_count)?;
    writeln!(file, "Total time: {:.6} seconds", total_time)?;
    writeln!(
        file,
        "Average time: {:.6} seconds",
        if file_count > 0 {
            total_time / file_count as f64
        } else {
            0.0
        }
    )?;

    Ok(())
}
