#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[cfg(test)]
mod tests;
mod models;
mod download;
mod cli;

use models::{AppError, Config, TaskConfig, IoItem};
use models::{FILE_LOCKS, SCORE_REGEX, RUBRICS_DIR, MAX_RETRIES};
use models::{DATA_URL, RUBRIC_URL, DATA_DIR};
use std::path::Path;

use crate::download::{download_flow_judge_llamafile, download_file};

use log::{debug, error, info, warn};
use minijinja::{context, Environment};
use serde_json::{self, Value};
use serde_json::from_str as json_from_str;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::thread;
use tokio::fs;
use tokio::sync::Mutex;
use regex::Regex;
use env_logger::Env;
use futures::stream::{self, StreamExt};
use console::{style, Term, Style};
use textwrap::wrap;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Instant;
use std::sync::Arc;
use std::fs::File;
use csv::{Reader, Writer, QuoteStyle};
use tokio::io::AsyncReadExt;
use csv::{WriterBuilder, ReaderBuilder};
use std::io::{Read, Write, BufReader, BufWriter};
use std::str::from_utf8;
use serde::{Serialize, Deserialize};
use std::env::args;
use crate::cli::Args;
use clap_complete::{generate, shells::Bash, shells::Fish, shells::Zsh, shells::Elvish, shells::PowerShell};
use std::io;
use clap::CommandFactory;

impl Config {
    pub fn from_file(path: &str) -> Result<Self, AppError> {
        let config_str = std::fs::read_to_string(path)
            .map_err(|e| AppError::ConfigError(format!("Failed to read config file {}: {}", path, e)))?;

        let config: Config = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yml::from_str(&config_str)
                .map_err(|e| AppError::ConfigError(format!("Failed to parse YAML config file {}: {}", path, e)))?
        } else if path.ends_with(".json") {
            serde_json::from_str(&config_str)
                .map_err(|e| AppError::ConfigError(format!("Failed to parse JSON config file {}: {}", path, e)))?
        } else {
            return Err(AppError::ConfigError(format!("Unsupported config file format: {}", path)));
        };

        Ok(config)
    }
}

