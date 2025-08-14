mod models;
mod scraper;
mod analyzer;

use analyzer::{AdmissionAnalyzer, ChanceAnalysis};
use models::Config;
use anyhow::Result;
use clap::{Arg, Command};
use std::fs;
use std::path::Path;

/// Deduplicate records by SNILS within each program, keeping the best record for each SNILS
/// Priority: Original document (Да) > Consent (Да) > Priority number (lower is better)
fn deduplicate_records_by_snils(records: Vec<models::StudentRecord>) -> Vec<models::StudentRecord> {
    use std::collections::HashMap;
    use crate::models::normalize_snils;
    
    let mut best_records: HashMap<String, models::StudentRecord> = HashMap::new();
    
    for record in records {
        let normalized_snils = normalize_snils(&record.snils);
        
        match best_records.get(&normalized_snils) {
            None => {
                // First occurrence of this SNILS
                best_records.insert(normalized_snils, record);
            }
            Some(existing) => {
                // Compare and keep the better record
                let record_is_better = is_record_better(&record, existing);
                if record_is_better {
                    best_records.insert(normalized_snils, record);
                }
            }
        }
    }
    
    let mut result: Vec<models::StudentRecord> = best_records.into_values().collect();
    // Sort by rank to maintain original order
    result.sort_by_key(|r| r.rank);
    result
}

/// Determine if record1 is better than record2 for the same SNILS
/// Priority: Original document (Да) > Consent (Да) > Priority number (lower is better)
fn is_record_better(record1: &models::StudentRecord, record2: &models::StudentRecord) -> bool {
    // First priority: Original document
    let r1_has_doc = record1.has_original_document();
    let r2_has_doc = record2.has_original_document();
    
    if r1_has_doc != r2_has_doc {
        return r1_has_doc; // Prefer the one with original document
    }
    
    // Second priority: Consent
    let r1_has_consent = record1.has_consent();
    let r2_has_consent = record2.has_consent();
    
    if r1_has_consent != r2_has_consent {
        return r1_has_consent; // Prefer the one with consent
    }
    
    // Third priority: Lower priority number (1 is better than 2)
    record1.priority < record2.priority
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("abitur-analyzer")
        .version("1.0")
        .about("Analyzes admission chances for medical programs")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .get_matches();

    let config_file = matches.get_one::<String>("config").unwrap();
    
    // Load or create configuration
    let config = if Path::new(config_file).exists() {
        println!("📋 Loading configuration from: {}", config_file);
        Config::load_from_file(config_file)?
    } else {
        println!("📝 Creating default configuration file: {}", config_file);
        let default_config = Config::default();
        default_config.save_to_file(config_file)?;
        println!("⚠️  Please edit {} and set your target SNILS, then run the program again.", config_file);
        return Ok(());
    };

    // Validate configuration
    if config.target_snils.is_empty() {
        println!("❌ Error: target_snils is empty in configuration file");
        println!("   Please edit {} and set the target SNILS", config_file);
        return Ok(());
    }

    let output_dir = config.output_directory.as_deref().unwrap_or("output");

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;
    
    // Clean up previous results
    clean_output_directory(output_dir)?;

    println!("🔍 Analyzing admission data for SNILS: {}", config.target_snils);
    println!(" Output directory: {} (cleaned)", output_dir);
    if let Some(programs) = &config.programs_of_interest {
        println!("🎯 Programs of interest: {}", programs.join(", "));
    } else {
        println!("🎯 Programs of interest: ALL PROGRAMS");
    }
    println!("💰 Target funding types: {}", config.target_funding_types.join(", "));
    println!("🌐 Data source mode: {:?}", config.data_source_mode);

    // Initialize components
    let scraper = scraper::AdmissionScraper::new();

    // Process data sources based on configuration
    let mut all_program_records = Vec::new();
    
    // Process local files if configured
    if matches!(config.data_source_mode, models::DataSourceMode::Local | models::DataSourceMode::Both) {
        if let Some(data_dir) = &config.data_directory {
            println!("📂 Processing local files from: {}", data_dir);
            
            if std::path::Path::new(data_dir).exists() {
                for entry in fs::read_dir(data_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    if path.extension().and_then(|s| s.to_str()) == Some("html") {
                        println!("📄 Processing local file: {:?}", path.file_name().unwrap());
                        
                        match scraper.scrape_file(path.to_str().unwrap()) {
                            Ok(programs) => {
                                for (program_info, records) in programs {
                                    let original_count = records.len();
                                    println!("   ✅ Found {} applicants for program: {}", 
                                           original_count, program_info.name);
                                    
                                    // Deduplicate records by SNILS within this program
                                    let deduplicated_records = deduplicate_records_by_snils(records);
                                    let duplicates_removed = original_count - deduplicated_records.len();
                                    if duplicates_removed > 0 {
                                        println!("   🔄 Removed {} duplicate SNILS records", duplicates_removed);
                                    }
                                    
                                    all_program_records.push((program_info.name, deduplicated_records));
                                }
                            }
                            Err(e) => {
                                println!("   ❌ Error processing local file: {}", e);
                            }
                        }
                    }
                }
            } else {
                println!("   ⚠️  Local data directory '{}' does not exist", data_dir);
            }
        }
    }
    
    // Process internet URLs if configured
    if matches!(config.data_source_mode, models::DataSourceMode::Internet | models::DataSourceMode::Both) {
        if let Some(urls) = &config.internet_urls {
            println!("🌐 Processing internet sources ({} URLs)", urls.len());
            
            for url in urls {
                match scraper.scrape_url(url).await {
                    Ok(programs) => {
                        for (program_info, records) in programs {
                            let original_count = records.len();
                            println!("   ✅ Found {} applicants for program: {}", 
                                   original_count, program_info.name);
                            
                            // Deduplicate records by SNILS within this program
                            let deduplicated_records = deduplicate_records_by_snils(records);
                            let duplicates_removed = original_count - deduplicated_records.len();
                            if duplicates_removed > 0 {
                                println!("   🔄 Removed {} duplicate SNILS records", duplicates_removed);
                            }
                            
                            all_program_records.push((program_info.name, deduplicated_records));
                        }
                    }
                    Err(e) => {
                        println!("   ❌ Error processing URL {}: {}", url, e);
                    }
                }
            }
        } else {
            println!("   ⚠️  No internet URLs configured");
        }
    }

    if all_program_records.is_empty() {
        println!("❌ No valid data sources found or all sources failed");
        return Ok(());
    }

    // Perform unified priority-based analysis for all funding types
    println!("\n🎯 Analyzing admission chances using priority-based algorithm...");
    let analyzer = AdmissionAnalyzer::new(
        config.target_snils.clone(), 
        config.clone()
    );
    
    let analysis = analyzer.analyze_all_programs(all_program_records.clone());
    let chance_analysis = analyzer.analyze_target_chances(&analysis);

    // Generate reports with new unified data
    generate_program_popularity_report(&analysis, output_dir)?;
    generate_detailed_csv(&all_program_records, output_dir)?;
    generate_individual_program_csvs(&all_program_records, output_dir)?;
    generate_filtered_eager_csvs(&analysis, &all_program_records, output_dir)?;
    generate_available_places_csvs(&analysis, &all_program_records, output_dir)?;
    generate_final_cutoff_analysis(&analysis, &chance_analysis, &all_program_records, output_dir)?;

    println!("✅ Priority-based analysis complete!");

    // Print summary
    print_unified_summary(&analysis, &chance_analysis);

    println!("\n✅ Analysis complete!");
    println!("📂 Results: {}", output_dir);
    println!("Check the output directory for detailed reports.");
    Ok(())
}

