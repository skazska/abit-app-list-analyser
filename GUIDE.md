# Abitur Analyzer - Comprehensive Guide

> **Disclaimer**: This program was created with the assistance of GitHub Copilot AI.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Usage](#usage)
5. [Data Sources](#data-sources)
6. [Understanding the Output](#understanding-the-output)
7. [Algorithm Details](#algorithm-details)
8. [Troubleshooting](#troubleshooting)
9. [Advanced Features](#advanced-features)

## Quick Start

1. **Install Rust**: Visit https://rustup.rs/ and follow the installation instructions
2. **Clone the repository**:
   ```bash
   git clone git@github.com:skazska/abit-app-list-analyser.git
   cd abit-app-list-analyser
   ```
3. **Build the project**:
   ```bash
   cargo build --release
   ```
4. **Run with your SNILS**:
   ```bash
   cargo run -- --snils "your-snils-here"
   ```

## Installation

### Prerequisites

- **Rust**: Install from https://rustup.rs/
- **Git**: For cloning the repository

### Building

```bash
# Clone the repository
git clone git@github.com:skazska/abit-app-list-analyser.git
cd abit-app-list-analyser

# Build release version (optimized)
cargo build --release

# The executable will be at target/release/abitur-analyzer
```

### Cross-Platform Builds

Build for different platforms using the provided scripts:

#### Using build script (Linux/macOS):
```bash
# Build for all supported platforms
./build.sh

# Binaries will be in builds/ directory
ls builds/
```

#### Using Makefile:
```bash
# Build for current platform
make build

# Build for all platforms
make cross-build

# Build for specific platforms
make linux      # Linux x64
make windows    # Windows x64 (requires mingw-w64)
make macos      # macOS x64

# Clean build artifacts
make clean
```

#### Using batch file (Windows):
```cmd
# Windows users can use the batch file
build.bat
```

#### Manual cross-compilation:
```bash
# Install target platforms
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-unknown-linux-gnu

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu

# Build for macOS
cargo build --release --target x86_64-apple-darwin

# Build for ARM64 Linux
cargo build --release --target aarch64-unknown-linux-gnu
```

### Static Linking

The release builds are configured for optimal size and performance:

- **LTO (Link Time Optimization)**: Enabled for smaller binaries
- **Panic strategy**: `abort` for reduced binary size
- **Optimization level**: Maximum (`opt-level = 3`)
- **Single codegen unit**: Better optimization

### Requirements for Cross-Compilation

#### Windows builds on Linux:
```bash
# Install mingw-w64
sudo apt-get install gcc-mingw-w64

# Add Windows target
rustup target add x86_64-pc-windows-gnu
```

#### macOS builds:
- macOS builds require either macOS or cross-compilation toolchain
- On Linux, requires osxcross or similar tools (complex setup)

#### ARM64 Linux builds:
```bash
# Install cross-compilation tools
sudo apt-get install gcc-aarch64-linux-gnu

# Add ARM64 target
rustup target add aarch64-unknown-linux-gnu
```

## Configuration

### Basic Configuration

1. **Create config file** (run once to generate):
   ```bash
   cargo run
   ```

2. **Edit configuration**:
   ```bash
   nano config.toml  # or use any text editor
   ```

### Configuration Options

#### Required Settings

```toml
# Target applicant's SNILS (required)
target_snils = "15124960041"
# Accepts formats: "15124960041" or "151-249-600 41"
```

#### Optional Settings

```toml
# Data source configuration
data_source_mode = "local"  # Options: "local", "internet", "both"
data_directory = "data-source"
output_directory = "output"

# Programs to analyze (if not specified, analyzes all)
programs_of_interest = [
    "ОП СПО Лечебное дело",
    "ОП СПО Фармация",
]

# Funding types to analyze
funding_types = ["Бюджетное финансирование", "Коммерческое финансирование"]

# Internet data sources (for internet mode)
internet_urls = [
    "https://university.ru/admission/list1",
    "https://university.ru/admission/list2",
]
```

## Usage

### Command Line Interface

```bash
# Basic usage
cargo run -- --snils "C25-00946"

# Advanced usage
cargo run -- --snils "C25-00946" --data-dir "data-source" --output-dir "output"

# Using configuration file
cargo run  # Reads config.toml automatically
```

### Command Line Options

- `--snils/-s`: Target applicant's SNILS (required if not in config)
- `--data-dir/-d`: Directory containing HTML files (default: "data-source")
- `--output-dir/-o`: Directory for output files (default: "output")

## Data Sources

### Local HTML Files

Place HTML files containing admission lists in the `data-source` directory.

**Expected HTML structure**:
- Program information (name, funding source, study form, available places)
- Applicant data table with columns:
  - SNILS
  - Priority
  - Consent status
  - Document type
  - Average score
  - Subject scores

### Internet Data Sources

Configure URLs in `config.toml`:

```toml
data_source_mode = "internet"  # or "both"
internet_urls = [
    "https://your-university.edu/admission-lists/program1",
    "https://your-university.edu/admission-lists/program2",
]
```

**Features**:
- Automatic detection of `<div class="data-wrap">` sections
- 30-second timeout per URL
- Graceful error handling
- Smart fallback to full page parsing

### Hybrid Mode

Use both local files and internet sources:

```toml
data_source_mode = "both"
data_directory = "data-source"
internet_urls = ["https://..."]
```

## Understanding the Output

### Console Output

```
📊 UNIFIED PRIORITY-BASED ADMISSION ANALYSIS
==========================================

🏆 Program Popularity Ranking (by average priority):
   1. ОП СПО Лечебное дело (Бюджетное финансирование) - 2.5 eager applicants per place
   2. ОП СПО Фармация (Бюджетное финансирование) - 1.8 eager applicants per place

🎯 Target Applicant Results:
✅ Target applicant found in the data
📋 Application Results:
   • ОП СПО Фармация: ✅ Admitted
   • ОП СПО Лечебное дело: ❌ Not_Admitted (258 applicants behind)

📝 Final Recommendation:
   Congratulations! You were admitted to: ОП СПО Фармация
```

### Status Icons

- **✅ Admitted**: Successfully admitted to the program
- **🟡 Admitted_ByScore_NotByPriority**: Would qualify by score but priority not selected
- **❌ Not_Admitted**: Not admitted (shows how many applicants ahead)
- **🔮 Hypothetical**: Prediction for programs not applied to
- **🚫 Cannot determine**: Insufficient data for analysis

### Generated Files

#### 1. `all_applicants.csv`
Complete dataset with all extracted applicant information.

#### 2. `program_popularity.txt`
Program competitiveness analysis:
- Applications per available place
- Average score of top candidates
- Total number of eager applicants

#### 3. `final_cutoff_analysis.txt`
Detailed analysis for each program:
```
Program: ОП СПО Фармация
Funding: Бюджетное финансирование
Available places: 15
Target score: 3.9231
Cutoff score: 3.4615
Status: Admitted_ByScore_NotByPriority (would qualify by score but priority 3 not selected)
```

#### 4. Individual Program CSVs
- `output/programs/`: Raw data for each program
- `output/filtered_eager/`: Filtered eager applicants
- `output/admitted_lists/`: Final admission lists

## Algorithm Details

### Core Logic

The admission simulation follows real-world educational institution logic:

1. **Program Popularity Calculation**:
   - Applications per available place ratio
   - Average score of top candidates
   - Average priority of eager applicants

2. **Eager Applicant Filtering**:
   - Only those with original documents OR consent
   - Excludes casual applications

3. **Priority-Based Processing**:
   - Programs processed by popularity (most competitive first)
   - Applicants ranked by score within each program
   - Admitted students excluded from less popular programs

4. **Multi-Funding Support**:
   - Budget funding analyzed first
   - Commercial funding considers budget exclusions
   - Separate output directories maintained

### Technical Implementation

```rust
// Key components:
- Global exclusion tracking (HashSet-based SNILS normalization)
- Popularity-based processing order
- Cross-program dependencies
- Score-based predictions for hypothetical scenarios
```

## Troubleshooting

### Common Issues

**Issue**: "Target applicant not found"
- **Solution**: Verify SNILS format, check if applicant applied to analyzed programs

**Issue**: "No valid data sources found"
- **Solution**: Ensure HTML files are in correct directory, check file permissions

**Issue**: "HTTP request failed"
- **Solution**: Check internet connection, verify URLs are accessible

**Issue**: Build errors
- **Solution**: Ensure Rust is properly installed, run `cargo clean` then `cargo build`

### Debug Mode

Run with detailed logging:
```bash
RUST_LOG=debug cargo run -- --snils "your-snils"
```

## Advanced Features

### Custom Program Analysis

Modify `programs_of_interest` in config to analyze specific programs:

```toml
programs_of_interest = [
    "Your Custom Program 1",
    "Your Custom Program 2",
]
```

### Hypothetical Analysis

The system provides predictions for programs the applicant didn't apply to:
- Based on score comparisons with actual cutoffs
- Indicates likelihood of admission
- Explains reasoning behind predictions

### Real-Time Data Tracking

Use internet mode for real-time admission tracking:
- Automatic updates as admission lists change
- Perfect for monitoring during admission periods
- Combines reliability of local backup with freshness of live data

### Performance Optimization

- Async HTTP requests for internet sources
- Parallel processing of multiple data sources
- Efficient memory usage with streaming CSV processing
- Smart caching for repeated analyses

---

## Support

For issues, questions, or contributions, please visit the GitHub repository:
https://github.com/skazska/abit-app-list-analyser

## License

This project is open source. Please check the repository for license details.
