use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{error, info};

pub trait Delegate {
    /// Returns the path to the attachments directory.
    fn attachments_dir(&self) -> &Path;
    /// Updates the path to the local snapshot of an external link.
    fn update_local_link(&self, external_link: &str, local_path: &Path);
}

pub async fn download_link<D>(url: &str, delegate: D)
where
    D: Delegate,
{
    download_webpage(url, delegate).await
}

async fn download_webpage<D>(url: &str, delegate: D)
where
    D: Delegate,
{
    info!("Downloading webpage {}", url);

    let webpages_dir = delegate.attachments_dir().join("webpages");
    std::fs::create_dir_all(&webpages_dir).unwrap();

    let filename = format!("{}.html", util::url_to_safe_filename(url));
    let filepath = webpages_dir.join(filename);
    let filepath = filepath.to_string_lossy();

    let result = Command::new("monolith")
        .args(&[url, "-o", &filepath])
        .output()
        .await;

    if let Err(err) = result {
        error!("Failed to download webpage {}: {}", url, err);
    } else {
        info!("Downloaded webpage {} to {}", url, filepath);
        let filepath = PathBuf::from(filepath.as_ref());
        delegate.update_local_link(url, &filepath);
    }
}

mod util {

    pub fn url_to_safe_filename(url: &str) -> String {
        let mut safe_name = String::with_capacity(url.len());

        let stripped_url = url
            .trim()
            .strip_prefix("http://")
            .unwrap_or(url)
            .strip_prefix("https://")
            .unwrap_or(url);

        for c in stripped_url.chars() {
            match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => safe_name.push('_'),
                c if c.is_alphanumeric() || c == '-' || c == '.' || c == '_' => safe_name.push(c),
                _ => safe_name.push('_'),
            }
        }

        safe_name.trim_matches(|c| c == '.' || c == ' ').to_string()
    }
}
