use crate::models::{StudentRecord, normalize_snils, Config};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ProgramPopularity {
    pub program_name: String,
    pub applications_per_place: f64,
    pub top_third_average_score: f64,
    pub available_places: u32,
    pub total_applications: usize, // new: all applicants in the list
    pub total_applicants: usize,
    pub eager_applicants: Vec<StudentRecord>,
}

#[derive(Debug, Clone)]
pub struct AdmissionAnalysis {
    pub program_popularities: Vec<ProgramPopularity>,
    pub final_admission_results: HashMap<String, Vec<String>>, // program_name -> admitted SNILSes
    pub target_applicant_found: bool,
    pub target_applicant_results: Vec<(String, bool)>, // (program_name, admitted)
}

#[derive(Debug, Clone)]
pub struct ChanceAnalysis {
    pub target_snils: String,
    pub programs_admitted_to: Vec<String>,
    pub programs_rejected_from: Vec<String>,
    pub final_recommendation: String,
}

pub struct AdmissionAnalyzer {
    pub target_snils: String,
    pub config: Config,
    pub funding_filter: Option<Vec<String>>,
    pub budget_exclusions: Option<AdmissionAnalysis>,
}

impl AdmissionAnalyzer {
    pub fn new(target_snils: String, config: Config) -> Self {
        Self { 
            target_snils, 
            config,
            funding_filter: None,
            budget_exclusions: None,
        }
    }

    pub fn new_with_funding_filter(target_snils: String, config: Config, funding_types: Vec<String>) -> Self {
        Self { 
            target_snils, 
            config,
            funding_filter: Some(funding_types),
            budget_exclusions: None,
        }
    }

    pub fn new_with_funding_filter_and_budget_exclusions(
        target_snils: String, 
        config: Config, 
        funding_types: Vec<String>,
        budget_analysis: AdmissionAnalysis
    ) -> Self {
        Self { 
            target_snils, 
            config,
            funding_filter: Some(funding_types),
            budget_exclusions: Some(budget_analysis),
        }
    }

