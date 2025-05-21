use crate::error::DownloadError;

pub struct ChapterToDownload {
  pub link: String,
  pub title: String,
  pub images: Vec<String>,
  pub document: scraper::Html,
}

impl ChapterToDownload {
  pub async fn new(link: String) -> Result<Self, DownloadError> {
      let response = reqwest::get(&link).await?;
      let body = response.text().await?;
      let document = scraper::Html::parse_document(&body.trim());
      let mut chapter = Self { link, title: String::new(), images: Vec::new(), document };
      chapter.process_title()?;
      println!("processing images of chapter");
      chapter.process_images()?;
      Ok(chapter)
  }

  fn process_title(&mut self) -> Result<(), DownloadError> {
      let title_selector = scraper::Selector::parse("#chapter-heading")
          .map_err(|_| DownloadError::SelectorError(String::from("Failed to parse #chapter-heading selector")))?;

      let title = self.document.select(&title_selector).next()
          .ok_or_else(|| DownloadError::ElementNotFound(String::from("Chapter heading element not found")))?;

      self.title = title.text().collect::<Vec<_>>().join(" ");
      Ok(())
  }

  pub fn process_images(&mut self) -> Result<(), DownloadError> {
      let images_selector = scraper::Selector::parse(".page-break img")
          .map_err(|_| DownloadError::SelectorError(String::from("Failed to parse .page-break img selector")))?;

      let images = self.document.select(&images_selector).map(|e| {
          if let Some(src) = e.attr("src") {
              src.trim().to_string()
          } else {
              match e.attr("data-cfsrc") {
                  Some(attr) => attr.trim().to_string(),
                  None => String::new(),
              }
          }
      }).filter(|url| !url.is_empty()).collect::<Vec<_>>();

      if images.is_empty() {
          return Err(DownloadError::ElementNotFound(String::from("No images found in chapter")));
      }

      self.images = images;
      Ok(())
  }
}