fn filter_records_by_funding(
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    funding_types: &[String],
) -> Vec<(String, Vec<models::StudentRecord>)> {
    let mut filtered_records = Vec::new();
    
    for (program_name, records) in all_program_records {
        let filtered_program_records: Vec<models::StudentRecord> = records
            .iter()
            .filter(|record| funding_types.contains(&record.funding_source))
            .cloned()
            .collect();
        
        if !filtered_program_records.is_empty() {
            filtered_records.push((program_name.clone(), filtered_program_records));
        }
    }
    
    filtered_records
}

fn generate_program_popularity_report(
    analysis: &analyzer::AdmissionAnalysis,
    output_dir: &str,
) -> Result<()> {
    let mut content = String::new();
    content.push_str("Program Popularity Analysis\n");
    content.push_str("==========================\n\n");

    for popularity in &analysis.program_popularities {
        let eager_per_place = popularity.total_eager_applicants as f64 / popularity.available_places as f64;
        
        content.push_str(&format!(
            "Program: {} ({})\n\
            Eager applicants per place: {:.2}\n\
            Top candidates average priority: {:.2}\n\
            Average score: {:.2}\n\
            Available places: {}\n\
            Total eager applicants: {}\n\n",
            popularity.program_name,
            popularity.funding_source,
            eager_per_place,
            popularity.top_candidates_average_priority,
            popularity.average_score,
            popularity.available_places,
            popularity.total_eager_applicants
        ));
    }

    fs::write(Path::new(output_dir).join("program_popularity.txt"), content)?;
    Ok(())
}


fn generate_detailed_csv(
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;

    let csv_path = Path::new(output_dir).join("all_applicants.csv");
    let mut writer = Writer::from_path(csv_path)?;

    // Write headers
    writer.write_record(&[
        "Program",
        "Rank",
        "SNILS",
        "Priority",
        "Consent",
        "Document Type",
        "Average Score",
        "Subject Scores",
        "Psychological Test",
        "Funding Source",
        "Study Form",
        "Available Places",
    ])?;

    // Write data
    for (program_name, records) in all_program_records {
        for record in records {
            writer.write_record(&[
                program_name,
                &record.rank.to_string(),
                &record.snils,
                &record.priority.to_string(),
                &record.consent,
                &record.document_type,
                &record.average_score,
                &record.subject_scores,
                &record.psychological_test,
                &record.funding_source,
                &record.study_form,
                &record.available_places.to_string(),
            ])?;
        }
    }

    writer.flush()?;
    Ok(())
}

