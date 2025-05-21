use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::io;

use crate::error::DownloadError;

/// Structure to hold cache metadata for a manga chapter
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedChapter {
    /// Title of the chapter
    pub title: String,
    /// URL of the chapter
    pub url: String,
    /// Timestamp when the chapter was cached
    pub timestamp: u64,
    /// Checksum of the chapter content
    pub checksum: String,
    /// List of image files
    pub images: Vec<CachedImage>,
}

/// Structure to hold cache metadata for an image
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedImage {
    /// URL of the image
    pub url: String,
    /// Local path relative to the cache directory
    pub path: String,
    /// Checksum of the image file
    pub checksum: String,
    /// Size of the image in bytes
    pub size: u64,
}

/// Main cache manager
#[derive(Debug)]
pub struct CacheManager {
    /// Base directory for the cache
    cache_dir: PathBuf,
    /// Cache index mapping URLs to cached content
    index: HashMap<String, CachedChapter>,
    /// Maximum age for cached content (in seconds)
    max_age: u64,
}

impl CacheManager {
    /// Create a new cache manager with the given cache directory
    pub fn new(cache_dir: impl AsRef<Path>, max_age_days: u64) -> Result<Self, DownloadError> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Ensure the cache directory exists
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| DownloadError::IoError(e))?;
        }

        let index_path = cache_dir.join("index.json");
        let index = if index_path.exists() {
            // Load existing index
            let file = File::open(&index_path)
                .map_err(|e| DownloadError::IoError(e))?;
            serde_json::from_reader(file)
                .map_err(|e| DownloadError::ParsingError(format!("Failed to parse cache index: {}", e)))?
        } else {
            // Create a new empty index
            HashMap::new()
        };

        Ok(Self {
            cache_dir,
            index,
            // Convert days to seconds (86400 seconds in a day)
            max_age: max_age_days * 86400,
        })
    }

    /// Save the cache index to disk
    pub fn save_index(&self) -> Result<(), DownloadError> {
        let index_path = self.cache_dir.join("index.json");
        let file = File::create(&index_path)
            .map_err(|e| DownloadError::IoError(e))?;

        serde_json::to_writer_pretty(file, &self.index)
            .map_err(|e| DownloadError::ParsingError(format!("Failed to write cache index: {}", e)))?;

        Ok(())
    }

    /// Check if a chapter is cached and up-to-date
    pub fn is_chapter_cached(&self, url: &str) -> bool {
        if let Some(cached) = self.index.get(url) {
            // Get current timestamp
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Check if the cache is fresh enough
            if now - cached.timestamp <= self.max_age {
                // Check if all cached images exist
                for image in &cached.images {
                    let image_path = self.cache_dir.join(&image.path);
                    if !image_path.exists() {
                        return false;
                    }
                }
                return true;
            }
        }
        false
    }

    /// Get the paths to cached images for a chapter
    pub fn get_cached_image_paths(&self, url: &str) -> Option<Vec<PathBuf>> {
        if let Some(cached) = self.index.get(url) {
            let image_paths = cached.images.iter()
                .map(|img| self.cache_dir.join(&img.path))
                .collect::<Vec<_>>();

            // Only return the paths if all images exist
            if image_paths.iter().all(|p| p.exists()) {
                return Some(image_paths);
            }
        }
        None
    }

    /// Cache a downloaded image
    pub fn cache_image(&mut self, chapter_url: &str, image_url: &str, image_path: &Path) -> Result<PathBuf, DownloadError> {
        // Create a chapter entry if it doesn't exist
        if !self.index.contains_key(chapter_url) {
            self.index.insert(
                chapter_url.to_string(),
                CachedChapter {
                    title: extract_chapter_title(chapter_url),
                    url: chapter_url.to_string(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    checksum: String::new(), // Will be updated later
                    images: Vec::new(),
                }
            );
        }

        // Generate a cache path for the image
        let cache_filename = format!("{}.jpg", compute_hash(image_url));
        let cache_subdir = compute_hash(chapter_url).chars().take(2).collect::<String>();
        let cache_relpath = Path::new(&cache_subdir).join(&cache_filename);
        let cache_fullpath = self.cache_dir.join(&cache_relpath);

        // Ensure the cache subdirectory exists
        if let Some(parent) = cache_fullpath.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| DownloadError::IoError(e))?;
            }
        }

        // Copy the image to the cache
        fs::copy(image_path, &cache_fullpath)
            .map_err(|e| DownloadError::IoError(e))?;

        // Calculate checksum and file size
        let checksum = calculate_file_checksum(&cache_fullpath)?;
        let size = fs::metadata(&cache_fullpath)
            .map_err(|e| DownloadError::IoError(e))?
            .len();

        // Update the cache index
        if let Some(chapter) = self.index.get_mut(chapter_url) {
            // Remove any existing entry for this image URL
            chapter.images.retain(|img| img.url != image_url);

            // Add the new cached image
            chapter.images.push(CachedImage {
                url: image_url.to_string(),
                path: cache_relpath.to_string_lossy().to_string(),
                checksum,
                size,
            });

            // Update the chapter timestamp
            chapter.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        // Save the updated index
        self.save_index()?;

        Ok(cache_fullpath)
    }

    /// Cache a complete chapter (metadata only)
    pub fn cache_chapter(&mut self, chapter_url: &str, chapter_title: &str, image_urls: &[String]) -> Result<(), DownloadError> {
        // Create or update the chapter entry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let chapter = self.index.entry(chapter_url.to_string())
            .or_insert(CachedChapter {
                title: chapter_title.to_string(),
                url: chapter_url.to_string(),
                timestamp: now,
                checksum: compute_hash(&format!("{:?}", image_urls)),
                images: Vec::new(),
            });

        // Update the timestamp and checksum
        chapter.timestamp = now;
        chapter.title = chapter_title.to_string();
        chapter.checksum = compute_hash(&format!("{:?}", image_urls));

        // Save the updated index
        self.save_index()?;

        Ok(())
    }

    /// Validate cached content by checking checksums
    pub fn validate_cache(&self) -> Result<(usize, usize), DownloadError> {
        let mut valid_items = 0;
        let mut invalid_items = 0;

        for (_, chapter) in &self.index {
            for image in &chapter.images {
                let image_path = self.cache_dir.join(&image.path);

                if image_path.exists() {
                    // Check the file checksum
                    match calculate_file_checksum(&image_path) {
                        Ok(checksum) if checksum == image.checksum => {
                            valid_items += 1;
                        },
                        _ => {
                            invalid_items += 1;
                        }
                    }
                } else {
                    invalid_items += 1;
                }
            }
        }

        Ok((valid_items, invalid_items))
    }

    /// Clear all cached content
    pub fn clear_cache(&mut self) -> Result<(), DownloadError> {
        // Remove all files in the cache directory (except the index file)
        let entries = fs::read_dir(&self.cache_dir)
            .map_err(|e| DownloadError::IoError(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| DownloadError::IoError(e))?;
            let path = entry.path();

            if path.file_name().map_or(false, |name| name != "index.json") {
                if path.is_dir() {
                    fs::remove_dir_all(&path)
                        .map_err(|e| DownloadError::IoError(e))?;
                } else {
                    fs::remove_file(&path)
                        .map_err(|e| DownloadError::IoError(e))?;
                }
            }
        }

        // Clear the index
        self.index.clear();
        self.save_index()?;

        Ok(())
    }

    /// Remove expired items from the cache
    pub fn clean_expired(&mut self) -> Result<usize, DownloadError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut removed_count = 0;
        let mut urls_to_remove = Vec::new();

        // Identify expired entries
        for (url, chapter) in &self.index {
            if now - chapter.timestamp > self.max_age {
                urls_to_remove.push(url.clone());

                // Remove the image files
                for image in &chapter.images {
                    let image_path = self.cache_dir.join(&image.path);
                    if image_path.exists() {
                        if let Err(e) = fs::remove_file(&image_path) {
                            eprintln!("Warning: Failed to remove cached file {}: {}", image_path.display(), e);
                        } else {
                            removed_count += 1;
                        }
                    }

                    // Try to remove parent directory if empty
                    if let Some(parent) = image_path.parent() {
                        // Only try if it's not the main cache directory
                        if parent != self.cache_dir {
                            let _ = fs::remove_dir(parent); // Ignore error if not empty
                        }
                    }
                }
            }
        }

        // Remove expired entries from the index
        for url in urls_to_remove {
            self.index.remove(&url);
        }

        // Save the updated index
        self.save_index()?;

        Ok(removed_count)
    }
}

