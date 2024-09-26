use crate::AppError;
use crate::Config;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::os::unix::fs::PermissionsExt;
use std::collections::HashMap;
use tokio::fs::OpenOptions;
use serde_json::{json, Value};
use tokio::io::{AsyncSeekExt, AsyncReadExt};
use log::{info};
use tokio::fs;
use std::path::PathBuf;

use crate::models::LLAMAFILE_LOCK_URL;

pub async fn download_flow_judge_llamafile(config: &Config) -> Result<(), AppError> {
    println!("{}",
        style("

             F   L   O   W   A   I

             ⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠀⠀⢀⣀⡀⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠋⠀⢀⣴⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⠈⠛⠛⠛⠛⠛⠛⠛⠛⠋⠀⢀⣴⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⢀⣤⣤⣤⣤⡄⠀⠀⠀⢀⣴⣿⣿⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⡿⠃⠀⢀⣴⣿⣿⣿⣿⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⠸⣿⡿⠋⠀⠀⢰⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
    ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀
    ").white());

    println!("{}", style("            --------------------------------------------------------------------------------").white().dim());

    println!("{}", style("
            Welcome friend.

            This is a quick-start for the Flow-Judge-v0.1 model.

            This tool can evaluate 'input' and 'output' pairs from csv and json files.

            The program will add columns 'score' and 'feedback' to the given file, editing it in-place.

            Before you begin you might want to read the instructions from the model card:

            https://huggingface.co/flowaicom/Flow-Judge-v0.1#prompt-format
    ").white());

    println!("{}", style("            --------------------------------------------------------------------------------").white().dim());

    println!("{}", style("
            We won't show this notice again. Unless you delete your cache.

            Which, by the way, lives here: ~/.cache/fwj/

            Press any key to continue.
    ").white().dim());

    println!("{}", style("            ❤\n\n").red());


    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    info!("\n{}", style("Checking Flow-Judge-v0.1 llamafile"));

    let file_path = PathBuf::from(&config.cache_dir).join("flow-judge.llamafile");
    let lock_file_path = PathBuf::from(&config.cache_dir).join("flow-judge.llamafile.lock");

    // Create the .cache directory if it doesn't exist
    tokio::fs::create_dir_all(&config.cache_dir).await?;

    // Fetch and save the lock file
    fetch_and_save_lock_file(&config.llamafile_url, &lock_file_path).await?;

    // Check if file exists and verify
    if let Ok(_metadata) = tokio::fs::metadata(&file_path).await {
        info!("Existing llamafile found. Verifying...");

        if verify_file(&file_path, &lock_file_path).await? {
            info!("{}", style("Verification passed. Using existing llamafile."));
            return Ok(());
        } else {
            info!("Verification failed. Re-downloading llamafile.");
        }
    }

    println!(
        "{}",
        style("Downloading Flow-Judge-v0.1 quantized to Q4_K_M and converted to llamafile format..")
            .green()
            .bold()
    );
    println!("\n{}", style("File details:").yellow());
    println!("  Name: {}", style("flow-judge.llamafile").green());
    println!("  Size: {}", style("2.4 GB").green());
    println!("  URL: {}", style(&config.llamafile_url).green());
    println!("  Date added to hub: {}", style("25.09.2024").green());
    println!("  SHA256: {}", style("4845b598e88dbae320d2773edc15b52e054a53dd3a64b069121c33c3806c2dec").green());
    println!("  Llamafile version: {}\n", style("v0.8.13").green());

    // Clone the URL before moving it into the async block
    let llamafile_url = config.llamafile_url.clone();
    let client = Client::new();
    let total_size: u64 = 2_404_988_741;
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({percent}%) {eta}")
        .unwrap()
        .with_key("elapsed_precise", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:02}:{:02}:{:03}",
                state.elapsed().as_secs() / 60,
                state.elapsed().as_secs() % 60,
                state.elapsed().subsec_millis()
            ).unwrap();
        })
        .progress_chars("━━╾─"));

    // Enable steady tick for the progress bar
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Ensure the progress bar starts at 0
    pb.set_position(0);

    // Start the download
    let mut response = client.get(&llamafile_url).send().await?;
    let mut file = tokio::fs::File::create(file_path.to_str().unwrap()).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Download completed");

    // Set executable permissions
    let mut perms = tokio::fs::metadata(file_path.to_str().unwrap()).await?.permissions();
    perms.set_mode(0o755);
    tokio::fs::set_permissions(file_path.to_str().unwrap(), perms).await?;

    // After successful download
    set_download_complete_flag(file_path.to_str().unwrap()).await?;

    println!(
        "\n\n{}",
        style("Successfully downloaded, moved, and made executable the llamafile.")
            .green()
            .bold()
    );
    println!("Placed into: {}\n", style(file_path.to_str().unwrap()).yellow());
    Ok(())
}

async fn fetch_and_save_lock_file(_url: &str, lock_file_path: &PathBuf) -> Result<(), AppError> {
    let client = Client::new();
    let response = client.get(LLAMAFILE_LOCK_URL).send().await?.text().await?;

    // Ensure the directory exists
    if let Some(parent) = std::path::Path::new(lock_file_path).parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut file = File::create(lock_file_path).await?;
    file.write_all(response.as_bytes()).await?;

    Ok(())
}

async fn set_download_complete_flag(file_path: &str) -> Result<(), AppError> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(file_path)
        .await?;

    let metadata = json!({
        "download_complete": true,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // Write metadata to the end of the file
    file.seek(std::io::SeekFrom::End(0)).await?;
    file.write_all(serde_json::to_string(&metadata)?.as_bytes()).await?;

    Ok(())
}

async fn verify_file(file_path: &PathBuf, lock_file_path: &PathBuf) -> Result<bool, AppError> {
    info!("Starting verification process");

    let lock_content = tokio::fs::read_to_string(lock_file_path).await
        .map_err(|e| {
            info!("Failed to read lock file: {}. Will download.", e);
            AppError::IoError(e)
        })?;

    if lock_content.trim().is_empty() {
        info!("Lock file is empty. Will download.");
        return Ok(false);
    }

    info!("Lock file content: {}", lock_content);

    let expected_hash = lock_content.lines()
        .find(|line| line.starts_with("oid sha256:"))
        .and_then(|line| line.split(':').nth(1))
        .ok_or_else(|| AppError::ParseError("Failed to parse hash from lock file".to_string()))?;

    info!("Expected hash from lock file: {}", expected_hash);

    if expected_hash != expected_hash {
        info!("Hash mismatch between lock file and URL");
        return Ok(false);
    }

    let mut file = File::open(file_path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    // Check for download_complete flag
    if file_size > 200 {
        file.seek(tokio::io::SeekFrom::End(-200)).await?;
        let mut buffer = vec![0; 200];
        file.read_exact(&mut buffer).await?;

        // Try to find valid JSON at the end of the buffer
        let metadata_str = String::from_utf8_lossy(&buffer);
        if let Some(json_start) = metadata_str.rfind('{') {
            let clean_metadata = &metadata_str[json_start..];
            info!("Cleaned metadata buffer: {}", clean_metadata);

            let download_complete = serde_json::from_str::<HashMap<String, Value>>(clean_metadata)
                .map(|metadata| metadata.get("download_complete") == Some(&json!(true)))
                .unwrap_or(false);

            if download_complete {
                info!("Download complete flag found and hash matches");
                return Ok(true);
            }
        }
    }

    info!("Download complete flag not found or false");
    Ok(false)
}

pub async fn download_file(url: &str, file_path: &str) -> Result<(), AppError> {
    println!("Downloading file from: {}", style(url).yellow());

    // Check if the file already exists
    if tokio::fs::metadata(file_path).await.is_ok() {
        println!("{}", style(format!("File already exists at: {}", file_path)).yellow());
        return Ok(());
    }

    let client = Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(AppError::DownloadError(format!("Failed to download file: HTTP {}", response.status())));
    }

    let content = response.bytes().await?;

    // Ensure the directory exists
    if let Some(parent) = std::path::Path::new(file_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Use OpenOptions to create the file only if it doesn't exist
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file_path)
        .await?;

    file.write_all(&content).await?;

    println!("File downloaded and saved to: {}", style(file_path).green());
    Ok(())
}