fn print_summary(
    budget_analysis: &analyzer::AdmissionAnalysis,
    budget_chance_analysis: &ChanceAnalysis,
    commercial_analysis: Option<&analyzer::AdmissionAnalysis>,
    commercial_chance_analysis: Option<&ChanceAnalysis>,
) {
    println!("\n📊 COMPREHENSIVE ADMISSION ANALYSIS");
    println!("===================================\n");
    
    // Print Budget Analysis
    print_funding_analysis("BUDGET", budget_analysis, budget_chance_analysis, "budget");
    
    // Print Commercial Analysis if available
    if let (Some(commercial_analysis), Some(commercial_chance_analysis)) = (commercial_analysis, commercial_chance_analysis) {
        println!("\n");
        print_funding_analysis("COMMERCIAL", commercial_analysis, commercial_chance_analysis, "commercial");
    }
}

fn print_unified_summary(
    analysis: &analyzer::AdmissionAnalysis,
    chance_analysis: &ChanceAnalysis,
) {
    println!("\n📊 UNIFIED PRIORITY-BASED ADMISSION ANALYSIS");
    println!("==========================================\n");
    
    println!("🏆 Program Popularity Ranking (by average priority):");
    for (i, popularity) in analysis.program_popularities.iter().enumerate().take(10) {
        let eager_per_place = popularity.eager_applicants.len() as f64 / popularity.available_places as f64;
        println!(
            "   {}. {} ({}) - {:.1} eager applicants per place (avg score: {:.2}, avg priority: {:.2})",
            i + 1,
            popularity.program_name,
            popularity.funding_source,
            eager_per_place,
            popularity.average_score,
            popularity.top_candidates_average_priority
        );
    }
    
    println!("\n🎯 Target Applicant Results:");
    if analysis.target_applicant_found {
        println!("✅ Target applicant found in the data");
        println!("📋 Application Results:");
        for (program_key, admitted) in &analysis.target_applicant_results {
            let status = if *admitted { "✅ ADMITTED" } else { "❌ Not admitted" };
            println!("   • {}: {}", program_key, status);
        }
        println!("\n📝 Final Recommendation:");
        println!("   {}", chance_analysis.final_recommendation);
    } else {
        println!("❌ Target applicant not found in the data");
        println!("   This could mean:");
        println!("   • The SNILS is incorrect");
        println!("   • The applicant didn't apply to any programs");
        println!("   • The data source doesn't contain this applicant");
    }
}

fn print_funding_analysis(
    funding_type: &str,
    analysis: &analyzer::AdmissionAnalysis,
    chance_analysis: &ChanceAnalysis,
    output_subdir: &str,
) {
    let icon = if funding_type == "BUDGET" { "💰" } else { "💳" };
    
    println!("{} {} FUNDING ANALYSIS:", icon, funding_type);
    println!("{}📈 Program Popularity (most to least competitive):", "");
    for (i, popularity) in analysis.program_popularities.iter().enumerate() {
        let eager_per_place = popularity.eager_applicants.len() as f64 / popularity.available_places as f64;
        println!(
            "   {}. {} ({}) - {:.1} eager applicants per place (avg score: {:.2}, avg priority: {:.2})",
            i + 1,
            popularity.program_name,
            popularity.funding_source,
            eager_per_place,
            popularity.average_score,
            popularity.top_candidates_average_priority
        );
    }
    
    println!("\n🎯 Detailed Cutoff Analysis:");
    
    // Read and display cutoff analysis
    let cutoff_path = format!("output/{}/final_cutoff_analysis.txt", output_subdir);
    if let Ok(cutoff_content) = fs::read_to_string(&cutoff_path) {
        let lines: Vec<&str> = cutoff_content.lines().collect();
        let mut current_program = String::new();
        let mut program_info = Vec::new();
        
        for line in lines.iter().skip(3) { // Skip header lines
            if line.is_empty() {
                if !current_program.is_empty() && !program_info.is_empty() {
                    println!("   📋 {}", current_program);
                    for info in &program_info {
                        println!("      {}", info);
                    }
                    println!();
                }
                current_program.clear();
                program_info.clear();
            } else if line.starts_with("Program: ") {
                current_program = line.strip_prefix("Program: ").unwrap_or(line).to_string();
            } else if line.starts_with("Status: ") {
                let status = line.strip_prefix("Status: ").unwrap_or(line);
                if status.contains("Admitted") {
                    program_info.push(format!("✅ {}", status));
                } else if status.contains("Not_Admitted") {
                    program_info.push(format!("❌ {}", status));
                } else if status.contains("Hypothetical") {
                    if status.contains("Would likely be admitted") {
                        program_info.push(format!("🔮 {}", status));
                    } else {
                        program_info.push(format!("🚫 {}", status));
                    }
                }
            } else if line.starts_with("Target score: ") || line.starts_with("Cutoff score: ") || line.starts_with("Available places: ") || line.starts_with("Position in admitted list: ") {
                program_info.push(format!("   {}", line));
            }
        }
        
        // Handle last program if file doesn't end with empty line
        if !current_program.is_empty() && !program_info.is_empty() {
            println!("   📋 {}", current_program);
            for info in &program_info {
                println!("      {}", info);
            }
        }
    }
    
    println!("\n💡 {} Recommendation: {}", funding_type, chance_analysis.final_recommendation);
}

