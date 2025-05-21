use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum DownloadError {
    RequestFailed(reqwest::Error),
    IoError(std::io::Error),
    ParsingError(String),
    SelectorError(String),
    ElementNotFound(String),
    AttributeNotFound(String),
    ImageProcessingError(String),
    PdfGenerationError(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::RequestFailed(e) => write!(f, "Failed to make HTTP request: {}", e),
            DownloadError::IoError(e) => write!(f, "IO operation failed: {}", e),
            DownloadError::ParsingError(msg) => write!(f, "Failed to parse HTML: {}", msg),
            DownloadError::SelectorError(msg) => write!(f, "Invalid CSS selector: {}", msg),
            DownloadError::ElementNotFound(msg) => write!(f, "Element not found: {}", msg),
            DownloadError::AttributeNotFound(msg) => write!(f, "Attribute not found: {}", msg),
            DownloadError::ImageProcessingError(msg) => write!(f, "Image processing error: {}", msg),
            DownloadError::PdfGenerationError(msg) => write!(f, "PDF generation error: {}", msg),

        }
    }
}

impl Error for DownloadError {}

impl From<reqwest::Error> for DownloadError {
    fn from(err: reqwest::Error) -> Self {
        DownloadError::RequestFailed(err)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::IoError(err)
    }
}

impl From<genpdf::error::Error> for DownloadError {
    fn from(err: genpdf::error::Error) -> Self {
        DownloadError::PdfGenerationError(err.to_string())
    }
}