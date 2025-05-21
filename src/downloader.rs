use std::{fs, path::{Path, PathBuf}, sync::Arc, time::Duration};
use futures::{stream, StreamExt};
use tokio::sync::Semaphore;
use std::env;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress, ProgressState};
use std::fmt::Write;
use tokio::io::AsyncWriteExt;

use crate::error::DownloadError;

/// Downloads a single image from a URL to a specified path
pub async fn download_image(url: &str, path: &Path, progress_bar: Option<&ProgressBar>) -> Result<(), DownloadError> {
    // Create a client with a longer timeout for slow connections
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send().await?;

    // Check if the response was successful
    if !response.status().is_success() {
        if let Some(pb) = progress_bar {
            pb.abandon_with_message(format!("Failed: HTTP error {}", response.status()));
        }
        return Err(DownloadError::ParsingError(
            format!("HTTP error: {} for URL {}", response.status(), url)
        ));
    }

    // Get the total size for progress tracking
    let total_size = response.content_length().unwrap_or(0);
    if let Some(pb) = progress_bar {
        pb.set_length(total_size);
    }

    // Create the file
    let mut file = tokio::fs::File::create(path).await
        .map_err(|e| DownloadError::IoError(e))?;

    // Stream the download with progress updates
    let stream = response.bytes();

    match stream.await {
        Ok(bytes) => {
            let downloaded = bytes.len() as u64;
            if let Some(pb) = progress_bar {
                pb.set_position(downloaded);
            }

            file.write_all(&bytes).await
                .map_err(|e| DownloadError::IoError(e))?;

            if let Some(pb) = progress_bar {
                pb.finish_with_message("Complete");
            }
        },
        Err(e) => {
            if let Some(pb) = progress_bar {
                pb.abandon_with_message(format!("Failed: {}", e));
            }
            return Err(e.into());
        }
    }

    Ok(())
}

/// Downloads multiple images concurrently with a semaphore to limit concurrency
pub async fn download_images(
    image_urls: Vec<String>,
    output_dir: &Path,
    concurrency: usize
) -> Vec<std::path::PathBuf> {
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // Setup progress bars
    let multi_progress = MultiProgress::new();
    let main_progress_style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} images ({eta})"
    )
    .unwrap()
    .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
    .progress_chars("#>-");

    let main_pb = multi_progress.add(ProgressBar::new(image_urls.len() as u64));
    main_pb.set_style(main_progress_style);
    main_pb.set_message("Downloading images...");

    let image_progress_style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} ({eta})"
    )
    .unwrap()
    .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
    .progress_chars("#>-");

    // Store the number of images to download
    let total_images = image_urls.len();

    let download_tasks = stream::iter(
        image_urls.into_iter().enumerate().map(|(i, image_url)| {
            let semaphore = Arc::clone(&semaphore);
            let output_dir = output_dir.to_path_buf();
            let img_progress_style = image_progress_style.clone();
            let multi_progress = multi_progress.clone();
            let main_pb = main_pb.clone();

            async move {
                // Acquire permit from semaphore (blocks if we hit max concurrency)
                let _permit = semaphore.acquire().await.unwrap();

                let image_path = output_dir.join(format!("image_{:03}.jpg", i));

                // Create a progress bar for this image
                let pb = multi_progress.add(ProgressBar::new(0));
                pb.set_style(img_progress_style);
                pb.set_message(format!("Image {}/{}", i + 1, total_images));

                match download_image(&image_url, &image_path, Some(&pb)).await {
                    Ok(_) => {
                        pb.finish_with_message(format!("✓ Image {}", i + 1));
                        main_pb.inc(1);
                        Ok(image_path)
                    },
                    Err(e) => {
                        pb.abandon_with_message(format!("✗ Failed: {}", e));
                        Err(e)
                    }
                }
            }
        })
    )
    .buffer_unordered(concurrency)
    .collect::<Vec<Result<_, _>>>()
    .await;

    main_pb.finish_with_message("All downloads complete!");

    // Filter out errors and keep successful downloads
    download_tasks.into_iter()
        .filter_map(Result::ok)
        .collect()
}

/// Builds a path for a chapter directory with OS-aware path handling
pub fn build_chapter_path(output_dir: &Path, chapter_title: &str) -> std::path::PathBuf {
    // Sanitize chapter title to be safe for all file systems
    let chapter_slug = sanitize_filename(chapter_title);
    output_dir.join(chapter_slug)
}

/// Ensures a directory exists, creating it if necessary
pub fn ensure_dir_exists(path: &Path) -> Result<(), DownloadError> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| DownloadError::IoError(e))?;
    }
    Ok(())
}

/// Sanitizes a string to be safe as a filename across different operating systems
pub fn sanitize_filename(input: &str) -> String {
    // Remove characters that are problematic on various file systems
    let invalid_chars = match env::consts::OS {
        "windows" => r#"\/:*?"<>|"#,
        _ => "/",  // Unix-like systems mainly forbid slashes
    };

    let mut result = input.to_lowercase().replace(" ", "-");

    for c in invalid_chars.chars() {
        result = result.replace(c, "_");
    }

    // Handle Windows reserved filenames
    if env::consts::OS == "windows" {
        let reserved_names = [
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ];

        if reserved_names.iter().any(|&name| result.eq_ignore_ascii_case(name)) {
            result = format!("_{}", result);
        }
    }

    // Ensure filename doesn't start with a dot (hidden file on Unix)
    if result.starts_with('.') {
        result = format!("_{}", result);
    }

    // Trim to reasonable length (some filesystems have limits)
    if result.len() > 255 {
        result.truncate(255);
    }

    result
}

/// Get a system-specific temporary directory for file operations
pub fn get_temp_dir() -> PathBuf {
    let mut temp_dir = env::temp_dir();
    temp_dir.push("manga-downloader");

    // Ensure the directory exists
    let _ = fs::create_dir_all(&temp_dir);

    temp_dir
}