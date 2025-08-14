.PHONY: all build clean test install linux windows macos help

# Default target
all: build

# Build for current platform
build:
	@echo "🚀 Building abitur-analyzer..."
	cargo build --release
	@echo "✅ Build complete! Binary: target/release/abitur-analyzer"

# Build for all platforms
cross-build:
	@echo "🌍 Building for all platforms..."
	./build.sh

# Build optimized Linux binary
linux:
	@echo "🐧 Building for Linux..."
	cargo build --release
	mkdir -p builds
	cp target/release/abitur-analyzer builds/abitur-analyzer-linux-x64
	@echo "✅ Linux build: builds/abitur-analyzer-linux-x64"

# Build Windows binary (requires mingw-w64)
windows:
	@echo "🪟 Building for Windows..."
	rustup target add x86_64-pc-windows-gnu
	cargo build --release --target x86_64-pc-windows-gnu
	mkdir -p builds
	cp target/x86_64-pc-windows-gnu/release/abitur-analyzer.exe builds/abitur-analyzer-windows-x64.exe
	@echo "✅ Windows build: builds/abitur-analyzer-windows-x64.exe"

# Build macOS binary (requires macOS or cross-compilation setup)
macos:
	@echo "🍎 Building for macOS..."
	rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	mkdir -p builds
	cp target/x86_64-apple-darwin/release/abitur-analyzer builds/abitur-analyzer-macos-x64
	@echo "✅ macOS build: builds/abitur-analyzer-macos-x64"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf builds/
	@echo "✅ Clean complete!"

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test

# Install locally
install:
	@echo "📦 Installing abitur-analyzer..."
	cargo install --path .
	@echo "✅ Installed! Use: abitur-analyzer --snils \"your-snils\""

# Show available commands
help:
	@echo "Available commands:"
	@echo "  build        - Build for current platform"
	@echo "  cross-build  - Build for all platforms using build.sh"
	@echo "  linux        - Build for Linux x64"
	@echo "  windows      - Build for Windows x64 (requires mingw-w64)"
	@echo "  macos        - Build for macOS x64"
	@echo "  clean        - Clean build artifacts"
	@echo "  test         - Run tests"
	@echo "  install      - Install locally"
	@echo "  help         - Show this help"
