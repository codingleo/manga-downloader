// Expose modules for integration testing
pub mod cache;
pub mod chapter_to_download;
pub mod downloader;
pub mod error;
pub mod manga_to_download;
pub mod pdf;

// Re-export important types for easier use in tests
pub use error::DownloadError;
pub use manga_to_download::MangaToDownload;
pub use chapter_to_download::ChapterToDownload;
pub use cache::CacheManager;