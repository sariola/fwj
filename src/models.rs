use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;
use dirs;

// Constants
pub const LLAMAFILE_URL: &str =
    "https://huggingface.co/sariola/flow-judge-llamafile/resolve/main/flow-judge.llamafile";
pub const LLAMAFILE_LOCK_URL: &str =
    "https://huggingface.co/sariola/flow-judge-llamafile/raw/main/flow-judge.llamafile";
pub const MAX_RETRIES: u32 = 3;
pub const SCORE_REGEX_PATTERN: &str = r"<score>\s*(\d+)\s*</score>";
pub const FEEDBACK_REGEX_PATTERN: &str = r"(?s)<feedback>(.+?)</feedback>";
pub const RUBRICS_DIR: &str = "./rubrics";
pub const DATA_DIR: &str = "./data";
pub const DATA_URL: &str = "https://raw.githubusercontent.com/sariola/fwj/refs/heads/main/data/subquery-data.json";
pub const RUBRIC_URL: &str = "https://raw.githubusercontent.com/sariola/fwj/refs/heads/main/rubrics/subquery-decomp.jinja";

lazy_static! {
    pub static ref FILE_LOCKS: Mutex<HashMap<String, Mutex<()>>> = Mutex::new(HashMap::new());
    pub static ref SCORE_REGEX: Regex = Regex::new(SCORE_REGEX_PATTERN).unwrap();
    pub static ref FEEDBACK_REGEX: Regex = Regex::new(FEEDBACK_REGEX_PATTERN).unwrap();
}


#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("CSV write error: {0}")]
    CsvWriteError(String),
    #[error("CSV read error: {0}")]
    CsvReadError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Template error: {0}")]
    TemplateError(#[from] minijinja::Error),
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
    #[error("Command execution failed: {0}")]
    CommandExecutionError(String),
    #[error("JSON parse error: Invalid structure")]
    JsonParseError(String),
    #[error("JSON write error: Invalid structure")]
    JsonWriteError(String),
    #[error("File read error: {0}")]
    FileReadError(String),
    #[error("File write error: {0}")]
    FileWriteError(String), // Add this line
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yml::Error),
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("{0}")]
    CustomError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Download error: {0}")]
    DownloadError(String),
    #[error("CSV parse error: {0}")]
    CsvParseError(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tasks: Vec<TaskConfig>,
    #[serde(default = "default_llamafile_url")]
    pub llamafile_url: String,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
    #[serde(default = "default_rubrics_dir")]
    pub rubrics_dir: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

pub fn default_llamafile_url() -> String { LLAMAFILE_URL.to_string() }
pub fn default_cache_dir() -> String {
    dirs::cache_dir()
        .map(|cache| cache.join("fwj"))
        .unwrap_or_else(|| {
            if cfg!(windows) {
                std::env::var("LOCALAPPDATA")
                    .map(|appdata| PathBuf::from(appdata).join("cache").join("fwj"))
                    .unwrap_or_else(|_| PathBuf::from(".fwj"))
            } else {
                std::env::var("HOME")
                    .map(|home| PathBuf::from(home).join(".cache").join("fwj"))
                    .unwrap_or_else(|_| PathBuf::from(".fwj"))
            }
        })
        .to_string_lossy()
        .into_owned()
}
pub fn default_rubrics_dir() -> String { RUBRICS_DIR.to_string() }
pub fn default_data_dir() -> String { DATA_DIR.to_string() }

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskConfig {
    pub data: String,
    pub rubric_template: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IoItem {
    pub input: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,
}
