#!/bin/bash

# Build script for cross-platform compilation
# Generates optimized, statically-linked executables for multiple platforms

set -e

echo "🚀 Building abitur-analyzer for multiple platforms..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create build directory
mkdir -p builds

# Function to build for a specific target
build_target() {
    local target=$1
    local name=$2
    local extension=$3
    
    echo -e "${YELLOW}Building for $name ($target)...${NC}"
    
    # Check if target is installed
    if ! rustup target list --installed | grep -q "^$target\$"; then
        echo -e "${YELLOW}Installing target $target...${NC}"
        rustup target add $target
    fi
    
    # Build the target
    if cargo build --release --target $target; then
        # Copy the binary to builds directory
        cp target/$target/release/abitur-analyzer$extension builds/abitur-analyzer-$name$extension
        echo -e "${GREEN}✅ Successfully built for $name${NC}"
        
        # Show file size
        ls -lh builds/abitur-analyzer-$name$extension | awk '{print "   Size: " $5}'
    else
        echo -e "${RED}❌ Failed to build for $name${NC}"
        return 1
    fi
}

# Build for current platform (Linux)
echo -e "${YELLOW}Building for current platform (Linux x64)...${NC}"
cargo build --release
cp target/release/abitur-analyzer builds/abitur-analyzer-linux-x64
echo -e "${GREEN}✅ Successfully built for Linux x64${NC}"
ls -lh builds/abitur-analyzer-linux-x64 | awk '{print "   Size: " $5}'

# Build for Windows
if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    build_target "x86_64-pc-windows-gnu" "windows-x64" ".exe"
else
    echo -e "${YELLOW}⚠️  mingw-w64 not found. Skipping Windows build.${NC}"
    echo "   To enable Windows builds, install: sudo apt-get install gcc-mingw-w64"
fi

# Build for macOS (if on macOS or with cross-compilation tools)
if [[ "$OSTYPE" == "darwin"* ]]; then
    build_target "x86_64-apple-darwin" "macos-x64" ""
    # Also build for Apple Silicon if available
    if rustup target list --installed | grep -q "aarch64-apple-darwin"; then
        build_target "aarch64-apple-darwin" "macos-arm64" ""
    fi
else
    echo -e "${YELLOW}⚠️  Not on macOS. Skipping macOS builds.${NC}"
    echo "   Cross-compilation for macOS requires macOS SDK."
fi

# Build for ARM64 Linux
build_target "aarch64-unknown-linux-gnu" "linux-arm64" ""

echo ""
echo -e "${GREEN}🎉 Build complete!${NC}"
echo ""
echo "📦 Built binaries:"
ls -la builds/
echo ""
echo "💡 Usage:"
echo "   Linux:   ./builds/abitur-analyzer-linux-x64 --snils \"your-snils\""
echo "   Windows: builds/abitur-analyzer-windows-x64.exe --snils \"your-snils\""
echo "   macOS:   ./builds/abitur-analyzer-macos-x64 --snils \"your-snils\""
echo ""
echo "🚀 Ready for distribution!"
