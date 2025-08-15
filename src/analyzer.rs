use crate::models::{StudentRecord, normalize_snils, ApplicantApplication, EagerApplicant};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ProgramPopularity {
    pub program_name: String,
    pub program_key: String, // program_name + funding_source for uniqueness
    pub funding_source: String,
    pub top_candidates_average_priority: f64,
    pub average_score: f64,
    pub available_places: u32,
    pub total_eager_applicants: usize,
    pub eager_applicants: Vec<StudentRecord>,
}

#[derive(Debug, Clone)]
pub struct AdmissionAnalysis {
    pub program_popularities: Vec<ProgramPopularity>,
    pub final_admission_results: HashMap<String, Vec<String>>, // program_key -> admitted SNILSes
    pub target_applicant_found: bool,
    pub target_applicant_results: Vec<(String, bool)>, // (program_key, admitted)
}
    
pub struct AdmissionAnalyzer<'a> {
    pub target_snils: &'a str,
}

impl<'a> AdmissionAnalyzer<'a> {
    pub fn new(target_snils: &'a str) -> Self {
        Self {
            target_snils, 
        }
    }

    /// Main analysis function following the new priority-based logic
    pub fn analyze_all_programs(&self, all_program_records: &Vec<(String, Vec<StudentRecord>)>) -> AdmissionAnalysis {
        // Step 1: Create program-funding combinations and calculate popularity
        let program_popularities = self.calculate_all_program_popularities(all_program_records);
        
        // Step 2: Prepare eager applicants with their applications ordered by priority
        let eager_applicants = self.prepare_eager_applicants(all_program_records);
        
        // Step 3: Sort eager applicants by score descending then average rank ascending
        let mut sorted_eager_applicants = eager_applicants;
        sorted_eager_applicants.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.average_rank.partial_cmp(&b.average_rank).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Step 4: Simulate admission process using the new priority-based algorithm
        let final_admission_results = self.simulate_priority_based_admission(&program_popularities, &sorted_eager_applicants);
        
        // Step 5: Check if target applicant was found and their results
        let (target_found, target_results) = self.check_target_applicant_results(&final_admission_results, &all_program_records);

        AdmissionAnalysis {
            program_popularities,
            final_admission_results,
            target_applicant_found: target_found,
            target_applicant_results: target_results,
        }
    }

    /// Calculate popularity for all program-funding combinations
    fn calculate_all_program_popularities(&self, all_program_records: &[(String, Vec<StudentRecord>)]) -> Vec<ProgramPopularity> {
        let mut popularities = Vec::new();
        
        // Group by program-funding combinations
        let mut program_funding_combinations: HashMap<String, (String, String, Vec<StudentRecord>)> = HashMap::new();
        
        for (program_name, records) in all_program_records {
            for record in records {
                let program_key = format!("{}_{}", program_name, record.funding_source);
                program_funding_combinations
                    .entry(program_key.clone())
                    .or_insert_with(|| (program_name.clone(), record.funding_source.clone(), Vec::new()))
                    .2
                    .push(record.clone());
            }
        }
        
        // Calculate popularity for each combination
        for (program_key, (program_name, funding_source, records)) in program_funding_combinations {
            let popularity = self.calculate_program_popularity(&program_name, &funding_source, &program_key, &records);
            popularities.push(popularity);
        }
        
        // Sort by average priority (lower is more popular)
        popularities.sort_by(|a, b| a.top_candidates_average_priority.partial_cmp(&b.top_candidates_average_priority).unwrap_or(std::cmp::Ordering::Equal));
        
        popularities
    }

    /// Calculate program popularity metrics based on new criteria
    fn calculate_program_popularity(&self, program_name: &str, funding_source: &str, program_key: &str, records: &[StudentRecord]) -> ProgramPopularity {
        let available_places = records[0].available_places;
        
        // Filter for eager applicants (have original document OR consent)
        let mut eager_applicants: Vec<StudentRecord> = records
            .iter()
            .filter(|record| record.has_original_document() || record.has_consent())
            .cloned()
            .collect();
        
        // Sort eager applicants by rank (best rank first - ascending order)
        eager_applicants.sort_by_key(|record| record.rank);

        let total_eager_applicants = eager_applicants.len();

        // Calculate average priority of top candidates (available_places * 2 or fewer if not enough)
        let top_count = std::cmp::min(available_places as usize * 2, eager_applicants.len());
        let top_priorities: Vec<u32> = eager_applicants
            .iter()
            .take(top_count)
            .map(|record| record.priority)
            .collect();

        let top_candidates_average_priority = if top_priorities.is_empty() {
            0.0
        } else {
            top_priorities.iter().sum::<u32>() as f64 / top_priorities.len() as f64
        };

        // Calculate average score of all eager applicants
        let all_scores: Vec<f64> = eager_applicants
            .iter()
            .filter_map(|record| record.get_numeric_score())
            .collect();

        let average_score = if all_scores.is_empty() {
            0.0
        } else {
            all_scores.iter().sum::<f64>() / all_scores.len() as f64
        };

        ProgramPopularity {
            program_name: program_name.to_string(),
            program_key: program_key.to_string(),
            funding_source: funding_source.to_string(),
            top_candidates_average_priority,
            average_score,
            available_places,
            total_eager_applicants,
            eager_applicants,
        }
    }

