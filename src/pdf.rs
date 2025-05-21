use std::path::Path;
use std::env;
use std::fs;
use crate::error::DownloadError;
use image;
use log::{debug, info, warn, trace};

/// Generates a PDF from a collection of image paths
pub fn create_pdf_from_images(image_paths: &[impl AsRef<Path>], output_path: &Path) -> Result<(), DownloadError> {
    if image_paths.is_empty() {
        return Err(DownloadError::PdfGenerationError(String::from("Cannot create PDF: no images provided")));
    }

    debug!("Creating PDF from {} images", image_paths.len());
    trace!("Output path: {:?}", output_path);

    // Try to find a suitable font
    let font_family = find_system_font()
        .map_err(|e| DownloadError::PdfGenerationError(format!("Failed to load font: {}", e)))?;

    let mut doc = genpdf::Document::new(font_family);

    // Configure document properties
    doc.set_title("Manga Chapter");
    doc.set_paper_size(genpdf::PaperSize::A4);

    // Get page width (in mm)
    // A4 paper size is 210mm x 297mm
    let page_width = 210.0;

    // Add each image to the document
    for (i, path) in image_paths.iter().enumerate() {
        trace!("Processing image {}/{}", i+1, image_paths.len());

        // Load the image to get its dimensions
        let img_data = load_image_from_path(path)?;
        let img_width = img_data.width() as f64;

        // Calculate scale to fit image to page width (considering margins)
        // Assuming 10mm margins on each side (20mm total horizontal margins)
        let available_width = page_width - 12.0; // Available width in mm

        // Convert image width to mm (assuming 300 DPI)
        let img_width_mm = img_width * 25.4 / 300.0;

        // Calculate scale factor to fit width
        let scale_factor = available_width / img_width_mm;
        trace!("Image dimensions: {}x{}, scale factor: {:.2}",
              img_data.width(), img_data.height(), scale_factor);

        // Create and add the image with proper scaling
        let img = genpdf::elements::Image::from_path(path)
            .map_err(|e| DownloadError::ImageProcessingError(format!("Failed to load image: {}", e)))?
            .with_alignment(genpdf::Alignment::Center)
            .with_scale(genpdf::Scale::new(scale_factor, scale_factor));

        doc.push(img);

        // Add a page break after each image except the last one
        if i < image_paths.len() - 1 {
            doc.push(genpdf::elements::PageBreak::new());
        }
    }

    // Render the PDF to file
    debug!("Rendering PDF to file: {:?}", output_path);
    doc.render_to_file(output_path)?;
    info!("PDF created successfully with {} pages", image_paths.len());

    Ok(())
}

/// Finds a suitable system font with cross-platform support
fn find_system_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    debug!("Looking for suitable font");

    // First try the embedded Roboto font which should be reliable
    if let Ok(font_family) = create_embedded_roboto_font() {
        info!("Using embedded Roboto font");
        return Ok(font_family);
    }

    // Then try with direct font paths that are known to work well
    if let Ok(font_family) = load_direct_system_font() {
        info!("Using direct system font");
        return Ok(font_family);
    }

    // Next, try to load from system font locations
    if let Ok(font_family) = find_system_font_from_paths() {
        info!("Using system font");
        return Ok(font_family);
    }

    // Then try to load from the bundled font file
    if let Ok(font_family) = load_bundled_font_from_file() {
        info!("Using bundled font file");
        return Ok(font_family);
    }

    warn!("Could not load any suitable font");
    Err("Could not load any suitable font".to_string())
}

