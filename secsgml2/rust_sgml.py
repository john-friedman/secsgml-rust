from typing import Dict, List, Any, Optional, Iterator, Callable, Union, BinaryIO, Tuple
import os

try:
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


class SgmlParser:
    def __init__(self, filepath: Optional[str] = None, content: Optional[str] = None):
        if filepath is None and content is None:
            raise ValueError("Either filepath or content must be provided")
            
        content_str = ""
        if content is not None:
            content_str = content
            
        self._parser = create_parser(content_str, filepath)
        
    def __iter__(self) -> Iterator["SgmlDocument"]:
        return self
        
    def __next__(self) -> "SgmlDocument":
        try:
            return self._parser.__next__()
        except StopIteration:
            raise StopIteration("No more documents")
    
    def get_metadata(self) -> Dict[str, Any]:
        return self._parser.get_metadata()
    
    def document_count(self) -> int:
        return self._parser.document_count()
    
    def get_document(self, idx: int) -> "SgmlDocument":
        return self._parser.get_document(idx)
    
    def process_batch(self, start: int, count: int, callback: Callable[["SgmlDocument"], None]) -> None:
        self._parser.process_batch(start, count, callback)


def parse_sgml_submission_streaming(filepath: Optional[str] = None, 
                                   content: Optional[str] = None) -> SgmlParser:
    return SgmlParser(filepath=filepath, content=content)


def parse_sgml_submission_into_memory(filepath: Optional[str] = None,
                                     content: Optional[str] = None) -> Tuple[Dict, List]:
    content_str = ""
    if content is not None:
        content_str = content
        
    return parse_sgml_submission(content_str, filepath)