// 1. Generate all programs popularity chain
fn generate_all_programs_popularity(
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use crate::models::StudentRecord;
    use std::collections::HashMap;

    let mut content = String::new();
    content.push_str("All Programs Popularity Chain\n");
    content.push_str("============================\n\n");
    content.push_str("Programs ordered from most to least popular (by funding type):\n\n");

    // Build: program_name -> funding_source -> Vec<StudentRecord>
    let mut by_program: HashMap<String, HashMap<String, Vec<StudentRecord>>> = HashMap::new();
    for (program_name, records) in all_program_records.iter() {
        for rec in records {
            by_program
                .entry(program_name.clone())
                .or_insert_with(HashMap::new)
                .entry(rec.funding_source.clone())
                .or_insert_with(Vec::new)
                .push(rec.clone());
        }
    }

    #[derive(Clone)]
    struct Entry {
        funding_source: String,
        applications_per_place: f64,
        top_subset_average_score: f64,
        available_places: u32,
        total_applications: usize,
        total_eager_applicants: usize,
    }

    #[derive(Clone)]
    struct ProgramGroup {
        program_name: String,
        // key for ordering programs: prefer budget metrics if present, else commercial, else first
        key_applications_per_place: f64,
        key_top_avg: f64,
        key_total_apps: usize,
        entries: Vec<Entry>,
    }

    let mut groups: Vec<ProgramGroup> = Vec::new();

    for (program_name, funding_map) in by_program.into_iter() {
        let mut entries: Vec<Entry> = Vec::new();
        // compute entry per funding
        for (funding, mut records) in funding_map.into_iter() {
            if records.is_empty() { continue; }
            records.sort_by_key(|r| r.rank);
            let available_places = records[0].available_places;
            let total_applications = records.len();
            let eager: Vec<StudentRecord> = records
                .into_iter()
                .filter(|r| r.has_original_document() || r.has_consent())
                .collect();
            let total_eager = eager.len();
            let applications_per_place = if available_places > 0 {
                total_eager as f64 / available_places as f64
            } else { 0.0 };
            // Top subset is available_places * 2 eager applicants
            let top_count = std::cmp::min(available_places as usize * 2, total_eager);
            let scores: Vec<f64> = eager
                .iter()
                .take(top_count)
                .filter_map(|r| r.get_numeric_score())
                .collect();
            let top_subset_average_score = if scores.is_empty() { 0.0 } else { scores.iter().sum::<f64>() / scores.len() as f64 };

            entries.push(Entry {
                funding_source: funding,
                applications_per_place,
                top_subset_average_score,
                available_places,
                total_applications,
                total_eager_applicants: total_eager,
            });
        }

        if entries.is_empty() { continue; }

        // determine key metrics: budget first, else commercial, else first
        let ke = if let Some(b) = entries.iter().find(|e| e.funding_source == "Бюджетное финансирование") {
            b.clone()
        } else if let Some(c) = entries.iter().find(|e| e.funding_source == "Коммерческое финансирование") {
            c.clone()
        } else {
            entries[0].clone()
        };

        groups.push(ProgramGroup {
            program_name,
            key_applications_per_place: ke.applications_per_place,
            key_top_avg: ke.top_subset_average_score,
            key_total_apps: ke.total_applications,
            entries,
        });
    }

    // Sort programs by key metrics
    groups.sort_by(|a, b| {
        b.key_applications_per_place
            .partial_cmp(&a.key_applications_per_place)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.key_top_avg.partial_cmp(&a.key_top_avg).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| b.key_total_apps.cmp(&a.key_total_apps))
    });

    // Emit entries: for each program, budget first (if any), then others by competitiveness
    let mut counter = 1usize;
    for group in groups.iter() {
        // Split budget vs others
        let mut budget: Vec<&Entry> = group.entries.iter().filter(|e| e.funding_source == "Бюджетное финансирование").collect();
        let mut others: Vec<&Entry> = group.entries.iter().filter(|e| e.funding_source != "Бюджетное финансирование").collect();
        // Sort others by competitiveness
        others.sort_by(|a, b| {
            b.applications_per_place
                .partial_cmp(&a.applications_per_place)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.top_subset_average_score.partial_cmp(&a.top_subset_average_score).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| b.total_applications.cmp(&a.total_applications))
        });

        // Print helper
        let mut print_entry = |e: &Entry| {
            let eager_per_place = if e.available_places > 0 {
                e.total_eager_applicants as f64 / e.available_places as f64
            } else { 0.0 };
            let total_per_place = if e.available_places > 0 {
                e.total_applications as f64 / e.available_places as f64
            } else { 0.0 };
            
            content.push_str(&format!(
                "{}. Program: {} ({})\n",
                counter,
                group.program_name,
                e.funding_source,
            ));
            content.push_str(&format!(
                "Applications per place: {:.2}\n",
                e.applications_per_place,
            ));
            content.push_str(&format!(
                "Eager applicants per place: {:.2}\n",
                eager_per_place,
            ));
            content.push_str(&format!(
                "Total applications per place: {:.2}\n",
                total_per_place,
            ));
            content.push_str(&format!(
                "Top candidates average score: {:.2}\n",
                e.top_subset_average_score,
            ));
            content.push_str(&format!(
                "Available places: {}\n",
                e.available_places,
            ));
            content.push_str(&format!(
                "Total applications: {}\n",
                e.total_applications,
            ));
            content.push_str(&format!(
                "Total eager applicants: {}\n\n",
                e.total_eager_applicants,
            ));
            counter += 1;
        };

        // Budget first
        if let Some(b) = budget.pop() { print_entry(b); }
        // Then others (e.g., commercial)
        for e in others { print_entry(e); }
    }

    fs::write(Path::new(output_dir).join("all_programs_popularity.txt"), content)?;
    Ok(())
}