/// Create a font family using the embedded Roboto font
fn create_embedded_roboto_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    trace!("Attempting to use embedded Roboto font");

    // Check if Geneva.ttf exists on macOS and use it directly (known to work)
    if env::consts::OS == "macos" {
        let geneva_path = Path::new("/System/Library/Fonts/Geneva.ttf");
        if geneva_path.exists() {
            debug!("Found Geneva.ttf on macOS: {:?}", geneva_path);
            if let Ok(bytes) = fs::read(geneva_path) {
                match genpdf::fonts::FontData::new(bytes, None) {
                    Ok(font_data) => {
                        info!("Using system Geneva font as embedded fallback");
                        let regular = font_data.clone();
                        let bold = font_data.clone();
                        let italic = font_data.clone();
                        let bold_italic = font_data;

                        return Ok(genpdf::fonts::FontFamily {
                            regular,
                            bold,
                            italic,
                            bold_italic,
                        });
                    },
                    Err(e) => {
                        debug!("Failed to load Geneva font: {}", e);
                    } // Continue to try other fonts
                }
            }
        }
    }

    // These bytes are directly embedded in the binary
    let font_bytes = include_bytes!("assets/fonts/Roboto-Regular.ttf");

    if font_bytes.len() < 1000 {
        warn!("Embedded Roboto font bytes appear to be incomplete");
        return Err("Embedded Roboto font bytes appear to be incomplete".to_string());
    }

    match genpdf::fonts::FontData::new(font_bytes.to_vec(), None) {
        Ok(font_data) => {
            debug!("Successfully loaded embedded Roboto font");
            // Use the same font for all styles
            let regular = font_data.clone();
            let bold = font_data.clone();
            let italic = font_data.clone();
            let bold_italic = font_data;

            Ok(genpdf::fonts::FontFamily {
                regular,
                bold,
                italic,
                bold_italic,
            })
        },
        Err(e) => {
            warn!("Failed to load embedded Roboto font: {}", e);
            Err(format!("Failed to load embedded Roboto font: {}", e))
        },
    }
}

/// Try to load a font that is known to work well with genpdf
fn load_direct_system_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    let os = env::consts::OS;
    debug!("Attempting to load direct system font for OS: {}", os);

    // Prioritize TTF files which work better with rusttype than TTC files
    let font_paths = match os {
        "macos" => vec![
            // TTF files first (these work better with rusttype)
            "/System/Library/Fonts/Geneva.ttf",
            "/System/Library/Fonts/Monaco.ttf",
            // TTC files as fallbacks
            "/System/Library/Fonts/Times.ttc",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/LucidaGrande.ttc",
        ],
        "windows" => vec![
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\verdana.ttf",
            "C:\\Windows\\Fonts\\tahoma.ttf",
            "C:\\Windows\\Fonts\\times.ttf",
            "C:\\Windows\\Fonts\\calibri.ttf",
        ],
        "linux" => vec![
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ],
        _ => vec![],
    };

    for path in font_paths {
        // Skip TTC files if they're likely to cause problems
        if path.ends_with(".ttc") && (path != "/System/Library/Fonts/Geneva.ttf") {
            trace!("Skipping TTC file: {}", path);
            continue; // Skip TTC files as they often fail with rusttype
        }

        trace!("Trying font: {}", path);
        if let Ok(bytes) = fs::read(path) {
            match genpdf::fonts::FontData::new(bytes.clone(), None) {
                Ok(font_data) => {
                    info!("Successfully loaded font: {}", path);
                    // Create a font family with all styles using the same font
                    return Ok(genpdf::fonts::FontFamily {
                        regular: font_data.clone(),
                        bold: font_data.clone(),
                        italic: font_data.clone(),
                        bold_italic: font_data,
                    });
                },
                Err(e) => {
                    trace!("Failed to load font {}: {}", path, e);
                    // Don't log every failure as it's normal to try multiple fonts
                    // before finding one that works
                }
            }
        } else {
            trace!("Font file not found or not readable: {}", path);
        }
    }

    debug!("No direct system font found");
    Err("No direct system font found".to_string())
}

