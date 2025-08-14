# Abitur Analyzer

A Rust application for analyzing admission chances to medical programs based on HTML admission lists.

## Features

- **HTML Scraping**: Extracts applicant data from HTML admission lists
- **Two-Stage Analysis**: Implements the real admission process for both funding types:
  - **Budget Funding Analysis**: Analyzes "Бюджетное финансирование" programs first
  - **Commercial Funding Analysis**: Analyzes "Коммерческое финансирование" programs considering budget exclusions
- **Real Admission Logic**: Implements the actual school admission process:
  - Filters eager applicants (those with original documents OR consent)
  - Processes programs by popularity (most competitive first)
  - Simulates exclusion of admitted students from less popular programs
  - Budget funding has priority - commercial funding only gets remaining candidates
- **Targeted Analysis**: Analyzes specific applicant's chances for programs of interest
- **Multiple Output Formats**: Generates CSV, text reports, and console summaries
- **Separated Results**: Creates separate output directories for budget (`output/budget`) and commercial (`output/commercial`) funding results

## Programs of Interest

Currently configured to analyze:
- "ОП СПО Лечебное дело" (Medical Practice)
- "ОП СПО Фармация" (Pharmacy)

## Funding Types

The analyzer supports two funding types:
- **Бюджетное финансирование** (Budget funding) - Always analyzed first
- **Коммерческое финансирование** (Commercial funding) - Only analyzed if included in configuration

## Analysis Logic

1. **Budget Funding Analysis**:
   - Analyzes only budget-funded lists
   - Identifies most popular programs
   - Simulates admission process excluding admitted students from less popular programs
   - Results saved to `output/budget/`

2. **Commercial Funding Analysis** (if configured):
   - Analyzes only commercial-funded lists
   - Considers students already admitted to budget funding as excluded
   - Simulates admission process for remaining candidates
   - Results saved to `output/commercial/`

## Installation

1. Ensure you have Rust installed: https://rustup.rs/
2. Clone or navigate to this project directory
3. Build the project:
```bash
cargo build --release
```

## Usage

### Basic Usage
```bash
cargo run -- --snils "C25-00946"
```

### Advanced Usage
```bash
cargo run -- --snils "C25-00946" --data-dir "data-source" --output-dir "output"
```

### Command Line Options
- `--snils/-s`: Target applicant's SNILS (required)
- `--data-dir/-d`: Directory containing HTML files (default: "data-source")
- `--output-dir/-o`: Directory for output files (default: "output")

## Input Data

Place HTML files containing admission lists in the `data-source` directory. The scraper expects HTML tables with the following structure:
- Program information (name, funding source, study form, available places)
- Applicant data table with columns for SNILS, priority, consent, document type, average score, etc.

## Output Files

The application generates several output files:

### 1. `all_applicants.csv`
Complete dataset in CSV format with all extracted applicant information.

### 2. `program_popularity.txt`
Analysis of program competitiveness including:
- Applications per available place
- Average score of top candidates
- Total number of eager applicants

### 3. `chance_analysis.txt`
Personalized analysis for the target applicant including:
- Programs likely to admit the applicant
- Programs with low admission chances
- Specific recommendations

### 4. Console Output
Real-time summary showing:
- Program popularity ranking
- Target applicant results
- Final recommendation

## Algorithm

The admission simulation follows these steps:

1. **Filter Programs**: Only analyze budget-funded programs
2. **Calculate Popularity**: 
   - Applications per place ratio
   - Average score of top third candidates
3. **Sort by Competitiveness**: Most popular programs processed first
4. **Filter Eager Applicants**: Only those with original documents OR consent
5. **Simulate Admissions**: 
   - Admit top applicants to most popular programs first
   - Exclude admitted students from consideration for less popular programs
6. **Analyze Target**: Check if target SNILS appears in final admission lists

## Example Output

```
📊 SUMMARY
==========

📈 Program Popularity (most to least competitive):
   1. ОП СПО Лечебное дело - 2.5 applicants per place (avg score: 4.8)
   2. ОП СПО Фармация - 1.8 applicants per place (avg score: 4.6)

🎯 Target Applicant Results:
   ✅ Likely admitted to: ОП СПО Фармация
   ❌ Unlikely admitted to: ОП СПО Лечебное дело

💡 Поздравляем! Высокие шансы на поступление на программу 'ОП СПО Фармация'.
Рекомендуется подтвердить намерения подачей согласия на зачисление.
```

## Customization

To analyze different programs or change the funding type, modify the constants in `src/analyzer.rs`:

```rust
const PROGRAMS_OF_INTEREST: &[&str] = &[
    "Your Program Name 1",
    "Your Program Name 2",
];

const TARGET_FUNDING: &str = "Your Funding Type";
```

## Dependencies

- `scraper`: HTML parsing with CSS selectors
- `csv`: CSV file generation
- `serde`: Data serialization
- `clap`: Command-line argument parsing
- `anyhow`: Error handling
- `regex`: Regular expressions for data extraction
- `tokio`: Async runtime
