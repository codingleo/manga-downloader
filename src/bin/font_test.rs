use std::path::Path;
use std::fs;

fn main() {
    println!("Font test program");
    println!("=================");

    // Try the Geneva font which is known to work
    let geneva_path = Path::new("/System/Library/Fonts/Geneva.ttf");
    println!("Geneva path: {}", geneva_path.display());
    println!("Geneva exists: {}", geneva_path.exists());

    if geneva_path.exists() {
        match fs::read(geneva_path) {
            Ok(bytes) => {
                println!("Geneva font read successfully. Size: {} bytes", bytes.len());

                // Test rusttype library directly
                match rusttype::Font::try_from_vec(bytes.clone()) {
                    Some(font) => {
                        println!("RustType successfully parsed Geneva font");
                        let scale = rusttype::Scale::uniform(24.0);
                        let v_metrics = font.v_metrics(scale);
                        println!("Geneva metrics - ascent: {}, descent: {}, line gap: {}",
                            v_metrics.ascent, v_metrics.descent, v_metrics.line_gap);
                    },
                    None => println!("RustType failed to parse Geneva font"),
                }

                // Try to create a new font with genpdf
                match genpdf::fonts::FontData::new(bytes, None) {
                    Ok(_) => println!("GenPDF successfully created a FontData object from Geneva"),
                    Err(e) => println!("GenPDF failed to create a FontData object from Geneva: {}", e),
                }
            },
            Err(e) => println!("Failed to read Geneva font file: {}", e),
        }
    }

    // Check if the LiberationSans font file exists
    let font_path = Path::new("assets/fonts/LiberationSans-Regular.ttf");
    println!("\nLiberation Sans path: {}", font_path.display());
    println!("Liberation Sans exists: {}", font_path.exists());

    if font_path.exists() {
        match fs::read(font_path) {
            Ok(bytes) => {
                println!("Liberation Sans font read successfully. Size: {} bytes", bytes.len());

                // Test rusttype library directly
                match rusttype::Font::try_from_vec(bytes.clone()) {
                    Some(font) => {
                        println!("RustType successfully parsed Liberation Sans font");
                        let scale = rusttype::Scale::uniform(24.0);
                        let v_metrics = font.v_metrics(scale);
                        println!("Liberation Sans metrics - ascent: {}, descent: {}, line gap: {}",
                            v_metrics.ascent, v_metrics.descent, v_metrics.line_gap);
                    },
                    None => println!("RustType failed to parse Liberation Sans font"),
                }

                // Try to create a new font with genpdf
                match genpdf::fonts::FontData::new(bytes, None) {
                    Ok(_) => println!("GenPDF successfully created a FontData object from Liberation Sans"),
                    Err(e) => println!("GenPDF failed to create a FontData object from Liberation Sans: {}", e),
                }
            },
            Err(e) => println!("Failed to read Liberation Sans font file: {}", e),
        }
    }

    // Create a small PDF with text to test
    println!("\nTrying to create a minimal PDF with Geneva font...");
    let result = create_test_pdf();
    match result {
        Ok(_) => println!("Successfully created test PDF"),
        Err(e) => println!("Failed to create test PDF: {}", e),
    }
}

fn create_test_pdf() -> Result<(), String> {
    // Try to use Geneva font which we know works
    let geneva_path = Path::new("/System/Library/Fonts/Geneva.ttf");
    let font_bytes = match fs::read(geneva_path) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to read Geneva font: {}", e)),
    };

    let font_data = match genpdf::fonts::FontData::new(font_bytes, None) {
        Ok(data) => data,
        Err(e) => return Err(format!("Failed to create font data from Geneva: {}", e)),
    };

    // Create a new font family
    let font_family = genpdf::fonts::FontFamily {
        regular: font_data.clone(),
        bold: font_data.clone(),
        italic: font_data.clone(),
        bold_italic: font_data,
    };

    // Create a simple document
    let mut doc = genpdf::Document::new(font_family);
    doc.set_title("Font Test Document");
    doc.set_paper_size(genpdf::PaperSize::A4);

    // Add a simple text element
    let mut decorator = genpdf::SimplePageDecorator::new();
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);

    doc.push(genpdf::elements::Paragraph::new("This is a test document to verify font loading."));
    doc.push(genpdf::elements::Paragraph::new("If you can read this, the font is working correctly."));

    // Write to the output file
    match doc.render_to_file("font_test.pdf") {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to render PDF: {}", e)),
    }
}