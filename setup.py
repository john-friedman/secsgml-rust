from setuptools import setup
from setuptools_rust import Binding, RustExtension
import os
import sys

print(f"Python version: {sys.version}")
print(f"Current directory: {os.getcwd()}")
print(f"Directory listing: {os.listdir('.')}")

try:
    # Check if Cargo.toml exists
    if os.path.exists('Cargo.toml'):
        print("Cargo.toml found")
    else:
        print("Cargo.toml not found in current directory!")
    
    # Simplified setup for debugging
    setup(
        name="secsgml2",
        version="0.1.0",
        packages=["secsgml2"],
        rust_extensions=[
            RustExtension(
                "secsgml2._rust_sgml",
                binding=Binding.PyO3,
                features=["python"],
                debug=True,  # Enable debug output
            )
        ],
        zip_safe=False,
    )
except Exception as e:
    print(f"Error during setup: {e}")
    raise