// 2. Generate individual CSV files for each program
fn generate_individual_program_csvs(
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    
    let programs_dir = Path::new(output_dir).join("programs");
    fs::create_dir_all(&programs_dir)?;

    for (program_name, records) in all_program_records {
        let safe_name = program_name.replace("/", "_").replace(" ", "_");
        let csv_path = programs_dir.join(format!("{}.csv", safe_name));
        let mut writer = Writer::from_path(csv_path)?;

        // Write headers
        writer.write_record(&[
            "Rank", "SNILS", "Priority", "Consent", "Document_Type", "Average_Score",
            "Subject_Scores", "Psychological_Test", "Funding_Source", "Study_Form", "Available_Places"
        ])?;

        // Write data
        for record in records {
            writer.write_record(&[
                &record.rank.to_string(),
                &record.snils,
                &record.priority.to_string(),
                &record.consent,
                &record.document_type,
                &record.average_score,
                &record.subject_scores,
                &record.psychological_test,
                &record.funding_source,
                &record.study_form,
                &record.available_places.to_string(),
            ])?;
        }

        writer.flush()?;
    }

    Ok(())
}

// 3. Generate filtered eager applicants with exclusion marks
fn generate_filtered_eager_csvs(
    analysis: &analyzer::AdmissionAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let filtered_dir = Path::new(output_dir).join("filtered_eager");
    fs::create_dir_all(&filtered_dir)?;

    // Create exclusion tracker based on admission simulation
    let analyzer_instance = analyzer::AdmissionAnalyzer::new("dummy".to_string(), Config::default());
    let program_funding_groups = analyzer_instance.group_by_program_and_funding_public(all_program_records.to_vec());
    let mut excluded_normalized_snils = std::collections::HashSet::new();

    // Process programs in popularity order
    for popularity in &analysis.program_popularities {
        let program_name = &popularity.program_name;
        let safe_name = program_name.replace("/", "_").replace(" ", "_");
        let csv_path = filtered_dir.join(format!("{}_filtered_eager.csv", safe_name));
        let mut writer = Writer::from_path(csv_path)?;

        // Write headers
        writer.write_record(&[
            "Rank", "SNILS", "Priority", "Consent", "Document_Type", "Average_Score",
            "Subject_Scores", "Psychological_Test", "Funding_Source", "Study_Form", 
            "Available_Places", "Is_Eager", "Excluded_By_Higher_Priority"
        ])?;

        if let Some(funding_groups) = program_funding_groups.get(program_name) {
            // Process budget funding first
            if let Some(budget_records) = funding_groups.get("Бюджетное финансирование") {
                for record in budget_records {
                    let is_eager = record.has_original_document() || record.has_consent();
                    let normalized_snils = normalize_snils(&record.snils);
                    let is_excluded = excluded_normalized_snils.contains(&normalized_snils);
                    
                    writer.write_record(&[
                        &record.rank.to_string(),
                        &record.snils,
                        &record.priority.to_string(),
                        &record.consent,
                        &record.document_type,
                        &record.average_score,
                        &record.subject_scores,
                        &record.psychological_test,
                        &record.funding_source,
                        &record.study_form,
                        &record.available_places.to_string(),
                        &if is_eager { "Да".to_string() } else { "Нет".to_string() },
                        &if is_excluded { "Да".to_string() } else { "Нет".to_string() },
                    ])?;
                }
                
                // Mark as excluded those who get admitted
                let available_places = budget_records[0].available_places as usize;
                let to_exclude: Vec<String> = budget_records
                    .iter()
                    .filter(|r| (r.has_original_document() || r.has_consent()) && !excluded_normalized_snils.contains(&normalize_snils(&r.snils)))
                    .take(available_places)
                    .map(|r| normalize_snils(&r.snils))
                    .collect();
                
                for snils in to_exclude {
                    excluded_normalized_snils.insert(snils);
                }
            }
            
            // Process commercial funding
            if let Some(commercial_records) = funding_groups.get("Коммерческое финансирование") {
                for record in commercial_records {
                    let is_eager = record.has_original_document() || record.has_consent();
                    let normalized_snils = normalize_snils(&record.snils);
                    let is_excluded = excluded_normalized_snils.contains(&normalized_snils);
                    
                    writer.write_record(&[
                        &record.rank.to_string(),
                        &record.snils,
                        &record.priority.to_string(),
                        &record.consent,
                        &record.document_type,
                        &record.average_score,
                        &record.subject_scores,
                        &record.psychological_test,
                        &record.funding_source,
                        &record.study_form,
                        &record.available_places.to_string(),
                        &if is_eager { "Да".to_string() } else { "Нет".to_string() },
                        &if is_excluded { "Да".to_string() } else { "Нет".to_string() },
                    ])?;
                }
                
                // Mark as excluded those who get admitted
                let available_places = commercial_records[0].available_places as usize;
                let to_exclude: Vec<String> = commercial_records
                    .iter()
                    .filter(|r| (r.has_original_document() || r.has_consent()) && !excluded_normalized_snils.contains(&normalize_snils(&r.snils)))
                    .take(available_places)
                    .map(|r| normalize_snils(&r.snils))
                    .collect();
                
                for snils in to_exclude {
                    excluded_normalized_snils.insert(snils);
                }
            }
        }

        writer.flush()?;
    }

    Ok(())
}

