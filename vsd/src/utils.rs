use anyhow::{Result, bail};
use reqwest::Response;
use std::{env, path::PathBuf};

pub(crate) type QUERY = [(String, String)];

pub async fn fetch_bytes(response: Response) -> Result<Vec<u8>> {
    let status = response.status();

    if !status.is_success() {
        bail!(
            "{} request failed ({}): '{}'",
            response.url().clone(),
            status,
            response.text().await?,
        );
    }

    Ok(response.bytes().await?.to_vec())
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(path) = env::current_dir() {
        paths.push(path);
    }
    if let Some(path) = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|p| p.to_path_buf()))
    {
        paths.push(path);
    }
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    #[cfg(target_os = "windows")]
    let bin = "ffmpeg.exe";
    #[cfg(not(target_os = "windows"))]
    let bin = "ffmpeg";
    paths.into_iter().map(|x| x.join(bin)).find(|x| x.exists())
}

pub fn gen_id(base_url: &str, uri: &str) -> String {
    blake3::hash(format!("{}+{}", base_url, uri).as_bytes()).to_hex()[..7].to_owned()
}