fn display_last_result(result: &str) {
    let term = Term::stdout();
    let width = term.size().1 as usize;

    println!("\n{}", style("Last Processed Result:\n\n").bold().underlined());

    if result.trim().is_empty() {
        println!("{}", style("No result to display.").yellow());
        return;
    }
    println!("{}", style(result).green().bright());
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = cli::parse_args();

    if let Some(cli::Commands::GenAutoCompletions { shell, output }) = args.command {
        let mut cmd = cli::Args::command();
        let mut buf = Vec::new();

        match shell {
            cli::Shell::Fish => {
                generate(clap_complete::shells::Fish, &mut cmd, "fwj", &mut buf);
            }
            cli::Shell::Bash => {
                generate(clap_complete::shells::Bash, &mut cmd, "fwj", &mut buf);
            }
            cli::Shell::Zsh => {
                generate(clap_complete::shells::Zsh, &mut cmd, "fwj", &mut buf);
            }
            cli::Shell::Elvish => {
                generate(clap_complete::shells::Elvish, &mut cmd, "fwj", &mut buf);
            }
            cli::Shell::PowerShell => {
                generate(clap_complete::shells::PowerShell, &mut cmd, "fwj", &mut buf);
            }
        }

        if let Some(path) = output {
            File::create(&path)
                .map_err(|e| AppError::FileWriteError(format!("Failed to create file '{}': {}", path, e)))?
                .write_all(&buf)
                .map_err(|e| AppError::FileWriteError(format!("Failed to write to file '{}': {}", path, e)))?;
            println!("Completions written to {}", path);
        } else {
            io::stdout().write_all(&buf)
                .map_err(|e| AppError::FileWriteError(format!("Failed to write to stdout: {}", e)))?;
        }

        return Ok(());
    }

    // Check if both verbose and log-level are specified
    if args.verbose && args.log_level != log::LevelFilter::Info {
        return Err(AppError::ConfigError(
            "Cannot use both --verbose and --log-level options simultaneously".to_string()
        ));
    }

    // Initialize logger
    let log_level = if args.verbose {
        log::LevelFilter::Debug
    } else {
        args.log_level
    };

    env_logger::Builder::from_env(Env::default().default_filter_or(log_level.to_string())).init();

    info!("Starting application");

    let mut config = Config {
        tasks: vec![],
        llamafile_url: models::default_llamafile_url(),
        max_retries: models::default_max_retries(),
        cache_dir: models::default_cache_dir(),
        rubrics_dir: models::default_rubrics_dir(),
        data_dir: models::default_data_dir(),
    };

    // Check if config file exists
    if std::path::Path::new(&args.config).exists() {
        info!("Loading configuration from {}", args.config);
        config = Config::from_file(&args.config)?;
    } else {
        info!("No config file found at {}", args.config);
    }

    // Handle data file
    let data_path = if args.data == "fetch" {
        let path = format!("{}/subquery-data.json", config.data_dir);
        if !Path::new(&path).exists() {
            download_file(DATA_URL, &path).await?;
        }
        path
    } else {
        args.data.clone()
    };

    // Handle rubric file
    let rubric_template = if args.rubric == "fetch" {
        let path = Path::new(&config.rubrics_dir).join("subquery-decomp.jinja");
        if !path.exists() {
            download_file(RUBRIC_URL, path.to_str().unwrap()).await?;
        }
        path.to_str().unwrap().to_string()
    } else {
        args.rubric.clone()
    };

    config.tasks = vec![models::TaskConfig {
        data: data_path,
        rubric_template,
    }];

    // Ensure we have tasks to process
    if config.tasks.is_empty() {
        return Err(AppError::ConfigError("No tasks found in configuration or CLI arguments".to_string()));
    }

    // Download the llamafile and wait for it to complete
    info!("Downloading Flow Judge llamafile");
    download_flow_judge_llamafile(&config).await?;
    info!("Download completed successfully");

    let mut parsing_failures = 0;
    let mut last_result = String::new();

    // Process tasks
    info!("Starting task processing");
    for task_config in &config.tasks {
        info!("Processing task with rubric: {}", task_config.rubric_template);
        match process_task(task_config, &config, args.batch_size, &args).await {
            Ok((failures, result)) => {
                info!("Task with rubric '{}' processed successfully", task_config.rubric_template);
                parsing_failures += failures;
                last_result = result;
            }
            Err(e) => {
                error!("Failed to process task with rubric '{}': {}", task_config.rubric_template, e);
                return Err(e);
            }
        }
    }

    // Save last result to file instead of displaying it
    save_last_result(&last_result, &config.cache_dir)?;

    // Display last result if --last-result flag is used
    if args.last_result {
        let last_result = read_last_result(&config.cache_dir)?;
        display_last_result(&last_result);
    }

    // Display summary
    if parsing_failures == 0 {
        println!("\n{}", style("All items processed successfully.").yellow());
    } else if parsing_failures == 1 {
        println!("\n{}", style(format!("Processing completed with 1 parsing failure.")).yellow());
    } else {
        println!("\n{}", style(format!("Processing completed with {} parsing failures.", parsing_failures)).red());
    }

    info!("All tasks processed. Application completed.");
    Ok(())
}

