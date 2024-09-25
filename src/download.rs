use crate::AppError;
use crate::Config;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::os::unix::fs::PermissionsExt;
use std::collections::HashMap;
use tokio::fs::OpenOptions;
use serde_json::{json, Value};
use tokio::io::{AsyncSeekExt, AsyncReadExt};
use log::{error, debug, info};
use tokio::fs;
use sha2::{Sha256, Digest};

use crate::models::LLAMAFILE_LOCK_URL;

pub async fn download_flow_judge_llamafile(config: &Config) -> Result<(), AppError> {
    println!("\n{}", style("Checking Flow-Judge-v0.1 llamafile"));

    let file_path = ".cache/flow-judge.llamafile";
    let lock_file_path = ".cache/flow-judge.llamafile.lock";

    // Create the .cache directory if it doesn't exist
    tokio::fs::create_dir_all(".cache").await?;

    // Fetch and save the lock file
    fetch_and_save_lock_file(&config.llamafile_url, lock_file_path).await?;

    // Extract hash from the URL
    let expected_hash = "4845b598e88dbae320d2773edc15b52e054a53dd3a64b069121c33c3806c2dec";

    // Check if file exists and verify
    if let Ok(_metadata) = tokio::fs::metadata(file_path).await {
        println!("Existing llamafile found. Verifying...");

        if verify_file(file_path, lock_file_path, expected_hash).await? {
            println!("{}", style("Verification passed. Using existing llamafile."));
            return Ok(());
        } else {
            println!("Verification failed. Re-downloading llamafile.");
        }
    }

    println!(
        "\n{}",
        style("Downloading Flow-Judge-v0.1 quantized to Q4_K_M and converted to llamafile format")
            .green()
            .bold()
    );
    println!("\n{}", style("File details:").magenta());
    println!("  Name: {}", style("flow-judge.llamafile").green());
    println!("  Size: {}", style("2.4 GB").green());
    println!("  URL: {}", style(&config.llamafile_url).yellow());
    println!("  Date added to hub: {}", style("25.09.2024").green());
    println!("  SHA256: {}", style("4845b598e88dbae320d2773edc15b52e054a53dd3a64b069121c33c3806c2dec").green());
    println!("  Llamafile version: {}\n", style("v0.8.13").green());

    // Clone the URL before moving it into the async block
    let llamafile_url = config.llamafile_url.clone();
    let client = Client::new();
    let total_size: u64 = 2_404_988_741;
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("━━╾─"));

    // Ensure the progress bar starts at 0
    pb.set_position(0);

    // Start the download
    let start = Instant::now();
    let mut response = client.get(&llamafile_url).send().await?;
    let mut file = tokio::fs::File::create(file_path).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);

        // Update ETA
        if downloaded > 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let rate = downloaded as f64 / elapsed;
            let remaining = (total_size - downloaded) as f64 / rate;
            pb.set_message(format!("ETA: {:.0}s", remaining));
        }
    }

    pb.finish_with_message("Download completed");

    // Set executable permissions
    let mut perms = tokio::fs::metadata(file_path).await?.permissions();
    perms.set_mode(0o755);
    tokio::fs::set_permissions(file_path, perms).await?;

    // After successful download
    set_download_complete_flag(file_path).await?;

    println!(
        "\n\n{}",
        style("Successfully downloaded, moved, and made executable the llamafile.")
            .green()
            .bold()
    );
    println!("Placed into: {}\n", style(file_path).yellow());
    Ok(())
}

async fn fetch_and_save_lock_file(_url: &str, lock_file_path: &str) -> Result<(), AppError> {
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

async fn verify_file(file_path: &str, lock_file_path: &str, expected_hash: &str) -> Result<bool, AppError> {
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

    let lock_file_hash = lock_content.lines()
        .find(|line| line.starts_with("oid sha256:"))
        .and_then(|line| line.split(':').nth(1))
        .ok_or_else(|| AppError::ParseError("Failed to parse hash from lock file".to_string()))?;

    info!("Lock file hash: {}", lock_file_hash);
    info!("Expected hash from URL: {}", expected_hash);

    if lock_file_hash != expected_hash {
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