// 4. Generate available places CSV files (only admitted students)
fn generate_available_places_csvs(
    analysis: &analyzer::AdmissionAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let admitted_dir = Path::new(output_dir).join("admitted_lists");
    fs::create_dir_all(&admitted_dir)?;

    // Get target SNILS from the analysis
    let config = models::Config::load_from_file("config.toml").unwrap_or_default();
    let target_snils = config.target_snils;
    let normalized_target = normalize_snils(&target_snils);

    // Process each program-funding combination
    for (program_key, admitted_snils_list) in &analysis.final_admission_results {
        let safe_name = program_key.replace("/", "_").replace(" ", "_");
        let csv_path = admitted_dir.join(format!("{}_admitted.csv", safe_name));
        let mut writer = Writer::from_path(csv_path)?;

        // Write headers
        writer.write_record(&[
            "Rank", "SNILS", "Priority", "Consent", "Document_Type", "Average_Score",
            "Subject_Scores", "Psychological_Test", "Funding_Source", "Study_Form", 
            "Available_Places", "Admission_Status"
        ])?;

        // Parse program_key to get program_name and funding_source
        let (program_name, funding_source) = if program_key.ends_with("_Бюджетное финансирование") {
            let name_part = program_key.strip_suffix("_Бюджетное финансирование").unwrap();
            (name_part.to_string(), "Бюджетное финансирование".to_string())
        } else if program_key.ends_with("_Коммерческое финансирование") {
            let name_part = program_key.strip_suffix("_Коммерческое финансирование").unwrap();
            (name_part.to_string(), "Коммерческое финансирование".to_string())
        } else {
            // Fallback for other funding types
            let last_underscore_pos = program_key.rfind('_').unwrap_or(0);
            if last_underscore_pos == 0 {
                continue; // Skip malformed keys
            }
            let program_name = program_key[..last_underscore_pos].to_string();
            let funding_source = program_key[last_underscore_pos + 1..].to_string();
            (program_name, funding_source)
        };

        // Find matching records in all_program_records
        let mut matching_records = Vec::new();
        let mut target_record: Option<models::StudentRecord> = None;
        
        for (record_program_name, program_records) in all_program_records {
            if record_program_name == &program_name {
                for record in program_records {
                    if record.funding_source == funding_source {
                        matching_records.push(record.clone());
                        
                        // Check if this is the target applicant
                        if normalize_snils(&record.snils) == normalized_target {
                            target_record = Some(record.clone());
                        }
                    }
                }
            }
        }

        if !matching_records.is_empty() {
            // Sort matching records by rank to maintain order
            matching_records.sort_by_key(|r| r.rank);
            
            // Create a set of admitted SNILS for quick lookup
            let admitted_snils_set: std::collections::HashSet<String> = admitted_snils_list
                .iter()
                .map(|snils| normalize_snils(snils))
                .collect();

            let available_places = matching_records[0].available_places as usize;

            // Calculate cutoff score from admitted students
            let cutoff_score = if !admitted_snils_list.is_empty() {
                let mut lowest_score = f64::MAX;
                for admitted_snils in admitted_snils_list {
                    for record in &matching_records {
                        if normalize_snils(&record.snils) == normalize_snils(admitted_snils) {
                            if let Some(score) = record.get_numeric_score() {
                                lowest_score = lowest_score.min(score);
                            }
                        }
                    }
                }
                if lowest_score == f64::MAX { 0.0 } else { lowest_score }
            } else {
                0.0
            };

            // Create a combined list with both admitted students and target applicant in proper rank order
            let mut all_relevant_records = Vec::new();
            
            for record in &matching_records {
                let normalized_record_snils = normalize_snils(&record.snils);
                let is_admitted = admitted_snils_set.contains(&normalized_record_snils);
                let is_target = normalized_record_snils == normalized_target;
                
                // Include if: admitted OR target applicant
                if is_admitted || is_target {
                    all_relevant_records.push((record.clone(), is_admitted, is_target));
                }
            }

            // Sort by rank to ensure proper order
            all_relevant_records.sort_by_key(|(record, _, _)| record.rank);

            // Write all records in proper rank order
            let mut admission_position = 0;
            for (record, is_admitted, is_target) in all_relevant_records {
                if is_admitted {
                    admission_position += 1;
                }

                let admission_status = if is_target {
                    // For target applicant, determine status based on score vs cutoff
                    let target_score = record.get_numeric_score().unwrap_or(0.0);
                    
                    if is_admitted {
                        // Target was actually admitted
                        if admission_position <= available_places {
                            match record.funding_source.as_str() {
                                "Бюджетное финансирование" => "Admitted_Budget+",
                                "Коммерческое финансирование" => "Admitted_Commercial+",
                                _ => "Admitted_Other+",
                            }
                        } else {
                            match record.funding_source.as_str() {
                                "Бюджетное финансирование" => "Admitted_Budget-",
                                "Коммерческое финансирование" => "Admitted_Commercial-",
                                _ => "Admitted_Other-",
                            }
                        }
                    } else {
                        // Target was not admitted - check if their score is above cutoff
                        if target_score > cutoff_score && cutoff_score > 0.0 {
                            "Target_NotAdmitted+"  // Score above cutoff but not admitted due to priority
                        } else {
                            "Target_NotAdmitted-"  // Score below cutoff or no cutoff available
                        }
                    }
                } else {
                    // Regular admitted student
                    match record.funding_source.as_str() {
                        "Бюджетное финансирование" => "Admitted_Budget",
                        "Коммерческое финансирование" => "Admitted_Commercial",
                        _ => "Admitted_Other",
                    }
                };

                writer.write_record(&[
                    &record.rank.to_string(),
                    &record.snils,
                    &record.priority.to_string(),
                    &record.consent,
                    &record.document_type,
                    &record.average_score,
                    &record.subject_scores,
                    &record.psychological_test,
                    &record.funding_source,
                    &record.study_form,
                    &record.available_places.to_string(),
                    admission_status,
                ])?;
            }
        }

        writer.flush()?;
    }

    Ok(())
}

