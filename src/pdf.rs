use std::path::Path;
use std::env;
use std::fs;
use crate::error::DownloadError;
use image;

/// Generates a PDF from a collection of image paths
pub fn create_pdf_from_images(image_paths: &[impl AsRef<Path>], output_path: &Path) -> Result<(), DownloadError> {
    if image_paths.is_empty() {
        return Err(DownloadError::PdfGenerationError(String::from("Cannot create PDF: no images provided")));
    }

    // Try to find a suitable font
    let font_family = find_system_font()
        .map_err(|e| DownloadError::PdfGenerationError(format!("Failed to load font: {}", e)))?;

    let mut doc = genpdf::Document::new(font_family);

    // Configure document properties
    doc.set_title("Manga Chapter");
    doc.set_paper_size(genpdf::PaperSize::A4);

    // Add each image to the document
    for path in image_paths {
        let img = genpdf::elements::Image::from_path(path)
            .map_err(|e| DownloadError::ImageProcessingError(format!("Failed to load image: {}", e)))?
            .with_alignment(genpdf::Alignment::Center)
            .with_scale(genpdf::Scale::new(1.0, 1.0));
        doc.push(img);
    }

    // Render the PDF to file
    doc.render_to_file(output_path)?;

    Ok(())
}

/// Finds a suitable system font with cross-platform support
fn find_system_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    // First try the embedded Roboto font which should be reliable
    if let Ok(font_family) = create_embedded_roboto_font() {
        println!("Using embedded Roboto font");
        return Ok(font_family);
    }

    // Then try with direct font paths that are known to work well
    if let Ok(font_family) = load_direct_system_font() {
        println!("Using direct system font");
        return Ok(font_family);
    }

    // Next, try to load from system font locations
    if let Ok(font_family) = find_system_font_from_paths() {
        println!("Using system font");
        return Ok(font_family);
    }

    // Then try to load from the bundled font file
    if let Ok(font_family) = load_bundled_font_from_file() {
        println!("Using bundled font file");
        return Ok(font_family);
    }

    Err("Could not load any suitable font".to_string())
}

/// Create a font family using the embedded Roboto font
fn create_embedded_roboto_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    // Check if Geneva.ttf exists on macOS and use it directly (known to work)
    if env::consts::OS == "macos" {
        let geneva_path = Path::new("/System/Library/Fonts/Geneva.ttf");
        if geneva_path.exists() {
            if let Ok(bytes) = fs::read(geneva_path) {
                match genpdf::fonts::FontData::new(bytes, None) {
                    Ok(font_data) => {
                        println!("Using system Geneva font as embedded fallback");
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
                    Err(_) => {} // Continue to try other fonts
                }
            }
        }
    }

    // These bytes are directly embedded in the binary
    let font_bytes = include_bytes!("assets/fonts/Roboto-Regular.ttf");

    if font_bytes.len() < 1000 {
        return Err("Embedded Roboto font bytes appear to be incomplete".to_string());
    }

    match genpdf::fonts::FontData::new(font_bytes.to_vec(), None) {
        Ok(font_data) => {
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
        Err(e) => Err(format!("Failed to load embedded Roboto font: {}", e)),
    }
}

/// Try to load a font that is known to work well with genpdf
fn load_direct_system_font() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    let os = env::consts::OS;

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
            continue; // Skip TTC files as they often fail with rusttype
        }

        if let Ok(bytes) = fs::read(path) {
            match genpdf::fonts::FontData::new(bytes.clone(), None) {
                Ok(font_data) => {
                    println!("Successfully loaded font: {}", path);
                    // Create a font family with all styles using the same font
                    return Ok(genpdf::fonts::FontFamily {
                        regular: font_data.clone(),
                        bold: font_data.clone(),
                        italic: font_data.clone(),
                        bold_italic: font_data,
                    });
                },
                Err(_) => {
                    // Don't log every failure as it's normal to try multiple fonts
                    // before finding one that works
                }
            }
        }
    }

    Err("No direct system font found".to_string())
}

/// Try to locate a system font from various paths
fn find_system_font_from_paths() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    // Common font locations by platform
    let font_paths: Vec<(String, &str)> = get_platform_font_paths();

    // Try each font path
    for (path, font) in font_paths {
        let font_path = Path::new(&path).join(font);
        if font_path.exists() {
            println!("Found font at: {}", font_path.display());
            // Try directly first
            if let Ok(bytes) = fs::read(&font_path) {
                if let Ok(font_data) = genpdf::fonts::FontData::new(bytes, None) {
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
                }
            }

            // Try the normal way
            if let Ok(font_family) = genpdf::fonts::from_files(&path, font, None) {
                return Ok(font_family);
            }
        }
    }

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