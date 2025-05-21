use download_manga::chapter_to_download::ChapterToDownload;
use download_manga::manga_to_download::ChapterInfo;
use download_manga::error::DownloadError;

// Helper function to parse chapter selection (copied from main.rs as it might not be exported)
fn parse_chapter_selection(chapters: &[ChapterInfo], input: &str) -> Result<Vec<usize>, DownloadError> {
    let input = input.trim().to_lowercase();

    // Check for "all" keyword
    if input == "all" {
        return Ok((0..chapters.len()).collect());
    }

    let mut selected_indices = Vec::new();

    // Split by comma for multiple selections
    for part in input.split(',') {
        if part.contains('-') {
            // Handle ranges like "1-3"
            let range_parts: Vec<&str> = part.split('-').collect();
            if range_parts.len() != 2 {
                return Err(DownloadError::ParsingError(format!("Invalid range format: {}", part)));
            }

            let start = range_parts[0].parse::<usize>()
                .map_err(|_| DownloadError::ParsingError(format!("Invalid number: {}", range_parts[0])))?;

            let end = range_parts[1].parse::<usize>()
                .map_err(|_| DownloadError::ParsingError(format!("Invalid number: {}", range_parts[1])))?;

            // Adjust for 1-based indexing in user input
            if start < 1 || end > chapters.len() || start > end {
                return Err(DownloadError::ParsingError(format!("Range {}-{} is out of bounds (1-{})", start, end, chapters.len())));
            }

            // Convert from 1-based to 0-based indexing
            for i in (start-1)..=(end-1) {
                selected_indices.push(i);
            }
        } else {
            // Handle single numbers
            let index = part.parse::<usize>()
                .map_err(|_| DownloadError::ParsingError(format!("Invalid number: {}", part)))?;

            // Adjust for 1-based indexing in user input
            if index < 1 || index > chapters.len() {
                return Err(DownloadError::ParsingError(format!("Chapter {} is out of bounds (1-{})", index, chapters.len())));
            }

            // Convert from 1-based to 0-based indexing
            selected_indices.push(index - 1);
        }
    }

    if selected_indices.is_empty() {
        return Err(DownloadError::ParsingError("No valid chapters selected".to_string()));
    }

    Ok(selected_indices)
}

#[test]
fn test_parse_chapter_selection_individual() {
    // Create a list of test chapters
    let chapters = vec![
        ChapterInfo {
            index: 0,
            title: "Chapter 1".to_string(),
            url: "https://example.com/chapter-1".to_string(),
        },
        ChapterInfo {
            index: 1,
            title: "Chapter 2".to_string(),
            url: "https://example.com/chapter-2".to_string(),
        },
        ChapterInfo {
            index: 2,
            title: "Chapter 3".to_string(),
            url: "https://example.com/chapter-3".to_string(),
        },
    ];

    // Test individual selection
    let selected = parse_chapter_selection(&chapters, "1,3").unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0], 0); // Chapter 1 (index 0)
    assert_eq!(selected[1], 2); // Chapter 3 (index 2)
}

#[test]
fn test_parse_chapter_selection_range() {
    // Create a list of test chapters
    let chapters = vec![
        ChapterInfo {
            index: 0,
            title: "Chapter 1".to_string(),
            url: "https://example.com/chapter-1".to_string(),
        },
        ChapterInfo {
            index: 1,
            title: "Chapter 2".to_string(),
            url: "https://example.com/chapter-2".to_string(),
        },
        ChapterInfo {
            index: 2,
            title: "Chapter 3".to_string(),
            url: "https://example.com/chapter-3".to_string(),
        },
        ChapterInfo {
            index: 3,
            title: "Chapter 4".to_string(),
            url: "https://example.com/chapter-4".to_string(),
        },
    ];

    // Test range selection
    let selected = parse_chapter_selection(&chapters, "2-4").unwrap();
    assert_eq!(selected.len(), 3);
    assert_eq!(selected, vec![1, 2, 3]); // Chapters 2-4 (indices 1-3)
}

#[test]
fn test_parse_chapter_selection_mixed() {
    // Create a list of test chapters
    let chapters = vec![
        ChapterInfo {
            index: 0,
            title: "Chapter 1".to_string(),
            url: "https://example.com/chapter-1".to_string(),
        },
        ChapterInfo {
            index: 1,
            title: "Chapter 2".to_string(),
            url: "https://example.com/chapter-2".to_string(),
        },
        ChapterInfo {
            index: 2,
            title: "Chapter 3".to_string(),
            url: "https://example.com/chapter-3".to_string(),
        },
        ChapterInfo {
            index: 3,
            title: "Chapter 4".to_string(),
            url: "https://example.com/chapter-4".to_string(),
        },
        ChapterInfo {
            index: 4,
            title: "Chapter 5".to_string(),
            url: "https://example.com/chapter-5".to_string(),
        },
    ];

    // Test mixed selection
    let selected = parse_chapter_selection(&chapters, "1,3-5").unwrap();
    assert_eq!(selected.len(), 4);
    assert_eq!(selected, vec![0, 2, 3, 4]); // Chapter 1 and 3-5
}

#[test]
fn test_parse_chapter_selection_invalid() {
    // Create a list of test chapters
    let chapters = vec![
        ChapterInfo {
            index: 0,
            title: "Chapter 1".to_string(),
            url: "https://example.com/chapter-1".to_string(),
        },
        ChapterInfo {
            index: 1,
            title: "Chapter 2".to_string(),
            url: "https://example.com/chapter-2".to_string(),
        },
    ];

    // Test invalid selections
    assert!(parse_chapter_selection(&chapters, "0").is_err()); // Index starts at 1
    assert!(parse_chapter_selection(&chapters, "3").is_err()); // Out of bounds
    assert!(parse_chapter_selection(&chapters, "1-3").is_err()); // Range out of bounds
    assert!(parse_chapter_selection(&chapters, "hello").is_err()); // Not a number
}

#[test]
fn test_parse_chapter_selection_all() {
    // Create a list of test chapters
    let chapters = vec![
        ChapterInfo {
            index: 0,
            title: "Chapter 1".to_string(),
            url: "https://example.com/chapter-1".to_string(),
        },
        ChapterInfo {
            index: 1,
            title: "Chapter 2".to_string(),
            url: "https://example.com/chapter-2".to_string(),
        },
        ChapterInfo {
            index: 2,
            title: "Chapter 3".to_string(),
            url: "https://example.com/chapter-3".to_string(),
        },
    ];

    // Test "all" selection
    let selected = parse_chapter_selection(&chapters, "all").unwrap();
    assert_eq!(selected.len(), 3);
    assert_eq!(selected, vec![0, 1, 2]); // All chapters
}