// 5. Generate final cutoff analysis for programs of interest with target applicant position
fn generate_final_cutoff_analysis(
    analysis: &analyzer::AdmissionAnalysis,
    chance_analysis: &analyzer::ChanceAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let final_path = Path::new(output_dir).join("final_cutoff_analysis.txt");
    let final_csv_path = Path::new(output_dir).join("final_cutoff_analysis.csv");
    
    let mut content = String::new();
    content.push_str(&format!("Final Cutoff Analysis for SNILS: {}\n", chance_analysis.target_snils));
    content.push_str("==========================================\n\n");

    let mut csv_writer = Writer::from_path(final_csv_path)?;
    csv_writer.write_record(&[
        "Program", "Funding_Type", "Position_In_Admitted", "Available_Places", 
        "Target_Score", "Cutoff_Score", "Admission_Position", "Admission_Status"
    ])?;

    let normalized_target = normalize_snils(&chance_analysis.target_snils);

    // Process each program-funding combination from admission results
    for (program_key, admitted_snils_list) in &analysis.final_admission_results {
        // Parse program_key to get program_name and funding_source
        let (program_name, funding_source) = if program_key.ends_with("_Бюджетное финансирование") {
            let name_part = program_key.strip_suffix("_Бюджетное финансирование").unwrap();
            (name_part.to_string(), "Бюджетное финансирование".to_string())
        } else if program_key.ends_with("_Коммерческое финансирование") {
            let name_part = program_key.strip_suffix("_Коммерческое финансирование").unwrap();
            (name_part.to_string(), "Коммерческое финансирование".to_string())
        } else {
            let last_underscore_pos = program_key.rfind('_').unwrap_or(0);
            if last_underscore_pos == 0 {
                continue; // Skip malformed keys
            }
            let program_name = program_key[..last_underscore_pos].to_string();
            let funding_source = program_key[last_underscore_pos + 1..].to_string();
            (program_name, funding_source)
        };

        // Find matching records in all_program_records
        let mut all_matching_records = Vec::new();
        
        for (record_program_name, program_records) in all_program_records {
            if record_program_name == &program_name {
                for record in program_records {
                    if record.funding_source == funding_source {
                        all_matching_records.push(record.clone());
                    }
                }
            }
        }

        if all_matching_records.is_empty() {
            continue;
        }

        // Sort records by rank to maintain order
        all_matching_records.sort_by_key(|r| r.rank);
        let available_places = all_matching_records[0].available_places as usize;

        // Check if target was admitted to this specific program-funding combination
        let is_admitted = admitted_snils_list
            .iter()
            .any(|snils| normalize_snils(snils) == normalized_target);

        // Calculate actual cutoff score (lowest score among admitted applicants)
        let cutoff_score = if !admitted_snils_list.is_empty() {
            let mut lowest_score = f64::MAX;
            for admitted_snils in admitted_snils_list {
                for record in &all_matching_records {
                    if normalize_snils(&record.snils) == normalize_snils(admitted_snils) {
                        if let Some(score) = record.get_numeric_score() {
                            lowest_score = lowest_score.min(score);
                        }
                    }
                }
            }
            if lowest_score == f64::MAX { 0.0 } else { lowest_score }
        } else {
            0.0
        };

        // Find the target record in the matching records
        let target_record = all_matching_records
            .iter()
            .find(|record| normalize_snils(&record.snils) == normalized_target);

        if let Some(target_rec) = target_record {
            let target_score = target_rec.get_numeric_score().unwrap_or(0.0);
            
            // Calculate position and status - FIXED LOGIC
            let (admission_status, status_detail, position_info) = if is_admitted {
                let position = admitted_snils_list
                    .iter()
                    .position(|snils| normalize_snils(snils) == normalized_target)
                    .map(|pos| pos + 1)
                    .unwrap_or(0);
                
                let position_str = format!("Position in admitted list: {} (of {} admitted)\n", position, admitted_snils_list.len());
                ("Admitted".to_string(), String::new(), position_str)
            } else {
                // FIXED: Check if target score is higher than cutoff - should be "Admitted" status
                if target_score > cutoff_score && cutoff_score > 0.0 {
                    // Target has good enough score but wasn't admitted due to priority logic
                    let target_rank_position = all_matching_records
                        .iter()
                        .position(|r| normalize_snils(&r.snils) == normalized_target)
                        .map(|pos| pos + 1)
                        .unwrap_or(0);
                    
                    let detail = format!(" (would qualify by score but priority {} not selected)", target_rec.priority);
                    ("Admitted_ByScore_NotByPriority".to_string(), detail, String::new())
                } else {
                    // Target doesn't have good enough score
                    let target_rank_position = all_matching_records
                        .iter()
                        .position(|r| normalize_snils(&r.snils) == normalized_target)
                        .map(|pos| pos + 1)
                        .unwrap_or(0);
                    
                    let applicants_behind = if target_rank_position > available_places {
                        target_rank_position - available_places
                    } else {
                        0
                    };
                    
                    let detail = if applicants_behind > 0 {
                        format!(" ({} applicants behind)", applicants_behind)
                    } else {
                        String::new()
                    };
                    
                    ("Not_Admitted".to_string(), detail, String::new())
                }
            };

            content.push_str(&format!(
                "Program: {}\n\
                Funding: {}\n\
                {}Available places: {}\n\
                Target score: {:.4}\n\
                Cutoff score: {:.4}\n\
                Status: {}{}\n\n",
                program_name,
                funding_source,
                position_info,
                available_places,
                target_score,
                cutoff_score,
                admission_status,
                status_detail
            ));

            let position_csv = if is_admitted {
                let position = admitted_snils_list
                    .iter()
                    .position(|snils| normalize_snils(snils) == normalized_target)
                    .map(|pos| pos + 1)
                    .unwrap_or(0);
                format!("Position {} of {}", position, admitted_snils_list.len())
            } else {
                "Not in list".to_string()
            };

            csv_writer.write_record(&[
                &program_name,
                &funding_source,
                &position_csv,
                &available_places.to_string(),
                &format!("{:.4}", target_score),
                &format!("{:.4}", cutoff_score),
                &position_csv,
                &admission_status,
            ])?;
        } else {
            // Target applicant not found in this program-funding combination
            content.push_str(&format!(
                "Program: {} - Target applicant not found\n\
                Funding: {}\n\
                Available places: {}\n\
                Target score: N/A\n\
                Cutoff score: {:.4}\n\
                Status: Hypothetical: Cannot determine (target did not apply)\n\n",
                program_name,
                funding_source,
                available_places,
                cutoff_score
            ));
        }
    }

    fs::write(final_path, content)?;
    csv_writer.flush()?;
    Ok(())
}

// Clean up previous results from output directory
fn clean_output_directory(output_dir: &str) -> Result<()> {
    let output_path = Path::new(output_dir);
    
    if !output_path.exists() {
        return Ok(());
    }
    
    println!("🧹 Cleaning previous results...");
    
    // List of files/directories to clean
    let items_to_clean = [
        "all_applicants.csv",
        "all_programs_popularity.txt", 
        "chance_analysis.txt",
        "program_popularity.txt",
        "final_cutoff_analysis.txt",
        "final_cutoff_analysis.csv",
        "programs",
        "filtered_eager",
        "admitted_lists",
    ];
    
    for item in &items_to_clean {
        let item_path = output_path.join(item);
        
        if item_path.exists() {
            if item_path.is_file() {
                fs::remove_file(&item_path)?;
                println!("   🗑️  Removed file: {}", item);
            } else if item_path.is_dir() {
                fs::remove_dir_all(&item_path)?;
                println!("   🗑️  Removed directory: {}", item);
            }
        }
    }
    
    println!("   ✅ Output directory cleaned");
    Ok(())
}
