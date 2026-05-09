use std::path::{Path, PathBuf};

use directories::UserDirs;
use ely_browser_core::BrowserCore;
use ely_domain::{DownloadDestination, UrlText};
use url::Url;

pub(super) enum ActiveTabDownloadTarget {
    Prompt { url: UrlText, directory: PathBuf, file_name: String },
    Ready { url: UrlText, target_path: PathBuf },
}

pub(super) fn active_tab_download_target(
    core: &BrowserCore,
) -> Result<ActiveTabDownloadTarget, String> {
    let tab = core.active_tab().map_err(|error| error.to_string())?;
    let parsed = Url::parse(tab.url().as_str()).map_err(|error| error.to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("Only HTTP and HTTPS pages can be downloaded.".to_string()),
    }

    let snapshot = core.snapshot().map_err(|error| error.to_string())?;
    let file_name = suggested_download_file_name(tab.url(), tab.title());
    match snapshot.active_download_policy.destination() {
        DownloadDestination::AskEveryTime => Ok(ActiveTabDownloadTarget::Prompt {
            url: tab.url().clone(),
            directory: default_download_directory()?,
            file_name,
        }),
        DownloadDestination::FixedDirectory(directory) => Ok(ActiveTabDownloadTarget::Ready {
            url: tab.url().clone(),
            target_path: unique_download_target_path(directory, &file_name),
        }),
    }
}

fn default_download_directory() -> Result<PathBuf, String> {
    UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
        .ok_or_else(|| "Downloads directory is unavailable.".to_string())
}

fn suggested_download_file_name(url: &UrlText, title: &str) -> String {
    let parsed = Url::parse(url.as_str()).ok();
    let url_file_name = parsed
        .as_ref()
        .filter(|url| !url.path().ends_with('/'))
        .and_then(|url| url.path_segments())
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .map(str::to_string);
    let raw_file_name = url_file_name
        .or_else(|| page_title_file_name(title))
        .or_else(|| {
            parsed.as_ref().and_then(|url| url.host_str().map(|host| format!("{host}.html")))
        })
        .unwrap_or_else(|| "download.html".to_string());

    sanitize_download_file_name(&raw_file_name)
}

fn page_title_file_name(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty()).then(|| format!("{title}.html"))
}

fn sanitize_download_file_name(file_name: &str) -> String {
    let cleaned = file_name
        .trim()
        .chars()
        .filter_map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => Some('-'),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .take(160)
        .collect::<String>();
    let cleaned = cleaned.trim_matches(|ch| ch == '.' || ch == ' ').to_string();

    match cleaned.as_str() {
        "" | "." | ".." => "download.html".to_string(),
        _ => cleaned,
    }
}

fn unique_download_target_path(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let (stem, extension) = split_file_name(file_name);
    for suffix in 1..1_000 {
        let file_name = match extension {
            Some(extension) => format!("{stem} ({suffix}).{extension}"),
            None => format!("{stem} ({suffix})"),
        };
        let candidate = directory.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    directory.join(format!("{stem} (1000)"))
}

fn split_file_name(file_name: &str) -> (&str, Option<&str>) {
    match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            (stem, Some(extension))
        }
        _ => (file_name, None),
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use ely_domain::UrlText;

    use super::{suggested_download_file_name, unique_download_target_path};

    #[test]
    fn suggested_download_file_name_prefers_url_leaf() -> Result<(), Box<dyn Error>> {
        let url = UrlText::parse("https://example.com/files/report.pdf?token=1")?;

        assert_eq!(suggested_download_file_name(&url, "Example"), "report.pdf");
        Ok(())
    }

    #[test]
    fn suggested_download_file_name_uses_title_for_directory_urls() -> Result<(), Box<dyn Error>> {
        let url = UrlText::parse("https://example.com/articles/")?;

        assert_eq!(suggested_download_file_name(&url, "A/B: Research"), "A-B- Research.html");
        Ok(())
    }

    #[test]
    fn unique_download_target_path_preserves_existing_files() -> Result<(), Box<dyn Error>> {
        let directory = std::env::temp_dir().join("ely-unique-download-test");
        fs::create_dir_all(&directory)?;
        let existing = directory.join("report.pdf");
        fs::write(&existing, [])?;

        assert_eq!(
            unique_download_target_path(&directory, "report.pdf"),
            directory.join("report (1).pdf")
        );

        fs::remove_file(existing)?;
        fs::remove_dir(directory)?;
        Ok(())
    }
}