    /// Prepare eager applicants with their applications sorted by priority
    fn prepare_eager_applicants(&self, all_program_records: &[(String, Vec<StudentRecord>)]) -> Vec<EagerApplicant> {
        let mut applicant_map: HashMap<String, Vec<ApplicantApplication>> = HashMap::new();

        // Collect all applications for each applicant
        for (program_name, records) in all_program_records {
            for record in records {
                // Only consider eager applicants
                if record.has_original_document() || record.has_consent() {
                    let normalized_snils = normalize_snils(&record.snils);
                    let program_key = format!("{}_{}", program_name, record.funding_source);
                    
                    let application = ApplicantApplication {
                        snils: record.snils.clone(),
                        program_key,
                        program_name: program_name.clone(),
                        funding_source: record.funding_source.clone(),
                        priority: record.priority,
                        score: record.get_numeric_score().unwrap_or(0.0),
                        rank: record.rank,
                        has_consent: record.has_consent(),
                        has_original_document: record.has_original_document(),
                    };
                    
                    applicant_map
                        .entry(normalized_snils.clone())
                        .or_insert_with(Vec::new)
                        .push(application);

                }
            }
        }
        
        // Create eager applicants with sorted applications
        let mut eager_applicants = Vec::new();
        for (snils, mut applications) in applicant_map {
            // Sort applications by priority (ascending - lower priority number is more preferred)
            applications.sort_by_key(|app| app.priority);
            
            // Calculate average rank across all applications
            let average_rank = applications.iter().map(|app| app.rank as f64).sum::<f64>() / applications.len() as f64;

            let score = applications.iter().map(|app| app.score).sum::<f64>() / applications.len() as f64;

            eager_applicants.push(EagerApplicant {
                snils,
                applications,
                average_rank,
                score,
            });
        }
        
        eager_applicants
    }

    /// Simulate admission process using priority-based algorithm
    fn simulate_priority_based_admission(
        &self,
        program_popularities: &[ProgramPopularity],
        sorted_eager_applicants: &[EagerApplicant],
    ) -> HashMap<String, Vec<String>> {
        let mut admission_lists: HashMap<String, Vec<String>> = HashMap::new();
        let mut admitted_applicants: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        // Initialize admission lists
        for popularity in program_popularities {
            admission_lists.insert(popularity.program_key.clone(), Vec::new());
        }
        
        // Iterate through applicants in order of average rank
        for applicant in sorted_eager_applicants {
            let normalized_snils = normalize_snils(&applicant.snils);
            
            // Skip if already admitted to any program
            if admitted_applicants.contains(&normalized_snils) {
                continue;
            }
            
            // Try to admit to programs in order of applicant's priority
            for application in &applicant.applications {
                let program_key = &application.program_key;
                
                // Find the program's available places
                let available_places = program_popularities
                    .iter()
                    .find(|p| p.program_key == *program_key)
                    .map(|p| p.available_places)
                    .unwrap_or(0);

                if normalized_snils == normalize_snils(self.target_snils) {
                    println!("Processing applicant: {} for program: {}", normalized_snils, program_key);
                }

                // Check if admission list is not full
                if let Some(admission_list) = admission_lists.get_mut(program_key) {
                    if normalized_snils == normalize_snils(self.target_snils) {
                        println!("Admission list length: {} available: {}", admission_list.len(), available_places);

                        let mut snils_str = String::new();
                        for admitted_snils in admission_list.clone() {
                            if (!snils_str.is_empty()) {
                                snils_str.push_str(", ");
                            }
                            if normalize_snils(&admitted_snils) == normalized_snils {
                                snils_str.push_str(&format!("*{}*", admitted_snils));
                            } else {
                                snils_str.push_str(&format!("{}", admitted_snils));
                            }
                        }
                        println!("{}", snils_str);
                    }
                    if admission_list.len() < available_places as usize {
                        // Admit the applicant and mark as admitted
                        admission_list.push(application.snils.clone());
                        admitted_applicants.insert(normalized_snils.clone());


                        if normalized_snils != normalize_snils(self.target_snils) {
                            // Move to next applicant if not the target applicant
                            break;
                        } else {
                            println!("Admitted target applicant: {}", normalized_snils);
                        }
                    }
                }
            }
        }
        
        admission_lists
    }

    /// Check if target applicant was found and their results
    fn check_target_applicant_results(
        &self,
        final_admission_results: &HashMap<String, Vec<String>>,
        all_program_records: &[(String, Vec<StudentRecord>)],
    ) -> (bool, Vec<(String, bool)>) {
        let normalized_target = normalize_snils(self.target_snils);
        let mut target_found = false;
        let mut target_results = Vec::new();
        
        // Check all programs the target applied to
        for (program_name, records) in all_program_records {
            for record in records {
                if normalize_snils(&record.snils) == normalized_target {
                    target_found = true;
                    let program_key = format!("{}_{}", program_name, record.funding_source);
                    
                    let admitted = final_admission_results
                        .get(&program_key)
                        .map(|list| list.iter().any(|snils| normalize_snils(snils) == normalized_target))
                        .unwrap_or(false);
                    
                    target_results.push((program_key, admitted));
                }
            }
        }
        
        (target_found, target_results)
    }

    /// Public method to group records by program and funding type (for reporting)
    pub fn group_by_program_and_funding_public(
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
}