async fn process_task(task_config: &TaskConfig, config: &Config, batch_size: usize, args: &Args) -> Result<(u32, String), AppError> {
    let data = fs::read_to_string(&task_config.data).await?;

    if data.trim().is_empty() {
        return Err(AppError::CustomError("Data file is empty".to_string()));
    }

    let file_format = detect_file_type(&task_config.data)?;

    let mut items: Vec<IoItem> = match file_format.as_str() {
        "json" => read_json(&task_config.data)?,
        "csv" => read_csv(&task_config.data)?,
        _ => return Err(AppError::ConfigError(format!("Unsupported file format: {}", file_format))),
    };

    let total_items = items.len();
    let concurrent_batch_size = batch_size;

    println!("\n{}", style(format!("Processing: {} entries", total_items)).yellow().bold());
    println!("{}", style(format!("Concurrent batch size: {}", concurrent_batch_size)).yellow().italic());

    println!(); // Add an empty line for spacing

    let multi_progress = Arc::new(MultiProgress::new());

    // Create progress bars for each item upfront
    let item_progress_bars: Vec<ProgressBar> = (0..concurrent_batch_size.min(total_items))
        .map(|i| {
            let pb = multi_progress.add(ProgressBar::new(1));
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap()
                .with_key("elapsed_precise", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
                    write!(w, "{:02}:{:02}:{:03}",
                        state.elapsed().as_secs() / 60,
                        state.elapsed().as_secs() % 60,
                        state.elapsed().subsec_millis()
                    ).unwrap();
                }));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb.set_message(style(format!("Item {} - Waiting", i + 1)).dim().bold().to_string());
            pb
        })
        .collect();

    // Create the main progress bar and add it last
    let main_progress_bar = multi_progress.add(ProgressBar::new(total_items as u64));
    main_progress_bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.green/black}] {pos}/{len} ({percent}%) {eta}")
        .unwrap()
        .with_key("elapsed_precise", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:02}:{:02}:{:03}",
                state.elapsed().as_secs() / 60,
                state.elapsed().as_secs() % 60,
                state.elapsed().subsec_millis()
            ).unwrap();
        })
        .progress_chars("━━╾─"));

    // Enable steady tick for the main progress bar
    main_progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

    // Ensure the main progress bar starts at 0
    main_progress_bar.set_position(0);

    // Ensure the progress bars are displayed immediately
    multi_progress.println("").unwrap();

    // Force the progress bars to render
    for pb in &item_progress_bars {
        pb.tick();
    }
    main_progress_bar.tick();

    let start_time = Instant::now();
    let parsing_failures = Arc::new(Mutex::new(0u32));
    let last_result = Arc::new(Mutex::new(String::new()));

    let rubric = load_rubric(&task_config.rubric_template).await?;

    // Process all items in the JSON array concurrently, limited to concurrent_batch_size at a time
    let results: Vec<Result<(), AppError>> = stream::iter(items.iter_mut().enumerate())
        .map(|(index, item)| {
            let rubric_clone = rubric.clone();
            let item_progress = item_progress_bars[index % concurrent_batch_size].clone();
            item_progress.set_message(style(format!("Item {}/{} - Processing", index + 1, total_items)).dim().bold().to_string());
            let main_progress_bar = main_progress_bar.clone();
            let parsing_failures = Arc::clone(&parsing_failures);
            let last_result = Arc::clone(&last_result);

            async move {
                let input = item.input.clone();
                let output = item.output.clone();
                let context = context! {
                    input => input,
                    output => output,
                };
                let populated_template = populate_template(&rubric_clone, &context)?;

                // Execute llamafile with the populated template
                let llamafile_output = execute_llamafile_with_retries(&populated_template, MAX_RETRIES, &config.cache_dir, args).await?;

                // Log the llamafile output for debugging
                debug!("Llamafile output for item {}: {}", index + 1, llamafile_output);

                item.feedback = Some(llamafile_output.trim().to_string());

                match SCORE_REGEX.captures(&llamafile_output) {
                    Some(captures) => {
                        if let Some(score_match) = captures.get(1) {
                            if let Ok(score_num) = score_match.as_str().trim().parse::<i32>() {
                                debug!("Extracted score: {}", score_num);
                                item.score = Some(score_num);
                            } else {
                                error!("Failed to parse score as integer");
                                item.score = None;
                            }
                        } else {
                            error!("Score regex matched but couldn't extract content");
                            item.score = None;
                        }
                    }
                    None => {
                        error!("No score found in llamafile output");
                        item.score = None;
                    }
                }

                item_progress.finish_with_message(
                    format!("{} {}",
                        style("✅").green(),
                        style(format!("Item {}/{} - Completed", index + 1, total_items)).dim().bold()
                    )
                );
                main_progress_bar.inc(1);
                *last_result.lock().await = llamafile_output;

                Ok(())
            }
        })
        .buffer_unordered(concurrent_batch_size)
        .collect()
        .await;

    // Handle errors
    for result in results {
        if let Err(e) = result {
            println!("{}", style(format!("Error processing item: {:?}", e)).red());
            *parsing_failures.lock().await += 1;
        }
    }

    // Clear all individual progress bars
    // for pb in item_progress_bars {
    //     pb.finish_and_clear();
    // }
    main_progress_bar.finish_with_message("All items processed");

    let elapsed = start_time.elapsed();
    let parsing_failures = *parsing_failures.lock().await;
    let last_result = last_result.lock().await.clone();

    // Write the updated JSON data back to the file
    match file_format.as_str() {
        "json" => write_json(&items, &task_config.data)?,
        "csv" => write_csv(&items, &task_config.data)?,
        _ => return Err(AppError::ConfigError(format!("Unsupported file format for saving: {}", file_format))),
    }

    println!("\n\n{}", style("Task Summary:").yellow().bold());
    println!("┌─────────────────┬────────────────────────────────┐");
    println!("│ Metric          │ Value                          │");
    println!("├─────────────────┼────────────────────────────────┤");
    println!("│ Time taken      │ {:<30} │", format!("{:.2} seconds", elapsed.as_secs_f64()));
    println!("│ Processed       │ {:<30} │", format!("{} items", total_items - parsing_failures as usize));
    println!("│ Results saved in│ {:<30} │", task_config.data);
    println!("└─────────────────┴────────────────────────────────┘");

    if parsing_failures > 0 {
        println!("{}", style(format!("Failed items: {}", parsing_failures)).yellow());
    }

    Ok((parsing_failures, last_result))
}

