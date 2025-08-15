mod models;
mod scraper;
mod analyzer;

use analyzer::{AdmissionAnalyzer};
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

/// finds max score in list of records
/// starting from first record set score to max if it less than max. do until meet last record with actualy max score
fn set_max_score_on_privileged_records(records: &mut Vec<models::StudentRecord>) {
    let max_score = records.iter()
        .filter_map(|r| r.get_numeric_score())
        .fold(0.0, |max, score| if max < score { score } else { max });

    let mut last_max_score_position = records.iter().rposition(|r| r.get_numeric_score() == Some(max_score)).unwrap_or(0);

    for record in records {
        if last_max_score_position == 0 { break; }

        if let Some(score) = record.get_numeric_score() {
            if score < max_score {
                record.set_numeric_score(max_score);
            } 
        }

        last_max_score_position -= 1;
    }
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
        .about("Simultes admission process")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .arg(
            Arg::new("snils")
                .short('s')
                .long("snils")
                .value_name("SNILS")
                .help("target applicant id")
        )
        .arg(
            Arg::new("data_source_mode")
                .short('d')
                .long("data_source_mode")
                .value_name("DATA_SOURCE_MODE")
                .help("data source mode 'local'/'internet")
                .default_value("")
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

    let target_snils = matches.get_one::<String>("snils").cloned().unwrap_or_else(|| config.target_snils.clone());

    // Validate configuration
    if target_snils.is_empty() {
        println!("❌ Error: target_snils is empty in configuration file and no argument provided");
        println!("   Please edit {} and set the target SNILS or pass it as a command-line argument", config_file);
        return Ok(());
    }

    println!("Data source mode from config: {:?}", config.data_source_mode);

    let data_source_mode_arg = matches.get_one::<String>("data_source_mode");
    println!("📂 Using data source mode from arguments: {:?}", data_source_mode_arg);
    let data_source_mode = match data_source_mode_arg {
        Some(str) => {
            if str == "local" {
                models::DataSourceMode::Local
            } else if str == "internet" {
                models::DataSourceMode::Internet
            } else {
                config.data_source_mode.clone()
            }
        },
        _ => config.data_source_mode.clone(),
    };

    let output_dir = config.output_directory.as_deref().unwrap_or("output");

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;
    
    // Clean up previous results
    clean_output_directory(output_dir)?;

    println!("🔍 Analyzing admission data for SNILS: {}", target_snils);
    println!(" Output directory: {} (cleaned)", output_dir);
    println!("🌐 Data source mode: {:?}", data_source_mode);

    // Initialize components
    let scraper = scraper::AdmissionScraper::new();

    // Process data sources based on configuration
    let mut all_program_records = Vec::new();
    
    // Process local files if configured
    if matches!(data_source_mode, models::DataSourceMode::Local | models::DataSourceMode::Both) {
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
                                    let mut deduplicated_records = deduplicate_records_by_snils(records);
                                    let duplicates_removed = original_count - deduplicated_records.len();
                                    if duplicates_removed > 0 {
                                        println!("   🔄 Removed {} duplicate SNILS records", duplicates_removed);
                                    }
                                    set_max_score_on_privileged_records(&mut deduplicated_records);
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
    if matches!(data_source_mode, models::DataSourceMode::Internet | models::DataSourceMode::Both) {
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
                            let mut deduplicated_records = deduplicate_records_by_snils(records);
                            let duplicates_removed = original_count - deduplicated_records.len();
                            if duplicates_removed > 0 {
                                println!("   🔄 Removed {} duplicate SNILS records", duplicates_removed);
                            }
                            set_max_score_on_privileged_records(&mut deduplicated_records);
                            
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
    let analyzer = AdmissionAnalyzer::new(&target_snils);

    let analysis = analyzer.analyze_all_programs(&all_program_records);

    // Generate reports with new unified data
    generate_program_popularity_report(&analysis, output_dir)?;
    generate_detailed_csv(&all_program_records, output_dir)?;
    generate_individual_program_csvs(&all_program_records, output_dir)?;
    generate_filtered_eager_csvs(&target_snils, &analysis, &all_program_records, output_dir)?;
    generate_available_places_csvs(&target_snils, &analysis, &all_program_records, output_dir)?;
    generate_final_cutoff_analysis(&target_snils, &analysis,  &all_program_records, output_dir)?;

    println!("✅ Priority-based analysis complete!");
    println!("📂 Results: {}", output_dir);
    println!("Check the output directory for detailed reports.");
    Ok(())
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
    target_snils: &str,
    analysis: &analyzer::AdmissionAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let filtered_dir = Path::new(output_dir).join("filtered_eager");
    fs::create_dir_all(&filtered_dir)?;

    // Create exclusion tracker based on admission simulation
    let analyzer_instance = analyzer::AdmissionAnalyzer::new(target_snils);
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
    target_snils: &str,
    analysis: &analyzer::AdmissionAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let admitted_dir = Path::new(output_dir).join("admitted_lists");
    fs::create_dir_all(&admitted_dir)?;

    // Get target SNILS from the analysis
    let normalized_target = normalize_snils(target_snils);

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
        
        for (record_program_name, program_records) in all_program_records {
            if record_program_name == &program_name {
                for record in program_records {
                    if record.funding_source == funding_source {
                        matching_records.push(record.clone());
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

// 5. Generate final cutoff analysis for programs by popularity of interest with target applicant position
fn generate_final_cutoff_analysis(
    target_snils: &str,
    analysis: &analyzer::AdmissionAnalysis,
    all_program_records: &[(String, Vec<models::StudentRecord>)],
    output_dir: &str,
) -> Result<()> {
    use csv::Writer;
    use crate::models::normalize_snils;
    
    let final_path = Path::new(output_dir).join("final_cutoff_analysis.txt");
    let final_csv_path = Path::new(output_dir).join("final_cutoff_analysis.csv");
    
    let mut content = String::new();
    content.push_str(&format!("Final Cutoff Analysis for SNILS: {}\n", target_snils));
    content.push_str("==========================================\n\n");

    let mut csv_writer = Writer::from_path(final_csv_path)?;
    csv_writer.write_record(&[
        "Program", "Funding_Type", "Position_In_Admitted", "Available_Places", 
        "Target_Score", "Cutoff_Score", "Admission_Position", "Admission_Status"
    ])?;

    let normalized_target = normalize_snils(target_snils);

    println!("📊 UNIFIED PRIORITY-BASED ADMISSION ANALYSIS for target SNILS: {}", target_snils);
    println!("==========================================");

    // Process each program-funding combination from admission results in order of popularity
    for program_popularity in &analysis.program_popularities {
        let program_key = &program_popularity.program_key;
        let admitted_snils_list = &analysis.final_admission_results[program_key];

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
                    let detail = format!(" (would qualify by score but priority {} not selected)", target_rec.priority);
                    ("Admitted_ByScore_NotByPriority".to_string(), detail, String::new())
                } else {
                    let detail = String::new(); 
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
            let eager_per_place = program_popularity.eager_applicants.len() as f64 / program_popularity.available_places as f64;

            let status_ico = if is_admitted {
                "✅"
            } else {
                "❌"
            };
            let target_priority = all_matching_records
                .iter()
                .find(|r| normalize_snils(&r.snils) == normalized_target)
                .map(|r| r.priority)
                .unwrap_or(0);
            println!("{} Program: {}, funding: {}", status_ico, program_name, funding_source);
            println!(
                "Available Places: {}, Cutoff Score: {:.4}, Applicants per place: {:.1}, Avg priority: {:.2}",
                available_places, cutoff_score, eager_per_place, program_popularity.top_candidates_average_priority
            );
            println!(
                "Priority:{}, Target Score: {:.4}, Status: {}, Position in admitted: {}",
                target_priority, target_score, admission_status, position_csv
            );
            println!("");


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