/// Extract a chapter title from its URL
fn extract_chapter_title(url: &str) -> String {
    // Try to extract chapter name from URL
    // Example: https://www.mangaread.org/manga/name/chapter-1/ -> "Chapter 1"
    url.split('/')
        .filter(|s| !s.is_empty())
        .find(|s| s.starts_with("chapter-"))
        .map(|s| s.replace("chapter-", "Chapter "))
        .unwrap_or_else(|| "Unknown Chapter".to_string())
}

/// Compute a hash of the given string
fn compute_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(input);
    hasher.result_str()
}

/// Calculate SHA-256 checksum of a file
fn calculate_file_checksum(file_path: &Path) -> Result<String, DownloadError> {
    let mut file = File::open(file_path)
        .map_err(|e| DownloadError::IoError(e))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0; 4096];

    loop {
        let bytes_read = file.read(&mut buffer)
            .map_err(|e| DownloadError::IoError(e))?;

        if bytes_read == 0 {
            break;
        }

        hasher.input(&buffer[..bytes_read]);
    }

    Ok(hasher.result_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::io::Write;

    // Helper to create a temporary test directory
    fn setup_test_cache_dir() -> PathBuf {
        let cache_dir = std::env::temp_dir().join("manga_downloader_test_cache");
        fs::create_dir_all(&cache_dir).unwrap();
        cache_dir
    }

    // Helper to clean up test directory
    fn cleanup_test_cache_dir(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    // Helper to create a test image file
    fn create_test_image(path: &Path, content: &[u8]) -> Result<(), io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        file.write_all(content)?;
        Ok(())
    }

    #[test]
    fn test_create_cache_entry() {
        let cache_dir = setup_test_cache_dir();
        let temp_dir = cache_dir.join("temp");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a test image file
        let test_img_path = temp_dir.join("test.jpg");
        let test_data = b"test image data".to_vec();
        create_test_image(&test_img_path, &test_data).unwrap();

        // Initialize cache manager
        let mut cache = CacheManager::new(cache_dir.clone(), 1).unwrap();

        // Cache the image
        let chapter_url = "https://example.com/manga/test-chapter";
        let image_url = "https://example.com/image.jpg";
        let result = cache.cache_image(chapter_url, image_url, &test_img_path);

        assert!(result.is_ok());

        // Verify the cache entry exists in the index
        assert!(cache.is_chapter_cached(chapter_url));

        // Clean up
        cleanup_test_cache_dir(&cache_dir);
    }

    #[test]
    fn test_retrieve_cached_data() {
        let cache_dir = setup_test_cache_dir();
        let temp_dir = cache_dir.join("temp");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a test image file
        let test_img_path = temp_dir.join("test.jpg");
        let test_data = b"test image data".to_vec();
        create_test_image(&test_img_path, &test_data).unwrap();

        // Initialize cache manager
        let mut cache = CacheManager::new(cache_dir.clone(), 1).unwrap();

        // Cache the image
        let chapter_url = "https://example.com/manga/test-chapter";
        let image_url = "https://example.com/image.jpg";
        cache.cache_image(chapter_url, image_url, &test_img_path).unwrap();

        // Retrieve the cached image paths
        let cached_paths = cache.get_cached_image_paths(chapter_url);
        assert!(cached_paths.is_some());
        assert_eq!(cached_paths.unwrap().len(), 1);

        // Clean up
        cleanup_test_cache_dir(&cache_dir);
    }

    #[test]
    fn test_is_chapter_cached() {
        let cache_dir = setup_test_cache_dir();
        let temp_dir = cache_dir.join("temp");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a test image file
        let test_img_path = temp_dir.join("test.jpg");
        let test_data = b"test image data".to_vec();
        create_test_image(&test_img_path, &test_data).unwrap();

        // Initialize cache manager
        let mut cache = CacheManager::new(cache_dir.clone(), 1).unwrap();

        // Cache the image
        let chapter_url = "https://example.com/manga/test-chapter";
        let image_url = "https://example.com/image.jpg";
        cache.cache_image(chapter_url, image_url, &test_img_path).unwrap();

        // Check if chapter is cached
        assert!(cache.is_chapter_cached(chapter_url));
        assert!(!cache.is_chapter_cached("https://example.com/manga/nonexistent"));

        // Clean up
        cleanup_test_cache_dir(&cache_dir);
    }
}