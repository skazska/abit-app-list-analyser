#!/bin/bash

# Build script for abitur-analyzer
# Creates optimized binaries for distribution

set -e

echo "🚀 Building abitur-analyzer for multiple platforms..."

# Create builds directory
mkdir -p builds

# Function to get file size in human readable format
get_size() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        stat -f%z "$1" | numfmt --to=iec-i 2>/dev/null || ls -lh "$1" | awk '{print $5}'
    else
        stat --printf="%s" "$1" 2>/dev/null | numfmt --to=iec-i 2>/dev/null || ls -lh "$1" | awk '{print $5}'
    fi
}

# Build for current platform (Linux x64)
echo "Building for current platform (Linux x64)..."
cargo build --release --quiet 2>/dev/null || cargo build --release
cp target/release/abitur-analyzer builds/abitur-analyzer-linux-x64
echo "✅ Successfully built for Linux x64"
echo "   Size: $(get_size builds/abitur-analyzer-linux-x64)"

# Try to build for Windows (requires mingw-w64)
if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "Building for Windows x64..."
    rustup target add x86_64-pc-windows-gnu >/dev/null 2>&1 || true
    
    # Set up cross-compilation for Windows
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
    export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
    export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
    
    if cargo build --release --target x86_64-pc-windows-gnu --quiet 2>/dev/null; then
        cp target/x86_64-pc-windows-gnu/release/abitur-analyzer.exe builds/abitur-analyzer-windows-x64.exe
        echo "✅ Successfully built for Windows x64"
        echo "   Size: $(get_size builds/abitur-analyzer-windows-x64.exe)"
    else
        echo "⚠️  Windows build failed (likely OpenSSL dependency issues)"
        echo "   Try installing: sudo apt-get install gcc-mingw-w64"
    fi
else
    echo "⚠️  mingw-w64 not found. Skipping Windows build."
    echo "   To enable Windows builds, install: sudo apt-get install gcc-mingw-w64"
fi

# Check if running on macOS for native macOS build
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Building for macOS x64..."
    rustup target add x86_64-apple-darwin >/dev/null 2>&1 || true
    if cargo build --release --target x86_64-apple-darwin --quiet 2>/dev/null; then
        cp target/x86_64-apple-darwin/release/abitur-analyzer builds/abitur-analyzer-macos-x64
        echo "✅ Successfully built for macOS x64"
        echo "   Size: $(get_size builds/abitur-analyzer-macos-x64)"
    else
        echo "⚠️  macOS build failed"
    fi
    
    # Try Apple Silicon build
    echo "Building for macOS ARM64 (Apple Silicon)..."
    rustup target add aarch64-apple-darwin >/dev/null 2>&1 || true
    if cargo build --release --target aarch64-apple-darwin --quiet 2>/dev/null; then
        cp target/aarch64-apple-darwin/release/abitur-analyzer builds/abitur-analyzer-macos-arm64
        echo "✅ Successfully built for macOS ARM64"
        echo "   Size: $(get_size builds/abitur-analyzer-macos-arm64)"
    else
        echo "⚠️  macOS ARM64 build failed"
    fi
else
    echo "⚠️  Not on macOS. Skipping macOS builds."
    echo "   Cross-compilation for macOS requires macOS SDK."
fi

echo ""
echo "🎉 Build Summary:"
echo "=================="
echo "Available binaries in builds/:"
for file in builds/*; do
    if [ -f "$file" ]; then
        filename=$(basename "$file")
        size=$(get_size "$file")
        echo "   $filename ($size)"
    fi
done

echo ""
echo "💡 Usage Examples:"
echo "   Linux:   ./builds/abitur-analyzer-linux-x64 --help"
if [ -f "builds/abitur-analyzer-windows-x64.exe" ]; then
    echo "   Windows: builds\\abitur-analyzer-windows-x64.exe --help"
fi
if [ -f "builds/abitur-analyzer-macos-x64" ]; then
    echo "   macOS:   ./builds/abitur-analyzer-macos-x64 --help"
fi

echo ""
echo "📦 Binaries are ready for distribution!"
echo ""
echo "🔧 To create a GitHub release:"
echo "   1. git tag v0.1.0"
echo "   2. git push origin v0.1.0"
echo "   3. Upload binaries from builds/ directory to the release"
