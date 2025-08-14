use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub target_snils: String,
    pub programs_of_interest: Option<Vec<String>>,
    pub target_funding_types: Vec<String>,
    // Data source configuration
    pub data_source_mode: DataSourceMode,
    pub data_directory: Option<String>,
    pub internet_urls: Option<Vec<String>>,
    pub output_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSourceMode {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "internet")]
    Internet,
    #[serde(rename = "both")]
    Both,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_snils: "".to_string(),
            programs_of_interest: Some(vec![
                "ОП СПО Лечебное дело".to_string(),
                "ОП СПО Фармация".to_string(),
            ]),
            target_funding_types: vec![
                "Бюджетное финансирование".to_string(),
                // Note: Comment out commercial funding to only analyze budget funding
                // "Коммерческое финансирование".to_string(),
            ],
            data_source_mode: DataSourceMode::Local,
            data_directory: Some("data-source".to_string()),
            internet_urls: Some(vec![
                "https://example.com/admission-list1".to_string(),
                "https://example.com/admission-list2".to_string(),
            ]),
            output_directory: Some("output".to_string()),
        }
    }
}

impl Config {
    pub fn load_from_file(file_path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(file_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file(&self, file_path: &str) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(file_path, content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentRecord {
    pub rank: u32,
    pub snils: String,
    pub priority: u32,
    pub consent: String,
    pub document_type: String,
    pub average_score: String,
    pub subject_scores: String,
    pub psychological_test: String,
    pub program_name: String,
    pub funding_source: String,
    pub study_form: String,
    pub available_places: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicantApplication {
    pub snils: String,
    pub program_key: String, // program_name + funding_source for uniqueness
    pub program_name: String,
    pub funding_source: String,
    pub priority: u32,
    pub rank: u32,
    pub average_score: f64,
    pub has_consent: bool,
    pub has_original_document: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EagerApplicant {
    pub snils: String,
    pub applications: Vec<ApplicantApplication>, // sorted by priority
    pub average_rank: f64, // average rank across all applications
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramInfo {
    pub name: String,
    pub funding_source: String,
    pub study_form: String,
    pub available_places: u32,
}

impl StudentRecord {
    pub fn get_numeric_score(&self) -> Option<f64> {
        self.average_score
            .replace(',', ".")
            .parse::<f64>()
            .ok()
    }

    pub fn has_consent(&self) -> bool {
        self.consent.to_lowercase().contains("да")
    }

    pub fn has_original_document(&self) -> bool {
        self.document_type.to_lowercase().contains("да")
    }

    pub fn get_normalized_snils(&self) -> String {
        normalize_snils(&self.snils)
    }
}

/// Normalize SNILS by keeping only alphanumeric characters
pub fn normalize_snils(snils: &str) -> String {
    snils.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}
