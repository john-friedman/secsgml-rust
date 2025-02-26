# __init__.py
from typing import Dict, List, Any, Optional, Iterator, Callable, Tuple
import os

try:
    # Import the optimized Rust functions and classes
    from ._rust_sgml import (
        create_parser, 
        parse_sgml_submission,
        extract_sgml_to_directory,
        benchmark_directory,
        SgmlParser as _RustSgmlParser,
        SgmlDocument as _RustSgmlDocument
    )
except ImportError:
    raise ImportError("Failed to import Rust SGML parser module.")

# Provide the original function for backward compatibility
def parse_sgml_submission_into_memory(filepath: Optional[str] = None,
                                     content: Optional[str] = None) -> Tuple[Dict, List]:
    """
    Parse SGML submission and return metadata and documents.
    
    This uses the optimized Rust implementation internally.
    
    Args:
        filepath: Path to SGML file
        content: String content of SGML
        
    Returns:
        Tuple of (metadata_dict, document_contents_list)
    """
    content_str = ""
    if content is not None:
        content_str = content
        
    return parse_sgml_submission(content_str, filepath)

# New streaming API
def parse_sgml_submission_streaming(filepath: Optional[str] = None,
                                   content: Optional[str] = None) -> "SgmlParser":
    """
    Create a streaming parser for efficient document processing.
    
    This provides better performance for large files.
    
    Args:
        filepath: Path to SGML file
        content: String content of SGML
        
    Returns:
        SgmlParser object
    """
    return SgmlParser(filepath=filepath, content=content)

# Python wrapper for Rust SgmlParser
class SgmlParser:
    """
    Streaming parser for SGML documents.
    """
    
    def __init__(self, filepath: Optional[str] = None, content: Optional[str] = None):
        """Initialize the parser with either file path or content."""
        if filepath is None and content is None:
            raise ValueError("Either filepath or content must be provided")
            
        content_str = ""
        if content is not None:
            content_str = content
            
        self._parser = create_parser(content_str, filepath)
        
    def __iter__(self) -> Iterator["SgmlDocument"]:
        """Allow iteration over documents."""
        return self
        
    def __next__(self) -> "SgmlDocument":
        """Get next document."""
        try:
            return self._parser.__next__()
        except StopIteration:
            raise StopIteration("No more documents")
    
    def get_metadata(self) -> Dict[str, Any]:
        """Get header metadata."""
        return self._parser.get_metadata()
    
    def document_count(self) -> int:
        """Get total number of documents."""
        return self._parser.document_count()
    
    def get_document(self, idx: int) -> "SgmlDocument":
        """Get document at specific index."""
        return self._parser.get_document(idx)
    
    def process_batch(self, start: int, count: int, 
                      callback: Callable[["SgmlDocument"], None]) -> None:
        """Process multiple documents with a callback function."""
        self._parser.process_batch(start, count, callback)