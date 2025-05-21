use crate::chapter_to_download::ChapterToDownload;
use crate::error::DownloadError;
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress, ProgressState};
use std::fmt::Write;
use std::time::Duration;

#[derive(Debug)]
pub struct ChapterInfo {
    pub index: usize,
    pub title: String,
    pub url: String,
}

pub struct MangaToDownload {
  pub link: String,
  pub title: String,
  pub chapters: Vec<ChapterToDownload>,
  pub document: scraper::Html,
  pub concurrency: usize,
}

impl MangaToDownload {
  pub async fn new(link: String, concurrency: usize) -> Result<Self, DownloadError> {
      // Create a spinner for initialization
      let spinner = ProgressBar::new_spinner();
      spinner.set_style(
          ProgressStyle::with_template("{spinner:.green} {msg}")
          .unwrap()
          .tick_strings(&[
              "⠋ ", "⠙ ", "⠹ ", "⠸ ", "⠼ ", "⠴ ", "⠦ ", "⠧ ", "⠇ ", "⠏ "
          ])
      );
      spinner.set_message("Fetching manga information...");
      spinner.enable_steady_tick(Duration::from_millis(100));

      let response = reqwest::get(&link).await?;
      let body = response.text().await?;
      let document = scraper::Html::parse_document(&body.trim());
      let mut manga = Self {
          link,
          title: String::new(),
          chapters: Vec::new(),
          document,
          concurrency,
      };

      spinner.set_message("Processing manga title...");
      manga.process_title()?;

      spinner.finish_with_message(format!("✓ Found manga: {}", manga.title));
      Ok(manga)
  }

  fn process_title(&mut self) -> Result<(), DownloadError> {
      let title_selector = scraper::Selector::parse(".post-title h1")
          .map_err(|_| DownloadError::SelectorError(String::from("Failed to parse .post-title h1 selector")))?;

      let title = self.document.select(&title_selector).next()
          .ok_or_else(|| DownloadError::ElementNotFound(String::from("Manga title element not found")))?;

      self.title = title.text().collect::<Vec<_>>().join(" ");
      Ok(())
  }

  // New method to list available chapters without downloading them
  pub fn list_available_chapters(&self) -> Result<Vec<ChapterInfo>, DownloadError> {
      let spinner = ProgressBar::new_spinner();
      spinner.set_style(
          ProgressStyle::with_template("{spinner:.green} {msg}")
          .unwrap()
          .tick_strings(&[
              "⠋ ", "⠙ ", "⠹ ", "⠸ ", "⠼ ", "⠴ ", "⠦ ", "⠧ ", "⠇ ", "⠏ "
          ])
      );
      spinner.set_message("Scanning for available chapters...");
      spinner.enable_steady_tick(Duration::from_millis(100));

      let list_of_chapters_selector = scraper::Selector::parse(".wp-manga-chapter a")
          .map_err(|_| DownloadError::SelectorError(String::from("Failed to parse .wp-manga-chapter a selector")))?;

      let chapters = self.document.select(&list_of_chapters_selector)
          .filter_map(|e| {
              let url = e.attr("href")?.to_string();
              let title = e.text().collect::<Vec<_>>().join(" ").trim().to_string();
              Some(ChapterInfo {
                  index: 0, // Will be updated after collection
                  title,
                  url,
              })
          })
          .collect::<Vec<_>>();

      if chapters.is_empty() {
          spinner.finish_with_message("✗ No chapters found for this manga");
          return Err(DownloadError::ElementNotFound(String::from("No chapters found for this manga")));
      }

      // Number the chapters in reverse order (newest first) and return them
      let mut numbered_chapters = chapters
          .into_iter()
          .rev() // Reverse to get newest first
          .enumerate()
          .map(|(i, mut chapter)| {
              chapter.index = i;
              chapter
          })
          .collect::<Vec<_>>();

      // Sort by index so they're in a logical order (usually newest first)
      numbered_chapters.sort_by_key(|c| c.index);

      spinner.finish_with_message(format!("✓ Found {} chapters", numbered_chapters.len()));
      Ok(numbered_chapters)
  }