async fn load_rubric(rubric_template: &str) -> Result<String, AppError> {
    if Path::new(rubric_template).exists() {
        // If it's a file path, read the file
        let content = tokio::fs::read_to_string(rubric_template).await
            .map_err(|e| AppError::FileReadError(format!("Failed to read rubric file '{}': {}", rubric_template, e)))?;
        Ok(normalize_line_endings(&content))
    } else {
        // If it's not a file path, assume it's the content itself
        Ok(normalize_line_endings(rubric_template))
    }
}

pub async fn update_json_file(
    file_path: &str,
    index: usize,
    field_name: &str,
    value: Value,
) -> Result<(), AppError> {
    let mut locks = FILE_LOCKS.lock().await;
    let file_lock = locks
        .entry(file_path.to_string())
        .or_insert_with(|| Mutex::new(()));
    let _guard = file_lock.lock().await;

    let file_content = fs::read(file_path).await?;
    let file_content = String::from_utf8(file_content)
        .map_err(|e| AppError::CustomError(format!("Failed to decode file content as UTF-8: {}", e)))?;

    let mut json: Value = serde_json::from_str(&file_content)?;

    if let Some(array) = json.as_array_mut() {
        if let Some(item) = array.get_mut(index) {
            if let Some(obj) = item.as_object_mut() {
                obj.insert(field_name.to_string(), value);
            } else {
                return Err(AppError::JsonParseError("Failed to parse JSON data".to_string()));
            }
        } else {
            return Err(AppError::JsonParseError("Failed to parse JSON data".to_string()));
        }
    } else {
        return Err(AppError::JsonParseError("Failed to parse JSON data".to_string()));
    }

    let updated_content = serde_json::to_string_pretty(&json)?;
    fs::write(file_path, updated_content.as_bytes()).await?;

    Ok(())
}

