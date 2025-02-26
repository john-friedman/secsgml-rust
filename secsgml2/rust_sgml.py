"""Python bindings for the Rust SGML parser library."""
import os
from . import _rust_sgml

class SGMLParserError(Exception):
    """Exception raised for SGML parsing errors."""
    pass

def parse_sgml_submission(content="", filepath=None):
    if not content and not filepath:
        raise ValueError("Either content or filepath must be provided")
    
    try:
        return _rust_sgml.parse_sgml_submission(content, filepath)
    except Exception as e:
        raise SGMLParserError(f"Failed to parse SGML: {str(e)}")

def extract_sgml_to_directory(content="", filepath=None, output_dir="./output"):
    if not content and not filepath:
        raise ValueError("Either content or filepath must be provided")
    
    os.makedirs(output_dir, exist_ok=True)
    
    try:
        _rust_sgml.extract_sgml_to_directory(content, filepath, output_dir)
    except Exception as e:
        raise SGMLParserError(f"Failed to extract SGML: {str(e)}")

def benchmark_directory(dir_path, output_file):
    try:
        _rust_sgml.benchmark_directory(dir_path, output_file)
    except Exception as e:
        raise SGMLParserError(f"Benchmark failed: {str(e)}")