# Implementation Changes: Two-Stage Funding Analysis

## Overview
The analyzer has been modified to support separate analysis for budget and commercial funding types, following the real admission process logic.

## Key Changes

### 1. Separated Analysis Process
- **Budget Analysis First**: Analyzes only "Бюджетное финансирование" programs
- **Commercial Analysis Second**: Analyzes only "Коммерческое финансирование" programs, considering budget exclusions

### 2. New Analyzer Methods
- `new_with_funding_filter()`: Creates analyzer with specific funding type filter
- `new_with_funding_filter_and_budget_exclusions()`: Creates analyzer for commercial funding with budget exclusions
- `filter_by_funding_types()`: Filters program records by funding type
- `simulate_admission_with_funding_filter()`: Simulates admission with funding filters

### 3. Output Structure Changes
```
output/
├── budget/           # Budget funding analysis results
│   ├── admitted_lists/
│   ├── filtered_eager/
│   ├── programs/
│   ├── all_applicants.csv
│   ├── chance_analysis.txt
│   ├── program_popularity.txt
│   └── ...
└── commercial/       # Commercial funding analysis results
    ├── admitted_lists/
    ├── filtered_eager/
    ├── programs/
    ├── all_applicants.csv
    ├── chance_analysis.txt
    ├── program_popularity.txt
    └── ...
```

### 4. Logic Implementation

#### Budget Funding Analysis
1. Filters only budget-funded program lists
2. Calculates program popularity based on budget funding data
3. Simulates admission process excluding admitted students from less popular programs
4. Saves results to `output/budget/`

#### Commercial Funding Analysis (if configured)
1. Filters only commercial-funded program lists
2. Loads budget analysis results to identify already-admitted students
3. Excludes budget-admitted students from commercial analysis
4. Simulates admission process for remaining candidates
5. Saves results to `output/commercial/`

### 5. Configuration Changes
The default configuration now includes comments about funding types:
```toml
target_funding_types = [
    "Бюджетное финансирование",
    # Note: Comment out commercial funding to only analyze budget funding
    # "Коммерческое финансирование",
]
```

### 6. Summary Output
The summary now shows results for both funding types:
- Budget funding analysis with program popularity and admission chances
- Commercial funding analysis (if configured) with separate results
- Separate recommendations for each funding type

## Real-World Logic
This implementation follows the real medical school admission process:
1. **Budget funding has priority** - top candidates get budget places first
2. **Commercial funding gets remaining candidates** - only students not admitted to budget funding are considered
3. **Exclusion logic** - students admitted to higher-priority programs are excluded from lower-priority programs
4. **Separate analysis** - each funding type can be analyzed independently or together

## Usage
- To analyze budget funding only: Include only "Бюджетное финансирование" in `target_funding_types`
- To analyze both: Include both funding types in `target_funding_types`
- Results are automatically separated into different directories for easy comparison