pub async fn execute_llamafile_with_retries(
    input: &str,
    max_retries: u32,
    cache_dir: &str,
    args: &Args,
) -> Result<String, AppError> {
    fs::create_dir_all(cache_dir).await?;

    let llamafile_path = PathBuf::from(cache_dir).join("flow-judge.llamafile");

    // Print file information for debugging
    let metadata = fs::metadata(&llamafile_path).await?;
    debug!("Llamafile size: {} bytes", metadata.len());
    debug!("Llamafile permissions: {:o}", metadata.permissions().mode());
    debug!("Llamafile full path: {:?}", llamafile_path);

    let thread_count = args.thread_count.unwrap_or_else(|| thread::available_parallelism().map(|p| p.get()).unwrap_or(1));

    let mut llamafile_command = format!(
        "{} -c {} -ngl {} {} --temp {} -n {} -t {} -p \"{}\"",
        llamafile_path.display(),
        args.context_size,
        args.gpu_layers,
        if args.enable_kv_offload { "" } else { "-nkvo" },
        args.temperature,
        args.max_tokens,
        thread_count,
        input
    );

    // Add additional llamafile arguments
    if let Some(extra_args) = &args.llamafile_kvargs {
        validate_llamafile_kvargs(&llamafile_path, extra_args).await?;
        for arg_pair in extra_args.split(',') {
            if let Some((key, value)) = arg_pair.split_once('=') {
                llamafile_command.push_str(&format!(" --{} {}", key, value));
            }
        }
    }

    for attempt in 1..=max_retries {
        debug!("Executing llamafile, attempt {}/{}", attempt, max_retries);

        let output = if cfg!(target_os = "windows") {
            tokio::process::Command::new("cmd")
                .args(&["/C", &llamafile_command])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?
        } else {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&llamafile_command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?
        };

        if output.status.success() {
            debug!("Llamafile execution successful");
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let error = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Attempt {}/{}: Command executed, but returned an error: {}",
            attempt, max_retries, error
        );

        if attempt < max_retries {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    error!("Max retries reached for llamafile execution");
    Err(AppError::CommandExecutionError(
        "Max retries reached".to_string(),
    ))
}

pub async fn fetch_rubrics(task_config: &TaskConfig) -> Result<String, AppError> {
    let rubric_path = PathBuf::from(RUBRICS_DIR).join(&task_config.rubric_template);
    let rubric_content = fs::read_to_string(&rubric_path).await?;
    Ok(rubric_content)
}

pub fn extract_input_names_from_rubric(rubric: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
    let mut input_names = Vec::new();

    for capture in re.captures_iter(rubric) {
        if let Some(name) = capture.get(1) {
            input_names.push(name.as_str().to_string());
        }
    }

    // Remove duplicates
    input_names.sort();
    input_names.dedup();

    input_names
}


pub fn populate_template(rubric: &str, context: &minijinja::value::Value) -> Result<String, AppError> {
    let mut env = Environment::new();
    env.add_template("rubric", rubric)?;

    let template = env.get_template("rubric")?;
    template.render(context).map_err(AppError::from)
}

fn save_last_result(result: &str, cache_dir: &str) -> Result<(), AppError> {
    let result_file_path = PathBuf::from(cache_dir).join("last_result.txt");
    let mut file = File::create(&result_file_path)
        .map_err(|e| AppError::FileWriteError(format!("Failed to create last result file: {}", e)))?;

    file.write_all(result.as_bytes())
        .map_err(|e| AppError::FileWriteError(format!("Failed to write last result to file: {}", e)))?;

    Ok(())
}

fn read_last_result(cache_dir: &str) -> Result<String, AppError> {
    let result_file_path = PathBuf::from(cache_dir).join("last_result.txt");
    std::fs::read_to_string(&result_file_path)
        .map_err(|e| AppError::FileReadError(format!("Failed to read last result file: {}", e)))
}

// Helper function to read file as UTF-8
async fn read_file_as_utf8(path: &PathBuf) -> Result<String, AppError> {
    let mut file = tokio::fs::File::open(path).await
        .map_err(|e| AppError::FileReadError(format!("Failed to open file '{}': {}", path.display(), e)))?;

    let mut content = Vec::new();
    file.read_to_end(&mut content).await
        .map_err(|e| AppError::FileReadError(format!("Failed to read file '{}': {}", path.display(), e)))?;

    String::from_utf8(content)
        .map_err(|e| AppError::EncodingError(format!("File '{}' is not valid UTF-8: {}", path.display(), e)))
}

// Helper function to normalize line endings
fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn detect_file_type(file_path: &str) -> Result<String, AppError> {
    let path = Path::new(file_path);
    match path.extension().and_then(|s| s.to_str()) {
        Some("json") => Ok("json".to_string()),
        Some("csv") => Ok("csv".to_string()),
        Some(ext) => Err(AppError::ConfigError(format!("Unsupported file type: {}", ext))),
        None => Err(AppError::ConfigError("File has no extension".to_string())),
    }
}

fn write_csv(items: &[IoItem], file_path: &str) -> Result<(), AppError> {
    let file = File::create(file_path)
        .map_err(|e| AppError::FileWriteError(format!("Failed to create file '{}': {}", file_path, e)))?;

    let mut writer = WriterBuilder::new()
        .from_writer(file);

    for item in items {
        writer.serialize(item)
            .map_err(|e| AppError::CsvWriteError(format!("Failed to write CSV record: {}", e)))?;
    }

    writer.flush()
        .map_err(|e| AppError::FileWriteError(format!("Failed to flush CSV writer: {}", e)))?;
    Ok(())
}

fn read_csv(file_path: &str) -> Result<Vec<IoItem>, AppError> {
    let file = File::open(file_path)
        .map_err(|e| AppError::FileReadError(format!("Failed to open file '{}': {}", file_path, e)))?;

    let mut reader = ReaderBuilder::new()
        .from_reader(file);

    let items: Result<Vec<IoItem>, _> = reader.deserialize().collect();
    items.map_err(|e| AppError::CsvReadError(format!("Failed to read CSV: {}", e)))
}

fn write_json(items: &[IoItem], file_path: &str) -> Result<(), AppError> {
    let file = File::create(file_path)
        .map_err(|e| AppError::FileWriteError(format!("Failed to create file '{}': {}", file_path, e)))?;
    let buf_writer = BufWriter::new(file);

    serde_json::to_writer_pretty(buf_writer, items)
        .map_err(|e| AppError::JsonWriteError(format!("Failed to write JSON: {}", e)))?;

    Ok(())
}

fn read_json(file_path: &str) -> Result<Vec<IoItem>, AppError> {
    let file = File::open(file_path)
        .map_err(|e| AppError::FileReadError(format!("Failed to open file '{}': {}", file_path, e)))?;
    let buf_reader = BufReader::new(file);

    let items: Vec<IoItem> = serde_json::from_reader(buf_reader)
        .map_err(|e| AppError::JsonParseError(format!("Failed to parse JSON: {}", e)))?;

    // Ensure UTF-8 validity for all string fields
    items.into_iter().map(|item| {
        Ok(IoItem {
            input: ensure_utf8(&item.input)?,
            output: ensure_utf8(&item.output)?,
            feedback: item.feedback.map(|f| ensure_utf8(&f)).transpose()?,
            score: item.score,
        })
    }).collect()
}

fn ensure_utf8(s: &str) -> Result<String, AppError> {
    from_utf8(s.as_bytes())
        .map(|s| s.to_string())
        .map_err(|e| AppError::EncodingError(format!("Invalid UTF-8 sequence: {}", e)))
}

async fn validate_llamafile_kvargs(llamafile_path: &Path, args: &str) -> Result<(), AppError> {
    let output = tokio::process::Command::new(llamafile_path)
        .arg("--help")
        .output()
        .await
        .map_err(|e| AppError::CommandExecutionError(format!("Failed to execute llamafile: {}", e)))?;

    let help_text = String::from_utf8_lossy(&output.stdout);

    for arg_pair in args.split(',') {
        if let Some((key, _)) = arg_pair.split_once('=') {
            if !help_text.contains(&format!("--{}", key)) {
                return Err(AppError::ConfigError(format!("Invalid llamafile argument: --{}", key)));
            }
        }
    }

    Ok(())
}
