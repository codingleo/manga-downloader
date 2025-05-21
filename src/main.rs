use std::path::Path;
use std::io::{self, Write};

use clap::Parser;

mod manga_to_download;
mod chapter_to_download;
mod error;
mod pdf;
mod downloader;

use manga_to_download::{MangaToDownload, ChapterInfo};
use error::DownloadError;
use pdf::create_pdf_from_images;
use downloader::{download_images, ensure_dir_exists, build_chapter_path};

/// Download a manga from a given link from https://www.mangaread.org
#[derive(Debug, Parser)]
#[command(version, about, long_about = "Download a manga from a given link from https://www.mangaread.org")]
pub struct Args {
    /// The link to the manga to download
    #[arg(short, long)]
    pub link: String,

    /// The output directory
    #[arg(short, long)]
    pub output_dir: String,

    /// Maximum number of concurrent downloads (default: 5)
    #[arg(short, long, default_value = "5")]
    pub concurrency: usize,

    /// Download all chapters without prompting
    #[arg(short, long)]
    pub all: bool,
}

#[tokio::main]
async fn main() -> Result<(), DownloadError> {
    let args = Args::parse();
    let mut manga = MangaToDownload::new(args.link, args.concurrency).await?;
    let title = manga.get_title();

    println!("Manga: {}", title);

    // Get the list of available chapters
    let chapters = manga.list_available_chapters()?;

    // Select which chapters to download
    let selected_indices = if args.all {
        // If --all flag is set, download all chapters
        println!("Downloading all {} chapters", chapters.len());
        (0..chapters.len()).collect::<Vec<_>>()
    } else {
        // Otherwise, let the user select chapters
        select_chapters(&chapters)?
    };

    // Download selected chapters
    manga.download_chapters(&selected_indices).await?;

    // Create output directory
    let output_dir = Path::new(&args.output_dir);
    ensure_dir_exists(output_dir)?;

    // Process downloaded chapters
    for chapter in manga.chapters {
        println!("Processing chapter: {}", chapter.title);

        // Create chapter directory
        let chapter_dir = build_chapter_path(output_dir, &chapter.title);
        ensure_dir_exists(&chapter_dir)?;

        // Download images
        println!("Downloading {} images for chapter: {}", chapter.images.len(), chapter.title);
        let image_paths = download_images(chapter.images, &chapter_dir, args.concurrency).await;

        if image_paths.is_empty() {
            eprintln!("Failed to download any images for chapter: {}", chapter.title);
            continue;
        }

        // Create PDF
        let chapter_slug = chapter.title.replace(" ", "-").to_lowercase();
        println!("Creating PDF for chapter: {}", chapter.title);

        match create_pdf_from_images(&image_paths, &output_dir.join(format!("{}.pdf", chapter_slug))) {
            Ok(_) => println!("✓ PDF created successfully"),
            Err(e) => eprintln!("✗ Failed to create PDF: {}", e),
        }
    }

    println!("All chapters have been processed");

    Ok(())
}

// Function to let user select which chapters to download
fn select_chapters(chapters: &[ChapterInfo]) -> Result<Vec<usize>, DownloadError> {
    println!("\nAvailable chapters:");
    println!("------------------");

    // Display chapters in groups of 20 to avoid flooding the terminal
    let chunk_size = 20;
    for chunk in chapters.chunks(chunk_size) {
        for chapter in chunk {
            println!("[{}] {}", chapter.index, chapter.title);
        }

        // If not the last chunk, wait for the user to continue
        if chunk.len() == chunk_size && chapter_index_of_last(chunk) + 1 < chapters.len() {
            print!("Press Enter to see more chapters...");
            io::stdout().flush().map_err(|e| DownloadError::IoError(e))?;
            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(|e| DownloadError::IoError(e))?;
        }
    }

    println!("\nEnter chapter numbers to download (comma-separated, ranges allowed e.g. '1,3-5,7'):");
    print!("> ");
    io::stdout().flush().map_err(|e| DownloadError::IoError(e))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| DownloadError::IoError(e))?;

    // Parse the comma-separated selection including ranges
    let selected_indices = parse_chapter_selection(&input, chapters.len())?;

    if selected_indices.is_empty() {
        return Err(DownloadError::ParsingError(String::from("No valid chapters selected")));
    }

    println!("Selected {} chapters for download", selected_indices.len());
    Ok(selected_indices)
}

// Helper function to get the index of the last chapter in a chunk
fn chapter_index_of_last(chunk: &[ChapterInfo]) -> usize {
    chunk.last().map(|c| c.index).unwrap_or(0)
}

// Parse user input for chapter selection
fn parse_chapter_selection(input: &str, max_chapters: usize) -> Result<Vec<usize>, DownloadError> {
    let mut selected = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.contains('-') {
            // Handle ranges like "1-5"
            let range_parts: Vec<&str> = part.split('-').collect();
            if range_parts.len() == 2 {
                let start = range_parts[0].trim().parse::<usize>()
                    .map_err(|_| DownloadError::ParsingError(format!("Invalid range start: {}", range_parts[0])))?;
                let end = range_parts[1].trim().parse::<usize>()
                    .map_err(|_| DownloadError::ParsingError(format!("Invalid range end: {}", range_parts[1])))?;

                if start <= end && end < max_chapters {
                    selected.extend(start..=end);
                } else {
                    eprintln!("Warning: Range {}-{} is invalid or out of bounds, ignoring", start, end);
                }
            } else {
                eprintln!("Warning: Invalid range format '{}', ignoring", part);
            }
        } else {
            // Handle single numbers
            match part.parse::<usize>() {
                Ok(index) if index < max_chapters => {
                    selected.push(index);
                },
                Ok(index) => {
                    eprintln!("Warning: Chapter index {} is out of bounds, ignoring", index);
                },
                Err(_) => {
                    eprintln!("Warning: Invalid chapter number '{}', ignoring", part);
                }
            }
        }
    }

    // Remove duplicates and sort
    selected.sort();
    selected.dedup();

    Ok(selected)
}
