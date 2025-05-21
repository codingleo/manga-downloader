# Manga Downloader

A Rust-based command-line tool for downloading manga from [mangaread.org](https://www.mangaread.org), with features for caching, batch downloading, and PDF generation.

## Features

- **Chapter Selection**: Download individual chapters or ranges of chapters
- **PDF Generation**: Automatically generates PDFs from downloaded manga images
- **Caching System**: Cache downloaded content to avoid redundant downloads
- **Concurrent Downloads**: Configurable concurrency for faster downloads
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Structured Logging**: Detailed logs with configurable verbosity levels

## Installation

### Prerequisites

- Rust and Cargo (1.66.0 or later)
- A working internet connection

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/download-manga.git
cd download-manga

# Build the project
cargo build --release

# The executable will be located at target/release/download-manga
```

## Usage

```bash
# Basic usage
download-manga --link "https://www.mangaread.org/manga/example-manga/" --output-dir "./manga"

# Download all chapters without prompting
download-manga --link "https://www.mangaread.org/manga/example-manga/" --output-dir "./manga" --all

# Enable caching with custom cache directory
download-manga --link "https://www.mangaread.org/manga/example-manga/" --output-dir "./manga" --cache --cache-dir "./custom-cache"

# Increase download concurrency
download-manga --link "https://www.mangaread.org/manga/example-manga/" --output-dir "./manga" --concurrency 10

# Enable verbose logging
download-manga --link "https://www.mangaread.org/manga/example-manga/" --output-dir "./manga" --verbose
```

### Interactive Chapter Selection

When run without the `--all` flag, the program will display a list of available chapters and prompt you to select which ones to download:

```
Available chapters:
------------------
[0] Chapter 1: Beginning
[1] Chapter 2: Adventure
...

Enter chapter numbers to download (comma-separated, ranges allowed e.g. '1,3-5,7'):
> 1,3-5,7
```

## Command Line Options

| Option | Description |
|--------|-------------|
| `--link`, `-l` | The link to the manga to download (required) |
| `--output-dir`, `-o` | The output directory for downloaded content (required) |
| `--concurrency`, `-c` | Maximum number of concurrent downloads (default: 5) |
| `--all`, `-a` | Download all chapters without prompting |
| `--cache` | Enable caching of downloaded content |
| `--cache-max-age` | Maximum age of cached content in days (default: 30) |
| `--cache-dir` | Cache directory (default: ~/.manga-cache) |
| `--validate-cache` | Validate cache integrity |
| `--clear-cache` | Clear the cache |
| `--verbose`, `-v` | Verbose mode (-v for info, -vv for debug, -vvv for trace) |
| `--help`, `-h` | Display help information |
| `--version` | Display version information |

## Project Structure

```
download-manga/
├── src/
│   ├── main.rs                  # Application entry point
│   ├── cache.rs                 # Cache management functionality
│   ├── chapter_to_download.rs   # Chapter representation and handling
│   ├── downloader.rs            # Image downloading logic
│   ├── error.rs                 # Error types and handling
│   ├── manga_to_download.rs     # Manga parsing and metadata
│   ├── pdf.rs                   # PDF generation from images
│   └── assets/
│       └── fonts/               # Embedded fonts for PDF generation
├── Cargo.toml                   # Project dependencies
└── README.md                    # This documentation
```

## Logging System

The application uses a structured logging system with different verbosity levels:

- No flag: Only warnings and errors
- `-v`: Info level (general operation information)
- `-vv`: Debug level (detailed operational data)
- `-vvv`: Trace level (very detailed execution flow)

Log messages include timestamps and module paths to help with troubleshooting.

## Caching System

The caching system stores downloaded manga chapters and images to avoid redundant downloads:

- Default cache location: `~/.manga-cache`
- Cached content includes chapter metadata and images
- Cache validation ensures integrity
- Configurable cache expiration (default: 30 days)

## PDF Generation

The PDF generation process:

- Creates one page per manga image
- Automatically scales images to fit page width while maintaining aspect ratio
- Uses embedded or system fonts with cross-platform compatibility
- Generates standalone PDF files for each chapter

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. 