/// Try to locate a system font from various paths
fn find_system_font_from_paths() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    debug!("Searching for system fonts in various paths");
    // Common font locations by platform
    let font_paths: Vec<(String, &str)> = get_platform_font_paths();

    // Try each font path
    for (path, font) in font_paths {
        let font_path = Path::new(&path).join(font);
        trace!("Checking font path: {:?}", font_path);

        if font_path.exists() {
            debug!("Found font at: {}", font_path.display());
            // Try directly first
            if let Ok(bytes) = fs::read(&font_path) {
                if let Ok(font_data) = genpdf::fonts::FontData::new(bytes, None) {
                    debug!("Successfully loaded font data directly");
                    let regular = font_data.clone();
                    let bold = font_data.clone();
                    let italic = font_data.clone();
                    let bold_italic = font_data;

                    return Ok(genpdf::fonts::FontFamily {
                        regular,
                        bold,
                        italic,
                        bold_italic,
                    });
                } else {
                    trace!("Failed to create font data from bytes for: {}", font_path.display());
                }
            } else {
                trace!("Failed to read font file: {}", font_path.display());
            }

            // Try the normal way
            match genpdf::fonts::from_files(&path, font, None) {
                Ok(font_family) => {
                    debug!("Successfully loaded font family via genpdf API");
                    return Ok(font_family);
                },
                Err(e) => {
                    trace!("Failed to load font via genpdf API: {}", e);
                }
            }
        }
    }

    debug!("No system font found in searched paths");
    Err("No system font found".to_string())
}

/// Try to load the bundled font from the assets directory
fn load_bundled_font_from_file() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    let bundled_font_path = Path::new("assets/fonts/LiberationSans-Regular.ttf");
    if bundled_font_path.exists() {
        println!("Found bundled font at: {}", bundled_font_path.display());
        // Try to read the file directly
        match fs::read(bundled_font_path) {
            Ok(bytes) => {
                if bytes.len() < 100 {
                    return Err("Bundled font file is too small or corrupt".to_string());
                }

                match genpdf::fonts::FontData::new(bytes, None) {
                    Ok(font_data) => {
                        let regular = font_data.clone();
                        let bold = font_data.clone();
                        let italic = font_data.clone();
                        let bold_italic = font_data;

                        return Ok(genpdf::fonts::FontFamily {
                            regular,
                            bold,
                            italic,
                            bold_italic,
                        });
                    },
                    Err(e) => return Err(format!("Could not create font data: {}", e)),
                }
            },
            Err(e) => return Err(format!("Could not read bundled font file: {}", e)),
        }
    }

    Err("Bundled font file does not exist".to_string())
}

