use sgml_parser::{determine_file_extension, parse_sgml_submission_into_memory, serde_json::Value};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err("Usage: sgml_parser <sgml_file> [output_dir]".to_string());
    }

    let filepath = Path::new(&args[1]);
    let output_dir = args.get(2).map(Path::new).unwrap_or(Path::new("."));

    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let (metadata, documents) = parse_sgml_submission_into_memory(None, Some(filepath))?;

    // Write metadata to JSON file
    let metadata_path = output_dir.join("metadata.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

    File::create(&metadata_path)
        .map_err(|e| format!("Failed to create metadata file: {}", e))?
        .write_all(metadata_json.as_bytes())
        .map_err(|e| format!("Failed to write metadata: {}", e))?;

    // Process documents based on available metadata
    match metadata.get("documents") {
        Some(Value::Array(docs_metadata)) => {
            // Write documents with appropriate extensions based on metadata
            for (i, (document, doc_meta)) in documents.iter().zip(docs_metadata.iter()).enumerate()
            {
                let extension = determine_file_extension(doc_meta);
                let document_path = output_dir.join(format!("document_{}.{}", i + 1, extension));

                File::create(&document_path)
                    .map_err(|e| format!("Failed to create document file: {}", e))?
                    .write_all(document)
                    .map_err(|e| format!("Failed to write document: {}", e))?;
            }
        }
        _ => {
            // Fallback for documents without metadata
            for (i, document) in documents.iter().enumerate() {
                let document_path = output_dir.join(format!("document_{}.txt", i + 1));

                File::create(&document_path)
                    .map_err(|e| format!("Failed to create document file: {}", e))?
                    .write_all(document)
                    .map_err(|e| format!("Failed to write document: {}", e))?;
            }
        }
    }

    Ok(())
}
