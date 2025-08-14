# Configuration Guide

## Quick Start

1. **Run the program once** to create a default `config.toml`:

   ```bash
   cargo run
   ```

2. **Edit the configuration file**:

   ```bash
   nano config.toml  # or use any text editor
   ```

3. **Set your target SNILS** (required):

   ```toml
   target_snils = "15124960041"
   ```

4. **Run the analysis**:

   ```bash
   cargo run
   ```

## Configuration Options

### `target_snils` (required)

The SNILS of the applicant you want to analyze.

```toml
target_snils = "15124960041"
# or with dashes: "151-249-600 41"
```

### `programs_of_interest` (optional)

List of programs to focus the analysis on. If not specified, all programs will be analyzed.

```toml
programs_of_interest = [
    "ОП СПО Лечебное дело",
    "ОП СПО Фармация",
]
```

### `target_funding_types` (optional)

Types of funding to consider in the analysis.

```toml
target_funding_types = [
    "Бюджетное финансирование",      # Budget funding
    "Коммерческое финансирование",   # Commercial funding
]
```

### `data_directory` (optional)

Directory containing HTML files with admission data.

```toml
data_directory = "data-source"  # default
```

### `output_directory` (optional)

Directory where output files will be saved.

```toml
output_directory = "output"  # default
```

## Example Configurations

### Budget Funding Only

```toml
target_snils = "15124960041"
target_funding_types = ["Бюджетное финансирование"]
```

### Single Program Analysis

```toml
target_snils = "15124960041"
programs_of_interest = ["ОП СПО Лечебное дело"]
```

### All Medical Programs

```toml
target_snils = "15124960041"
programs_of_interest = [
    "ОП СПО Лечебное дело",
    "ОП СПО Фармация", 
    "ОП СПО Сестринское дело",
    "ОП СПО Акушерское дело",
    "ОП СПО Лабораторная диагностика",
    "ОП СПО Стоматология профилактическая",
    "ОП СПО Стоматология ортопедическая",
]
```

## Command Line Options

You can also specify a custom configuration file:

```bash
cargo run -- --config my-config.toml
```

## Output

The program generates comprehensive reports in the output directory:

- `all_programs_popularity.txt` - Program popularity rankings
- `chance_analysis.txt` - Detailed admission chance analysis
- `final_cutoff_analysis.txt` - Cutoff scores and rankings
- `programs/` - Individual program CSV files
- `filtered_eager/` - Filtered applicant lists with exclusion tracking
- `admitted_lists/` - Final admission lists
