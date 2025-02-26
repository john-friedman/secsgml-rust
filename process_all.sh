#!/bin/bash

# Directory containing SGML files
INPUT_DIR="../sgml_samples"
# Directory for output files
OUTPUT_DIR="output"

# Ensure output directory exists
mkdir -p $OUTPUT_DIR

# Counter for output files
counter=1

# Start timer for total processing time
start_time=$(date +%s)

# Process each SGML file
for file in "$INPUT_DIR"/*.sgml; do
    # Get just the filename
    filename=$(basename "$file")

    # Start timer for individual file
    file_start_time=$(date +%s)

    # Run your cargo command
    echo "Processing $filename to $OUTPUT_DIR/$counter"
    cargo run --release "$file" "$OUTPUT_DIR/$counter"

    # End timer for individual file
    file_end_time=$(date +%s)
    file_elapsed_time=$((file_end_time - file_start_time))
    echo "Time taken for $filename: $file_elapsed_time seconds"


    # Increment counter
    ((counter++))
done

# End timer for total processing time
end_time=$(date +%s)
elapsed_time=$((end_time - start_time))

echo "All files processed!"
echo "Total processing time: $elapsed_time seconds"