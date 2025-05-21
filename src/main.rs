use std::path::Path;
use std::io::{self, Write};

use clap::Parser;
use log::{error, warn, info, debug, trace};

mod manga_to_download;
mod chapter_to_download;
mod error;
mod pdf;
mod downloader;
mod cache;

use manga_to_download::{MangaToDownload, ChapterInfo};
use error::DownloadError;
use pdf::create_pdf_from_images;
use downloader::{download_images, ensure_dir_exists, build_chapter_path};
use cache::CacheManager;

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

    /// Enable caching of downloaded content
    #[arg(long)]
    pub cache: bool,

    /// Maximum age of cached content in days (default: 30)
    #[arg(long, default_value = "30")]
    pub cache_max_age: u64,

    /// Cache directory (default: ~/.manga-cache)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Validate cache integrity
    #[arg(long)]
    pub validate_cache: bool,

    /// Clear the cache
    #[arg(long)]
    pub clear_cache: bool,

    /// Verbose mode (-v for info, -vv for debug, -vvv for trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[tokio::main]
async fn main() -> Result<(), DownloadError> {
    let args = Args::parse();

    // Initialize logger with appropriate verbosity level
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", match args.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        });

    env_logger::Builder::from_env(env)
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .format_module_path(true)
        .init();

    info!("Starting manga downloader");
    debug!("Command line arguments: {:?}", args);

    // Set up cache if enabled
    let cache_dir = args.cache_dir.clone()
        .map(|dir| Path::new(&dir).to_path_buf())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .expect("Failed to determine home directory")
                .join(".manga-cache")
        });

    debug!("Using cache directory: {:?}", cache_dir);

    // Initialize cache manager if caching is enabled
    let mut cache_manager = if args.cache || args.validate_cache || args.clear_cache {
        info!("Initializing cache manager");
        Some(CacheManager::new(&cache_dir, args.cache_max_age)?)
    } else {
        None
    };

    // Handle cache management commands
    if args.clear_cache {
        if let Some(ref mut cache) = cache_manager {
            info!("Clearing cache...");
            cache.clear_cache()?;
            info!("Cache cleared successfully.");
        }

        if !args.cache && !args.validate_cache {
            return Ok(());
        }
    }

    if args.validate_cache {
        if let Some(ref cache) = cache_manager {
            info!("Validating cache...");
            let (valid, invalid) = cache.validate_cache()?;
            info!("Cache validation complete: {} valid items, {} invalid items", valid, invalid);
            if invalid > 0 {
                warn!("Cache contains {} invalid items", invalid);
            }
        }

        if !args.cache {
            return Ok(());
        }
    }

    let mut manga = MangaToDownload::new(args.link.clone(), args.concurrency).await?;
    let title = manga.get_title();

    info!("Manga: {}", title);

    // Get the list of available chapters
    let chapters = manga.list_available_chapters()?;
    debug!("Found {} chapters", chapters.len());

    // Select which chapters to download
    let selected_indices = if args.all {
        // If --all flag is set, download all chapters
        info!("Downloading all {} chapters", chapters.len());
        (0..chapters.len()).collect::<Vec<_>>()
    } else {
        // Otherwise, let the user select chapters
        select_chapters(&chapters)?
    };

    info!("Selected {} chapters for download", selected_indices.len());

    // Download selected chapters
    manga.download_chapters(&selected_indices).await?;

    // Create output directory
    let output_dir = Path::new(&args.output_dir);
    ensure_dir_exists(output_dir)?;
    debug!("Created output directory: {:?}", output_dir);

    // Process downloaded chapters
    for chapter in manga.chapters {
        info!("Processing chapter: {}", chapter.title);
        debug!("Chapter URL: {}", chapter.url);

        // Check cache first if caching is enabled
        let mut use_cached_images = false;
        let mut cached_image_paths = Vec::new();

        if let Some(ref cache) = cache_manager {
            if cache.is_chapter_cached(&chapter.url) {
                info!("Using cached version of chapter: {}", chapter.title);
                if let Some(paths) = cache.get_cached_image_paths(&chapter.url) {
                    cached_image_paths = paths;
                    use_cached_images = true;
                    debug!("Retrieved {} cached images", cached_image_paths.len());
                } else {
                    warn!("Cache index indicates chapter is cached but images not found");
                }
            } else {
                debug!("Chapter not in cache or cache expired");
            }
        }

        let image_paths = if use_cached_images {
            cached_image_paths
        } else {
            // Create chapter directory
            let chapter_dir = build_chapter_path(output_dir, &chapter.title);
            ensure_dir_exists(&chapter_dir)?;
            debug!("Created chapter directory: {:?}", chapter_dir);

            // Download images
            info!("Downloading {} images for chapter: {}", chapter.images.len(), chapter.title);
            let downloaded_paths = download_images(chapter.images.clone(), &chapter_dir, args.concurrency).await;
            debug!("Downloaded {} images", downloaded_paths.len());

            // Cache the downloaded images if caching is enabled
            if let Some(ref mut cache) = cache_manager {
                debug!("Caching chapter metadata and images");
                // Cache chapter metadata
                cache.cache_chapter(&chapter.url, &chapter.title, &chapter.images)?;

                // Cache each downloaded image
                for (i, path) in downloaded_paths.iter().enumerate() {
                    if i < chapter.images.len() {
                        let image_url = &chapter.images[i];
                        match cache.cache_image(&chapter.url, image_url, path) {
                            Ok(_) => trace!("Cached image: {}", image_url),
                            Err(e) => warn!("Failed to cache image {}: {}", image_url, e),
                        }
                    }
                }

                info!("Chapter cached successfully");
            }

            downloaded_paths
        };

        if image_paths.is_empty() {
            error!("Failed to download any images for chapter: {}", chapter.title);
            continue;
        }

        // Create PDF
        let chapter_slug = chapter.title.replace(" ", "-").to_lowercase();
        info!("Creating PDF for chapter: {}", chapter.title);
        let pdf_path = output_dir.join(format!("{}.pdf", chapter_slug));
        debug!("PDF path: {:?}", pdf_path);

        match create_pdf_from_images(&image_paths, &pdf_path) {
            Ok(_) => info!("✓ PDF created successfully"),
            Err(e) => error!("✗ Failed to create PDF: {}", e),
        }
    }

    info!("All chapters have been processed");

    Ok(())
}

