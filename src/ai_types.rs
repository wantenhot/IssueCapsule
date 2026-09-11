use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiReproductionPlan {
    pub runtime: Option<String>,
    pub python_version: Option<String>,
    pub install_command: Option<String>,
    pub reproduction_command: String,
    pub expected_error: Option<String>,
    #[serde(default)]
    pub generated_files: Vec<GeneratedFile>,
    pub confidence: u8,
    pub reasoning_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiIssueContext {
    pub repository: String,
    pub detected_language: String,
    pub root_files: Vec<String>,
    pub issue_title: String,
    pub issue_body: String,
    pub requirements: Option<String>,
    pub pyproject: Option<String>,
    pub readme: Option<String>,
    pub code_blocks: Vec<String>,
}