/// Returns a list of platform-specific font paths to try
fn get_platform_font_paths() -> Vec<(String, &'static str)> {
    let mut paths = Vec::new();

    // Detect operating system
    let os = env::consts::OS;
    println!("Detected OS: {}", os);

    match os {
        "macos" => {
            // Prioritize TTF files first
            paths.push(("/System/Library/Fonts".to_string(), "Geneva.ttf"));
            paths.push(("/System/Library/Fonts".to_string(), "Monaco.ttf"));
            // Then try TTC files
            paths.push(("/System/Library/Fonts".to_string(), "Helvetica.ttc"));
            paths.push(("/Library/Fonts".to_string(), "Arial.ttf"));
            paths.push(("/System/Library/Fonts".to_string(), "LucidaGrande.ttc"));
            paths.push(("/System/Library/Fonts".to_string(), "Times.ttc"));
            paths.push(("/System/Library/Fonts".to_string(), "Menlo.ttc"));
            paths.push(("/System/Library/Fonts".to_string(), "AppleSDGothicNeo.ttc"));
        },
        "windows" => {
            paths.push(("C:\\Windows\\Fonts".to_string(), "arial.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "times.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "cour.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "tahoma.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "verdana.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "calibri.ttf"));
            paths.push(("C:\\Windows\\Fonts".to_string(), "segoeui.ttf"));
        },
        "linux" => {
            // Common Linux font paths
            paths.push(("/usr/share/fonts/truetype/dejavu".to_string(), "DejaVuSans.ttf"));
            paths.push(("/usr/share/fonts/TTF".to_string(), "Arial.ttf"));
            paths.push(("/usr/share/fonts/truetype/liberation".to_string(), "LiberationSans-Regular.ttf"));
            paths.push(("/usr/share/fonts/truetype/ubuntu".to_string(), "Ubuntu-R.ttf"));
            paths.push(("/usr/share/fonts/liberation".to_string(), "LiberationSans-Regular.ttf"));
            paths.push(("/usr/share/fonts/TTF".to_string(), "DejaVuSans.ttf"));
            paths.push(("/usr/share/fonts/opentype".to_string(), "SourceSansPro-Regular.otf"));
            paths.push(("/usr/share/fonts/noto".to_string(), "NotoSans-Regular.ttf"));
            paths.push(("/usr/share/fonts/truetype/noto".to_string(), "NotoSans-Regular.ttf"));
        },
        _ => {
            // Add some reasonable defaults for other platforms
            paths.push(("/usr/local/share/fonts".to_string(), "Arial.ttf"));
        }
    }

    // Also check user's home directory for fonts
    if let Ok(home) = home_dir() {
        match os {
            "macos" => {
                let user_font = home.join("Library/Fonts");
                paths.push((user_font.to_string_lossy().to_string(), "Arial.ttf"));
                paths.push((user_font.to_string_lossy().to_string(), "Helvetica.ttf"));
            },
            "windows" => {
                let user_font = home.join("AppData\\Local\\Microsoft\\Windows\\Fonts");
                paths.push((user_font.to_string_lossy().to_string(), "arial.ttf"));
                paths.push((user_font.to_string_lossy().to_string(), "calibri.ttf"));
            },
            "linux" => {
                let user_font = home.join(".local/share/fonts");
                paths.push((user_font.to_string_lossy().to_string(), "DejaVuSans.ttf"));
                paths.push((user_font.to_string_lossy().to_string(), "LiberationSans-Regular.ttf"));
            },
            _ => {}
        }
    }

    // Always add the local project font directory
    paths.push(("assets/fonts".to_string(), "LiberationSans-Regular.ttf"));
    paths.push(("assets/fonts".to_string(), "Roboto-Regular.ttf"));

    paths
}

/// Cross-platform function to get the home directory
fn home_dir() -> Result<std::path::PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())
}

// Helper function to load an image from a path
fn load_image_from_path(path: impl AsRef<Path>) -> Result<image::DynamicImage, DownloadError> {
    image::open(path.as_ref())
        .map_err(|e| DownloadError::PdfGenerationError(format!("Failed to load image: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    // Helper to create a temporary test image
    fn create_test_image(path: &Path, width: u32, height: u32) -> Result<(), DownloadError> {
        // Create an image
        let mut img = image::RgbImage::new(width, height);

        // Fill with a simple pattern
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([
                ((x as u32) % 256) as u8,
                ((y as u32) % 256) as u8,
                ((x + y) as u32 % 256) as u8,
            ]);
        }

        // Save it
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| DownloadError::IoError(e))?;
        }

        img.save(path).map_err(|e|
            DownloadError::ImageProcessingError(format!("Failed to save test image: {}", e))
        )?;

        Ok(())
    }

    #[test]
    fn test_create_pdf_from_images() {
        // Create a temporary directory for the test
        let temp_dir = std::env::temp_dir().join("manga_pdf_test");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create test images with different dimensions
        let test_images = vec![
            temp_dir.join("page1.jpg"),
            temp_dir.join("page2.jpg"),
            temp_dir.join("page3.jpg"),
        ];

        // Create images with different dimensions to test scaling
        create_test_image(&test_images[0], 800, 1200).unwrap();
        create_test_image(&test_images[1], 600, 900).unwrap();
        create_test_image(&test_images[2], 1000, 1500).unwrap();

        // Output PDF path
        let output_path = temp_dir.join("test_output.pdf");

        // Generate the PDF
        let result = create_pdf_from_images(&test_images, &output_path);

        // Verify the result
        assert!(result.is_ok());
        assert!(output_path.exists());

        // Get file size to verify it's a valid PDF
        let metadata = fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 100); // Make sure it's not empty

        // Clean up
        let _ = fs::remove_dir_all(temp_dir);
    }
}