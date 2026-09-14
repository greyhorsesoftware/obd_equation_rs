#!/bin/bash

# build_universal.sh - Build universal/fat libraries for obd_equation_rs
#
# This script builds universal binaries for iOS and macOS platforms,
# combining multiple architectures into single fat binaries.
#
# Usage:
#   ./build_universal.sh ios        # Build iOS universal library
#   ./build_universal.sh macos      # Build macOS universal library
#   ./build_universal.sh all        # Build both iOS and macOS
#   ./build_universal.sh clean      # Clean build artifacts
#   ./build_universal.sh            # Interactive mode

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Project configuration
PROJECT_NAME="obd_equation_rs"
LIBRARY_NAME="lib${PROJECT_NAME}.a"

# Deployment target
export MACOSX_DEPLOYMENT_TARGET="15.2"

# Function to print colored output
print_status() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Function to check if target is installed
check_target() {
    local target=$1
    if ! rustup target list --installed | grep -q "^${target}$"; then
        print_error "Target '${target}' is not installed."
        print_info "Install with: rustup target add ${target}"
        exit 1
    fi
}

# Function to build for a specific target
build_target() {
    local target=$1
    local description=$2

    print_info "Building for ${description} (${target})..."

    if ! cargo build --release --target "${target}"; then
        print_error "Failed to build for ${target}"
        exit 1
    fi

    print_status "Built for ${target}"
}

# Function to create fat library
create_fat_library() {
    local output_dir=$1
    local output_file=$2
    shift 2
    local input_files=("$@")

    print_info "Creating universal library: ${output_file}"

    # Create output directory
    mkdir -p "${output_dir}"

    # Create fat library
    if lipo -create "${input_files[@]}" -output "${output_file}"; then
        print_status "Created universal library"

        # Verify the fat library
        print_info "Verifying universal library..."
        lipo -info "${output_file}"

        # Also copy to lib directory for easy access
        local lib_dir="lib/$(basename "${output_dir}")"
        local lib_file="lib/$(basename "${output_file}")"
        mkdir -p "lib"
        cp "${output_file}" "${lib_file}"
        print_status "Copied to lib directory: ${lib_file}"
    else
        print_error "Failed to create universal library"
        exit 1
    fi
}

# Function to build iOS universal library
build_ios() {
    print_info "Building iOS universal library..."
    print_info "This requires both iOS device and simulator architectures."

    # Check targets
    check_target "aarch64-apple-ios"
    check_target "x86_64-apple-ios"

    # Build targets
    build_target "aarch64-apple-ios" "iOS device"
    build_target "x86_64-apple-ios" "iOS simulator"

    # Create universal library
    local output_dir="target/universal-ios/release"
    local output_file="${output_dir}/${LIBRARY_NAME}"

    create_fat_library \
        "${output_dir}" \
        "${output_file}" \
        "target/aarch64-apple-ios/release/${LIBRARY_NAME}" \
        "target/x86_64-apple-ios/release/${LIBRARY_NAME}"

    print_status "iOS universal library ready:"
    print_info "  - target/universal-ios/release/libobd_equation_rs.a"
    print_info "  - lib/universal-ios/libobd_equation_rs.a"
}

# Function to build macOS library (Apple Silicon only)
build_macos() {
    print_info "Building macOS library (Apple Silicon)..."

    # Check target
    check_target "aarch64-apple-darwin"

    # Build target
    build_target "aarch64-apple-darwin" "macOS Apple Silicon"

    # Copy library to lib directory
    mkdir -p "lib"
    cp "target/aarch64-apple-darwin/release/${LIBRARY_NAME}" "lib/"
    print_status "Copied to: lib/${LIBRARY_NAME}"

    print_status "macOS library ready:"
    print_info "  - target/aarch64-apple-darwin/release/${LIBRARY_NAME}"
    print_info "  - lib/${LIBRARY_NAME}"
}

# Function to clean build artifacts
clean() {
    print_info "Cleaning build artifacts..."

    # Remove target directories for cross-compilation targets
    rm -rf target/aarch64-apple-ios
    rm -rf target/x86_64-apple-ios
    rm -rf target/aarch64-apple-darwin
    rm -rf target/universal-ios

    # Remove lib directory with universal binaries
    rm -rf lib

    # Also clean regular debug/release builds
    cargo clean

    print_status "Cleaned all build artifacts"
}

# Function to show usage
usage() {
    echo "Usage: $0 [ios|macos|all|clean]"
    echo ""
    echo "Commands:"
    echo "  ios     Build iOS universal library (device + simulator)"
    echo "  macos   Build macOS universal library (Intel + Apple Silicon)"
    echo "  all     Build both iOS and macOS universal libraries"
    echo "  clean   Remove all build artifacts"
    echo ""
    echo "If no command is specified, interactive mode will be used."
    echo ""
    echo "Prerequisites:"
    echo "  - Install iOS targets: rustup target add aarch64-apple-ios x86_64-apple-ios"
    echo "  - Install macOS target: rustup target add aarch64-apple-darwin"
    echo "  - Xcode command line tools must be installed"
}

# Interactive mode
interactive() {
    echo "Universal Library Builder for ${PROJECT_NAME}"
    echo "========================================"
    echo ""
    echo "Available options:"
    echo "1) Build iOS universal library"
    echo "2) Build macOS universal library"
    echo "3) Build all universal libraries"
    echo "4) Clean build artifacts"
    echo "5) Exit"
    echo ""

    while true; do
        read -p "Choose an option (1-5): " choice
        case $choice in
            1) build_ios; break ;;
            2) build_macos; break ;;
            3) build_ios && build_macos; break ;;
            4) clean; break ;;
            5) exit 0 ;;
            *) print_warning "Invalid option. Please choose 1-5." ;;
        esac
    done
}

# Main script logic
main() {
    local command=$1

    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]]; then
        print_error "Please run this script from the project root directory (where Cargo.toml is located)"
        exit 1
    fi

    # Check if lipo is available (macOS development tool)
    if ! command -v lipo &> /dev/null; then
        print_error "lipo command not found. Please install Xcode command line tools."
        print_info "Run: xcode-select --install"
        exit 1
    fi

    case $command in
        "ios")
            build_ios
            ;;
        "macos")
            build_macos
            ;;
        "all")
            build_ios
            echo ""
            build_macos
            ;;
        "clean")
            clean
            ;;
        "")
            interactive
            ;;
        *)
            usage
            exit 1
            ;;
    esac

    echo ""
    print_status "Build script completed successfully!"
}

# Run main function with all arguments
main "$@"
