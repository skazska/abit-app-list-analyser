use crate::models::{ProgramInfo, StudentRecord};
use anyhow::{Context, Result};
use regex::Regex;
use scraper::{Html, Selector};
use std::fs;

pub struct AdmissionScraper {
    client: reqwest::Client,
}

impl AdmissionScraper {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn scrape_file(&self, file_path: &str) -> Result<Vec<(ProgramInfo, Vec<StudentRecord>)>> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path))?;

        self.parse_html_content(&content, Some(file_path))
    }

    pub async fn scrape_url(&self, url: &str) -> Result<Vec<(ProgramInfo, Vec<StudentRecord>)>> {
        println!("🌐 Fetching data from: {}", url);
        
        let response = self.client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {}", url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP request failed with status: {}", response.status()));
        }

        let content = response.text().await
            .with_context(|| format!("Failed to read response body from: {}", url))?;

        // Look for the data-wrap div specifically
        let document = Html::parse_document(&content);
        let data_wrap_selector = Selector::parse("div.data-wrap").unwrap();
        
        if let Some(data_wrap) = document.select(&data_wrap_selector).next() {
            // Create a new document from just the data-wrap content
            let data_wrap_html = data_wrap.html();
            println!("   ✅ Found data-wrap section ({} chars)", data_wrap_html.len());
            self.parse_html_content(&data_wrap_html, Some(url))
        } else {
            println!("   ⚠️  No data-wrap section found, parsing entire document");
            self.parse_html_content(&content, Some(url))
        }
    }

    fn parse_html_content(&self, content: &str, source: Option<&str>) -> Result<Vec<(ProgramInfo, Vec<StudentRecord>)>> {
        let document = Html::parse_document(content);
        
        let programs = self.extract_all_programs(&document)?;
        
        if let Some(src) = source {
            if programs.is_empty() {
                println!("   ⚠️  Warning: No programs found in {}", src);
            }
        }

        Ok(programs)
    }

    fn extract_all_programs(&self, document: &Html) -> Result<Vec<(ProgramInfo, Vec<StudentRecord>)>> {
        let mut programs = Vec::new();
        
        // Find all program name elements
        let strong_selector = Selector::parse("p > strong").unwrap();
        let program_elements: Vec<_> = document.select(&strong_selector).collect();
        
        for (i, program_element) in program_elements.iter().enumerate() {
            let program_name = program_element.text().collect::<String>().trim().to_string();
            
            // Skip if this doesn't look like a program name
            if !program_name.starts_with("ОП СПО") {
                continue;
            }
            
            // Find the containing div and extract program info
            if let Some(program_parent) = program_element.parent()
                .and_then(|p| p.parent()) 
            {
                // Convert back to ElementRef for the div
                let program_div = scraper::ElementRef::wrap(program_parent).unwrap();
                let program_info = self.extract_program_info_from_div(program_div, &program_name)?;
                
                // Find the table that follows this program info
                let table_records = self.extract_records_for_program(document, i, &program_info)?;
                
                if !table_records.is_empty() {
                    programs.push((program_info, table_records));
                }
            }
        }
        
        Ok(programs)
    }

    fn extract_program_info_from_div(&self, div_element: scraper::ElementRef, program_name: &str) -> Result<ProgramInfo> {
        let div_html = div_element.html();
        
        // Extract funding source
        let funding_regex = Regex::new(r"Источник финансирования:\s*<i>([^<]+)</i>").unwrap();
        let funding_source = funding_regex
            .captures(&div_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Extract study form
        let form_regex = Regex::new(r"Форма обучения:\s*<i>([^<]+)</i>").unwrap();
        let study_form = form_regex
            .captures(&div_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Extract available places
        let places_regex = Regex::new(r"Количество мест:\s*<i>(\d+)</i>").unwrap();
        let available_places = places_regex
            .captures(&div_html)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(0);

        Ok(ProgramInfo {
            name: program_name.to_string(),
            funding_source,
            study_form,
            available_places,
        })
    }

    fn extract_records_for_program(
        &self,
        document: &Html,
        program_index: usize,
        program_info: &ProgramInfo,
    ) -> Result<Vec<StudentRecord>> {
        // Find all tables in the document
        let table_selector = Selector::parse("table.table-bordered").unwrap();
        let tables: Vec<_> = document.select(&table_selector).collect();
        
        // Try to get the table that corresponds to this program
        let table = if let Some(table) = tables.get(program_index) {
            table
        } else {
            return Ok(Vec::new());
        };
        
        let row_selector = Selector::parse("tbody tr.srt").unwrap();
        let mut records = Vec::new();

        for row in table.select(&row_selector) {
            let cells: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
            
            if cells.len() < 8 {
                continue; // Skip incomplete rows
            }

            // Extract data from each cell
            let rank = cells[0]
                .text()
                .collect::<String>()
                .trim()
                .parse::<u32>()
                .unwrap_or(0);

            let snils = self.extract_snils(&cells[2]);
            let priority = self.extract_priority(&cells[3]);
            let consent = cells[4].text().collect::<String>().trim().to_string();
            let document_type = cells[5].text().collect::<String>().trim().to_string();
            let average_score = cells[6].text().collect::<String>().trim().to_string();
            let subject_scores = cells[7].text().collect::<String>().trim().to_string();
            let psychological_test = if cells.len() > 8 {
                cells[8].text().collect::<String>().trim().to_string()
            } else {
                "-".to_string()
            };

            records.push(StudentRecord {
                rank,
                snils,
                priority,
                consent,
                document_type,
                average_score,
                subject_scores,
                psychological_test,
                program_name: program_info.name.clone(),
                funding_source: program_info.funding_source.clone(),
                study_form: program_info.study_form.clone(),
                available_places: program_info.available_places,
            });
        }

        Ok(records)
    }

    fn extract_program_info(&self, document: &Html) -> Result<ProgramInfo> {
        let selector = Selector::parse("div > p > strong").unwrap();
        let program_name = document
            .select(&selector)
            .next()
            .and_then(|el| el.text().next())
            .unwrap_or("Unknown Program")
            .to_string();

        // Extract funding source
        let funding_regex = Regex::new(r"Источник финансирования:\s*<i>([^<]+)</i>").unwrap();
        let funding_source = funding_regex
            .captures(&document.html())
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Extract study form
        let form_regex = Regex::new(r"Форма обучения:\s*<i>([^<]+)</i>").unwrap();
        let study_form = form_regex
            .captures(&document.html())
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Extract available places
        let places_regex = Regex::new(r"Количество мест:\s*<i>(\d+)</i>").unwrap();
        let available_places = places_regex
            .captures(&document.html())
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(0);

        Ok(ProgramInfo {
            name: program_name,
            funding_source,
            study_form,
            available_places,
        })
    }

    fn extract_student_records(
        &self,
        document: &Html,
        program_info: &ProgramInfo,
    ) -> Result<Vec<StudentRecord>> {
        let row_selector = Selector::parse("tbody tr.srt").unwrap();
        let mut records = Vec::new();

        for row in document.select(&row_selector) {
            let cells: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
            
            if cells.len() < 8 {
                continue; // Skip incomplete rows
            }

            // Extract data from each cell
            let rank = cells[0]
                .text()
                .collect::<String>()
                .trim()
                .parse::<u32>()
                .unwrap_or(0);

            let snils = self.extract_snils(&cells[2]);
            let priority = self.extract_priority(&cells[3]);
            let consent = cells[4].text().collect::<String>().trim().to_string();
            let document_type = cells[5].text().collect::<String>().trim().to_string();
            let average_score = cells[6].text().collect::<String>().trim().to_string();
            let subject_scores = cells[7].text().collect::<String>().trim().to_string();
            let psychological_test = if cells.len() > 8 {
                cells[8].text().collect::<String>().trim().to_string()
            } else {
                "-".to_string()
            };

            records.push(StudentRecord {
                rank,
                snils,
                priority,
                consent,
                document_type,
                average_score,
                subject_scores,
                psychological_test,
                program_name: program_info.name.clone(),
                funding_source: program_info.funding_source.clone(),
                study_form: program_info.study_form.clone(),
                available_places: program_info.available_places,
            });
        }

        Ok(records)
    }

    fn extract_snils(&self, cell: &scraper::ElementRef) -> String {
        let full_text = cell.text().collect::<String>();
        
        // Try to find SNILS in the text
        // It could be in the first line or after "СНИЛС: "
        for line in full_text.lines() {
            let line = line.trim();
            if line.starts_with("СНИЛС: ") {
                return line.replace("СНИЛС: ", "").trim().to_string();
            }
            // If this line looks like a SNILS (contains alphanumeric with dashes)
            if line.len() > 3 && (line.contains('-') || line.chars().any(|c| c.is_alphanumeric())) {
                // Take only the first part before any additional text
                if let Some(space_pos) = line.find(' ') {
                    let potential_snils = &line[..space_pos];
                    if potential_snils.len() > 5 {
                        return potential_snils.to_string();
                    }
                } else if line.len() > 5 {
                    return line.to_string();
                }
            }
        }
        
        // Fallback: take the first non-empty line
        full_text.lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn extract_priority(&self, cell: &scraper::ElementRef) -> u32 {
        cell.text()
            .collect::<String>()
            .trim()
            .parse::<u32>()
            .unwrap_or(0)
    }
}
