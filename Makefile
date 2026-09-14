# Makefile for obd_equation_rs Rust library with FFI support

# Configuration
CARGO = cargo

# Default target
.PHONY: all
all: build

# Build the Rust library with FFI support
.PHONY: build
build:
	@echo "Building Rust library with FFI support..."
	cd equation_rs && $(CARGO) build
	@echo "Library built successfully"

# Build release version
.PHONY: release
release:
	@echo "Building release version..."
	cd equation_rs && $(CARGO) build --release
	@echo "Release build completed"

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	cd equation_rs && $(CARGO) clean
	rm -f *.so *.dylib *.dll
	@echo "Clean completed"

# Run Rust tests
.PHONY: test
test:
	@echo "Running Rust unit tests..."
	cd equation_rs && $(CARGO) test
	@echo "Rust tests completed"

# Debug target to check library
.PHONY: check-lib
check-lib:
	@echo "Checking debug library..."
	ls -la equation_rs/target/debug/libobd_equation_rs.*
	@echo "Debug library check completed"

# Debug target to check release library
.PHONY: check-lib-release
check-lib-release:
	@echo "Checking release library..."
	ls -la equation_rs/target/release/libobd_equation_rs.*
	@echo "Release library check completed"

# Show help
.PHONY: help
help:
	@echo "Available targets:"
	@echo "  all               - Build the Rust library (default)"
	@echo "  build             - Build the Rust library (debug)"
	@echo "  release           - Build the Rust library (release)"
	@echo "  test              - Run Rust unit tests"
	@echo "  clean             - Clean all build artifacts"
	@echo "  check-lib         - Check debug library files"
	@echo "  check-lib-release - Check release library files"
	@echo "  help              - Show this help message"