  // Download selected chapters by their indices
  pub async fn download_chapters(&mut self, selected_indices: &[usize]) -> Result<(), DownloadError> {
      // Setup progress tracking
      let multi_progress = MultiProgress::new();

      // Main progress style for chapters
      let main_progress_style = ProgressStyle::with_template(
          "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} chapters ({eta})"
      )
      .unwrap()
      .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
      .progress_chars("#>-");

      // Spinner style for chapter processing
      let chapter_spinner_style = ProgressStyle::with_template(
          "{spinner:.green} {prefix:.bold.dim} {msg}"
      )
      .unwrap()
      .tick_strings(&[
          "⠋ ", "⠙ ", "⠹ ", "⠸ ", "⠼ ", "⠴ ", "⠦ ", "⠧ ", "⠇ ", "⠏ "
      ]);

      // Fetch available chapters
      let spinner = multi_progress.add(ProgressBar::new_spinner());
      spinner.set_style(chapter_spinner_style.clone());
      spinner.set_message("Fetching chapter list...");
      spinner.enable_steady_tick(Duration::from_millis(100));

      let all_chapters = self.list_available_chapters()?;
      spinner.finish_with_message(format!("Found {} chapters total", all_chapters.len()));

      if selected_indices.is_empty() {
          return Err(DownloadError::ParsingError(String::from("No chapters selected for download")));
      }

      // Get the list of chapter URLs to download
      let chapters_to_download: Vec<&ChapterInfo> = all_chapters.iter()
          .filter(|chapter| selected_indices.contains(&chapter.index))
          .collect();

      if chapters_to_download.is_empty() {
          return Err(DownloadError::ParsingError(String::from("None of the selected indices match available chapters")));
      }

      // Store the number of chapters to download
      let chapters_count = chapters_to_download.len();

      // Set up the main progress bar
      let main_pb = multi_progress.add(ProgressBar::new(chapters_count as u64));
      main_pb.set_style(main_progress_style);
      main_pb.set_message(format!("Downloading {} selected chapters", chapters_count));

      // Use Stream to limit concurrent requests
      let mut successful_chapters = Vec::new();
      let mut failed_chapters = 0;

      let mut chapter_stream = stream::iter(chapters_to_download.into_iter().enumerate())
          .map(|(idx, chapter)| {
              let chapter_pb = multi_progress.add(ProgressBar::new_spinner());
              chapter_pb.set_style(chapter_spinner_style.clone());
              chapter_pb.set_prefix(format!("[Chapter {}/{}]", idx + 1, chapters_count));
              chapter_pb.set_message(format!("Downloading: {}", chapter.title));
              chapter_pb.enable_steady_tick(Duration::from_millis(100));

              async move {
                  let result = ChapterToDownload::new(chapter.url.clone()).await;
                  (chapter, result, chapter_pb)
              }
          })
          .buffer_unordered(self.concurrency);

      while let Some((chapter_info, result, chapter_pb)) = chapter_stream.next().await {
          match result {
              Ok(chapter) => {
                  chapter_pb.finish_with_message(format!("✓ Downloaded: {}", chapter.title));
                  successful_chapters.push(chapter);
                  main_pb.inc(1);
              }
              Err(err) => {
                  chapter_pb.finish_with_message(format!("✗ Failed: {}", chapter_info.title));
                  eprintln!("✗ Failed to process chapter {}: {}", chapter_info.title, err);
                  failed_chapters += 1;
                  main_pb.inc(1);
              }
          }
      }

      self.chapters = successful_chapters;
      main_pb.finish_with_message(format!("Downloaded {} chapters", self.chapters.len()));

      if self.chapters.is_empty() {
          return Err(DownloadError::ElementNotFound(String::from("Failed to process any chapters")));
      }

      if failed_chapters > 0 {
          println!("Warning: {} chapters failed to download", failed_chapters);
      }

      Ok(())
  }

  pub fn get_title(&self) -> String {
      self.title.clone()
  }
}