// Function to let user select which chapters to download
fn select_chapters(chapters: &[ChapterInfo]) -> Result<Vec<usize>, DownloadError> {
    info!("Displaying available chapters");
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
            debug!("User pressed Enter to continue viewing chapters");
        }
    }

    println!("\nEnter chapter numbers to download (comma-separated, ranges allowed e.g. '1,3-5,7'):");
    print!("> ");
    io::stdout().flush().map_err(|e| DownloadError::IoError(e))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| DownloadError::IoError(e))?;
    debug!("User input for chapter selection: '{}'", input.trim());

    // Parse the comma-separated selection including ranges
    let selected_indices = parse_chapter_selection(&input, chapters.len())?;

    if selected_indices.is_empty() {
        warn!("No valid chapters were selected");
        return Err(DownloadError::ParsingError(String::from("No valid chapters selected")));
    }

    debug!("Selected chapter indices: {:?}", selected_indices);
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
                    trace!("Adding range {}-{} to selection", start, end);
                    selected.extend(start..=end);
                } else {
                    warn!("Range {}-{} is invalid or out of bounds, ignoring", start, end);
                    println!("Warning: Range {}-{} is invalid or out of bounds, ignoring", start, end);
                }
            } else {
                warn!("Invalid range format '{}', ignoring", part);
                println!("Warning: Invalid range format '{}', ignoring", part);
            }
        } else {
            // Handle single numbers
            match part.parse::<usize>() {
                Ok(index) if index < max_chapters => {
                    trace!("Adding chapter {} to selection", index);
                    selected.push(index);
                },
                Ok(index) => {
                    warn!("Chapter index {} is out of bounds, ignoring", index);
                    println!("Warning: Chapter index {} is out of bounds, ignoring", index);
                },
                Err(_) => {
                    warn!("Invalid chapter number '{}', ignoring", part);
                    println!("Warning: Invalid chapter number '{}', ignoring", part);
                }
            }
        }
    }

    // Remove duplicates and sort
    selected.sort();
    selected.dedup();
    debug!("Final selection after deduplication: {:?}", selected);

    Ok(selected)
}
