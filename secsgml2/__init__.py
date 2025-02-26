"""SGML Parser for SEC filings - Python bindings for the Rust SGML parser library."""

from .rust_sgml import (
    parse_sgml_submission,
    extract_sgml_to_directory,
    benchmark_directory,
    SGMLParserError
)

__all__ = [
    'parse_sgml_submission',
    'extract_sgml_to_directory',
    'benchmark_directory',
    'SGMLParserError'
]