    /// Main analysis function following the real admission logic
    pub fn analyze_all_programs(&self, all_program_records: Vec<(String, Vec<StudentRecord>)>) -> AdmissionAnalysis {
        // Step 1: Group records by program and funding type
        let program_funding_groups = self.group_by_program_and_funding(all_program_records.clone());
        
        // Step 2: Filter by funding types if specified
        let filtered_program_funding_groups = if let Some(ref funding_filter) = self.funding_filter {
            self.filter_by_funding_types(program_funding_groups, funding_filter)
        } else {
            program_funding_groups
        };
        
        // Step 3: Calculate popularity for each program (considering only filtered funding types)
        let mut program_popularities = Vec::new();
        for (program_name, funding_groups) in &filtered_program_funding_groups {
            // Check if the program has any funding type that matches our target funding types
            let has_target_funding = funding_groups.keys().any(|funding_type| {
                if let Some(ref filter) = self.funding_filter {
                    filter.contains(funding_type)
                } else {
                    self.config.target_funding_types.contains(funding_type)
                }
            });
            
            if has_target_funding {
                // Use the first available funding type from our filtered list
                let records_for_popularity = funding_groups.values().next().unwrap();
                let popularity = self.calculate_program_popularity(program_name, records_for_popularity);
                program_popularities.push(popularity);
            }
        }

        // Step 4: Sort programs by popularity (most popular first)
        program_popularities.sort_by(|a, b| {
            b.applications_per_place
                .partial_cmp(&a.applications_per_place)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.top_third_average_score
                        .partial_cmp(&a.top_third_average_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        // Step 5: Simulate admission process
        let final_admission_results = if self.funding_filter.is_some() {
            self.simulate_admission_with_funding_filter(&program_popularities, &filtered_program_funding_groups)
        } else {
            self.simulate_admission_with_funding_priority(&program_popularities, &filtered_program_funding_groups)
        };

        // Step 6: Check if target applicant was found and their results
        let (target_found, target_results) = self.check_target_applicant_results(&final_admission_results, &all_program_records);

        AdmissionAnalysis {
            program_popularities,
            final_admission_results,
            target_applicant_found: target_found,
            target_applicant_results: target_results,
        }
    }

    /// Public method to group records by program and funding type (for reporting)
    pub fn group_by_program_and_funding_public(
        &self,
        all_program_records: Vec<(String, Vec<StudentRecord>)>,
    ) -> HashMap<String, HashMap<String, Vec<StudentRecord>>> {
        self.group_by_program_and_funding(all_program_records)
    }

    /// Group records by program name and funding type
    fn group_by_program_and_funding(
        &self,
        all_program_records: Vec<(String, Vec<StudentRecord>)>,
    ) -> HashMap<String, HashMap<String, Vec<StudentRecord>>> {
        let mut grouped: HashMap<String, HashMap<String, Vec<StudentRecord>>> = HashMap::new();
        
        for (program_name, records) in all_program_records {
            if records.is_empty() {
                continue;
            }
            
            // Group by funding type within each program
            for record in records {
                let funding_type = record.funding_source.clone();
                grouped
                    .entry(program_name.clone())
                    .or_insert_with(HashMap::new)
                    .entry(funding_type)
                    .or_insert_with(Vec::new)
                    .push(record);
            }
        }
        
        grouped
    }

    /// Filter programs by funding types
    fn filter_by_funding_types(
        &self,
        program_funding_groups: HashMap<String, HashMap<String, Vec<StudentRecord>>>,
        funding_filter: &[String],
    ) -> HashMap<String, HashMap<String, Vec<StudentRecord>>> {
        let mut filtered = HashMap::new();
        
        for (program_name, funding_groups) in program_funding_groups {
            let mut filtered_funding_groups = HashMap::new();
            
            for (funding_type, records) in funding_groups {
                if funding_filter.contains(&funding_type) {
                    filtered_funding_groups.insert(funding_type, records);
                }
            }
            
            if !filtered_funding_groups.is_empty() {
                filtered.insert(program_name, filtered_funding_groups);
            }
        }
        
        filtered
    }

    /// Simulate admission with funding filter and potential budget exclusions
    fn simulate_admission_with_funding_filter(
        &self,
        program_popularities: &[ProgramPopularity],
        program_funding_groups: &HashMap<String, HashMap<String, Vec<StudentRecord>>>,
    ) -> HashMap<String, Vec<String>> {
        let mut final_results = HashMap::new();
        let mut excluded_normalized_snils = std::collections::HashSet::new();

        // If we have budget exclusions, exclude students admitted to programs in budget analysis
        if let Some(ref budget_analysis) = self.budget_exclusions {
            for (_, admitted_list) in &budget_analysis.final_admission_results {
                for snils in admitted_list {
                    // For commercial funding, exclude students admitted to budget programs
                    // except if it's the target applicant and they're applying to the same program
                    let normalized_snils = normalize_snils(snils);
                    if normalize_snils(&self.target_snils) != normalized_snils {
                        excluded_normalized_snils.insert(normalized_snils);
                    }
                }
            }
        }

        // Process programs from most to least popular
        for popularity in program_popularities {
            let program_name = &popularity.program_name;
            
            // For commercial funding with budget exclusions, allow target applicant 
            // to be considered for the same program even if they were admitted to budget
            let mut local_excluded = excluded_normalized_snils.clone();
            if let Some(ref budget_analysis) = self.budget_exclusions {
                if let Some(budget_admitted) = budget_analysis.final_admission_results.get(program_name) {
                    let target_normalized = normalize_snils(&self.target_snils);
                    // Allow target applicant to be reconsidered for same program in commercial
                    if budget_admitted.iter().any(|snils| normalize_snils(snils) == target_normalized) {
                        local_excluded.remove(&target_normalized);
                    }
                }
            }
            
            if let Some(funding_groups) = program_funding_groups.get(program_name) {
                let mut all_admitted_to_program = Vec::new();
                
                // Process each funding type (should be only one due to filtering)
                for records in funding_groups.values() {
                    let admitted = self.process_funding_type(
                        records,
                        &mut local_excluded,
                    );
                    all_admitted_to_program.extend(admitted);
                }
                
                // Update the global excluded set with newly admitted students
                for snils in &all_admitted_to_program {
                    excluded_normalized_snils.insert(normalize_snils(snils));
                }
                
                final_results.insert(program_name.clone(), all_admitted_to_program);
            }
        }

        final_results
    }

    /// Simulate admission with budget-first, then commercial funding priority
    fn simulate_admission_with_funding_priority(
        &self,
        program_popularities: &[ProgramPopularity],
        program_funding_groups: &HashMap<String, HashMap<String, Vec<StudentRecord>>>,
    ) -> HashMap<String, Vec<String>> {
        let mut final_results = HashMap::new();
        let mut excluded_normalized_snils = std::collections::HashSet::new();

        // Process programs from most to least popular
        for popularity in program_popularities {
            let program_name = &popularity.program_name;
            
            if let Some(funding_groups) = program_funding_groups.get(program_name) {
                let mut all_admitted_to_program = Vec::new();
                
                // Step 1: Process budget funding first
                if let Some(budget_records) = funding_groups.get("Бюджетное финансирование") {
                    let budget_admitted = self.process_funding_type(
                        budget_records,
                        &mut excluded_normalized_snils,
                    );
                    all_admitted_to_program.extend(budget_admitted);
                }
                
                // Step 2: Process commercial funding with remaining candidates
                if let Some(commercial_records) = funding_groups.get("Коммерческое финансирование") {
                    let commercial_admitted = self.process_funding_type(
                        commercial_records,
                        &mut excluded_normalized_snils,
                    );
                    all_admitted_to_program.extend(commercial_admitted);
                }
                
                final_results.insert(program_name.clone(), all_admitted_to_program);
            }
        }

        final_results
    }

    /// Process a specific funding type for a program
    fn process_funding_type(
        &self,
        records: &[StudentRecord],
        excluded_normalized_snils: &mut std::collections::HashSet<String>,
    ) -> Vec<String> {
        if records.is_empty() {
            return Vec::new();
        }
        
        let available_places = records[0].available_places as usize;
        
        // Filter for eager applicants who haven't been excluded yet, and ensure uniqueness
        let mut seen_snils = std::collections::HashSet::new();
        let mut eligible_applicants = Vec::new();
        
        for record in records {
            let normalized_snils = normalize_snils(&record.snils);
            
            // Skip if already excluded or already seen in this iteration
            if excluded_normalized_snils.contains(&normalized_snils) || seen_snils.contains(&normalized_snils) {
                continue;
            }
            
            // Check if eligible (has original document OR consent)
            if record.has_original_document() || record.has_consent() {
                eligible_applicants.push(record);
                seen_snils.insert(normalized_snils);
                
                // Stop when we have enough unique applicants
                if eligible_applicants.len() >= available_places {
                    break;
                }
            }
        }

        // If we don't have enough eager applicants, fill remaining places with other applicants
        if eligible_applicants.len() < available_places {
            for record in records {
                let normalized_snils = normalize_snils(&record.snils);
                
                // Skip if already excluded, already seen, or already selected
                if excluded_normalized_snils.contains(&normalized_snils) || seen_snils.contains(&normalized_snils) {
                    continue;
                }
                
                // Add non-eager applicants to fill remaining places
                if !record.has_original_document() && !record.has_consent() {
                    eligible_applicants.push(record);
                    seen_snils.insert(normalized_snils);
                    
                    // Stop when we have enough unique applicants
                    if eligible_applicants.len() >= available_places {
                        break;
                    }
                }
            }
        }

        // Admit these applicants and exclude them from further consideration
        let mut admitted = Vec::new();
        for applicant in eligible_applicants {
            admitted.push(applicant.snils.clone());
            let normalized = normalize_snils(&applicant.snils);
            excluded_normalized_snils.insert(normalized);
        }

        admitted
    }

    /// Calculate program popularity metrics
    fn calculate_program_popularity(&self, program_name: &str, records: &[StudentRecord]) -> ProgramPopularity {
        let available_places = records[0].available_places;
        
        // Count all applications
        let total_applications = records.len();
        
        // Filter for eager applicants (have original document OR consent)
        let eager_applicants: Vec<StudentRecord> = records
            .iter()
            .filter(|record| record.has_original_document() || record.has_consent())
            .cloned()
            .collect();

        let total_applicants = eager_applicants.len();
        let applications_per_place = total_applicants as f64 / available_places as f64;

        // Calculate average score of top subset (available_places * 2 or fewer if not enough)
        let top_count = std::cmp::min(available_places as usize * 2, eager_applicants.len());
        let top_scores: Vec<f64> = eager_applicants
            .iter()
            .take(top_count)
            .filter_map(|record| record.get_numeric_score())
            .collect();

        let top_third_average_score = if top_scores.is_empty() {
            0.0
        } else {
            top_scores.iter().sum::<f64>() / top_scores.len() as f64
        };

        ProgramPopularity {
            program_name: program_name.to_string(),
            applications_per_place,
            top_third_average_score,
            available_places,
            total_applications,
            total_applicants,
            eager_applicants,
        }
    }

    /// Simulate the admission process following school rules
    fn simulate_admission_process(
        &self,
        program_popularities: &[ProgramPopularity],
    ) -> HashMap<String, Vec<String>> {
        let mut final_results = HashMap::new();
        let mut excluded_normalized_snils = std::collections::HashSet::new();

        // Process programs from most to least popular
        for popularity in program_popularities {
            let mut admitted_to_this_program = Vec::new();
            let available_places = popularity.available_places as usize;

            // Take eligible applicants who haven't been excluded yet
            let eligible_applicants: Vec<&StudentRecord> = popularity
                .eager_applicants
                .iter()
                .filter(|record| {
                    let normalized = normalize_snils(&record.snils);
                    !excluded_normalized_snils.contains(&normalized)
                })
                .take(available_places)
                .collect();

            // Admit these applicants and exclude them from further consideration
            for applicant in eligible_applicants {
                admitted_to_this_program.push(applicant.snils.clone());
                let normalized = normalize_snils(&applicant.snils);
                excluded_normalized_snils.insert(normalized);
            }

            final_results.insert(popularity.program_name.clone(), admitted_to_this_program);
        }

        final_results
    }

    /// Check target applicant results
    fn check_target_applicant_results(
        &self,
        final_results: &HashMap<String, Vec<String>>,
        all_program_records: &[(String, Vec<StudentRecord>)],
    ) -> (bool, Vec<(String, bool)>) {
        let mut target_results = Vec::new();
        let mut target_found_anywhere = false;
        let normalized_target = normalize_snils(&self.target_snils);
        let mut processed_programs = std::collections::HashSet::new();

        // First, check if the target applicant exists in any program data at all
        for (program_name, records) in all_program_records {
            // Skip if we've already processed this program name
            if processed_programs.contains(program_name) {
                continue;
            }
            processed_programs.insert(program_name.clone());
            
            let exists_in_program = records.iter()
                .any(|record| normalize_snils(&record.snils) == normalized_target);
            
            if exists_in_program {
                target_found_anywhere = true;
                
                // Check if they were admitted to this program
                let admitted = final_results.get(program_name)
                    .map(|admitted_list| {
                        admitted_list.iter()
                            .any(|snils| normalize_snils(snils) == normalized_target)
                    })
                    .unwrap_or(false);
                
                target_results.push((program_name.clone(), admitted));
                
                // Print detailed information about the applicant's position
                if exists_in_program {
                    self.print_applicant_details(program_name, records);
                }
            }
        }

        (target_found_anywhere, target_results)
    }

    /// Print detailed information about the applicant's position in a program
    fn print_applicant_details(&self, program_name: &str, records: &[StudentRecord]) {
        let normalized_target = normalize_snils(&self.target_snils);
        
        println!("\n📋 Detailed position for program '{}':", program_name);
        
        for record in records {
            if normalize_snils(&record.snils) == normalized_target {
                println!("   • Rank: {} ({} funding, {} places available)", 
                         record.rank, 
                         record.funding_source,
                         record.available_places);
                println!("   • Score: {}", record.average_score);
                println!("   • Priority: {}", record.priority);
                println!("   • Document: {}, Consent: {}", 
                         record.document_type, 
                         record.consent);
            }
        }
    }

    /// Generate chance analysis for target applicant
    pub fn analyze_target_chances(&self, analysis: &AdmissionAnalysis) -> ChanceAnalysis {
        let mut programs_admitted_to = Vec::new();
        let mut programs_rejected_from = Vec::new();

        for (program_name, admitted) in &analysis.target_applicant_results {
            if *admitted {
                programs_admitted_to.push(program_name.clone());
            } else {
                programs_rejected_from.push(program_name.clone());
            }
        }

        let final_recommendation = self.generate_recommendation(
            &programs_admitted_to,
            &programs_rejected_from,
            analysis,
        );

        ChanceAnalysis {
            target_snils: self.target_snils.clone(),
            programs_admitted_to,
            programs_rejected_from,
            final_recommendation,
        }
    }

    fn generate_recommendation(
        &self,
        admitted_programs: &[String],
        _rejected_programs: &[String],
        analysis: &AdmissionAnalysis,
    ) -> String {
        if !analysis.target_applicant_found {
            return format!(
                "Абитуриент с СНИЛС '{}' не найден в списках поступающих на интересующие программы.",
                self.target_snils
            );
        }

        if admitted_programs.is_empty() {
            let mut recommendation = "К сожалению, шансы на поступление на данные программы низкие.\n".to_string();
            
            // Find the most accessible program and suggest improvements
            if let Some(least_popular) = analysis.program_popularities
                .iter()
                .min_by(|a, b| a.applications_per_place.partial_cmp(&b.applications_per_place).unwrap())
            {
                recommendation.push_str(&format!(
                    "Рекомендации:\n\
                     - Программа '{}' менее конкурентная ({:.1} заявок на место)\n\
                     - Средний балл лучших кандидатов: {:.2}\n\
                     - Рассмотрите возможность улучшения документов или согласия на зачисление",
                    least_popular.program_name,
                    least_popular.applications_per_place,
                    least_popular.top_third_average_score
                ));
            }
            
            recommendation
        } else if admitted_programs.len() == 1 {
            format!(
                "Поздравляем! Высокие шансы на поступление на программу '{}'.\n\
                 Рекомендуется подтвердить намерения подачей согласия на зачисление.",
                admitted_programs[0]
            )
        } else {
            format!(
                "Отличные результаты! Есть шансы на поступление на несколько программ: {}.\n\
                 Выберите наиболее приоритетную программу для подачи согласия на зачисление.",
                admitted_programs.join(", ")
            )
        }
    }
}
