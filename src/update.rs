use anyhow::{Context, Result, anyhow};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const REPOSITORY: &str = "Runa-chama/SlideVideo_Divider";
const USER_AGENT: &str = "SlideVideoDivider";

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub notes: String,
    pub installer_name: String,
    pub installer_url: String,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn check_for_update() -> Result<Option<ReleaseInfo>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("{USER_AGENT}/{}", current_version()))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .context("更新確認クライアントを作成できませんでした")?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("更新情報を取得できませんでした")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let json: serde_json::Value = response
        .error_for_status()
        .context("更新情報の取得に失敗しました")?
        .text()
        .context("更新情報を読み取れませんでした")
        .and_then(|text| serde_json::from_str(&text).context("更新情報を解析できませんでした"))?;
    let tag = json["tag_name"].as_str().unwrap_or_default();
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    if version.is_empty() || !is_newer(&version, current_version()) {
        return Ok(None);
    }
    let Some(asset) = json["assets"].as_array().and_then(|assets| {
        assets.iter().find(|asset| {
            asset["name"]
                .as_str()
                .is_some_and(|name| name.ends_with("-setup.exe"))
        })
    }) else {
        return Ok(None);
    };
    let installer_url = asset["browser_download_url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let installer_name = asset["name"].as_str().unwrap_or_default().to_string();
    if installer_url.is_empty() || installer_name.is_empty() {
        return Ok(None);
    }
    Ok(Some(ReleaseInfo {
        version,
        notes: json["body"].as_str().unwrap_or_default().to_string(),
        installer_name,
        installer_url,
    }))
}

pub fn download_installer(
    release: &ReleaseInfo,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<PathBuf> {
    let directory = std::env::temp_dir().join("SlideVideoDivider-update");
    std::fs::create_dir_all(&directory).context("更新用フォルダを作成できませんでした")?;
    let path = directory.join(&release.installer_name);
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("{USER_AGENT}/{}", current_version()))
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(60 * 60))
        .build()
        .context("更新ダウンロード用クライアントを作成できませんでした")?;
    let mut response = client
        .get(&release.installer_url)
        .send()
        .context("更新インストーラのダウンロードを開始できませんでした")?
        .error_for_status()?;
    let total = response.content_length();
    let mut file = File::create(&path).context("更新インストーラを保存できませんでした")?;
    let mut buffer = vec![0u8; 128 * 1024];
    let mut written = 0u64;
    let mut last_percent = -1i32;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(&path);
            return Err(anyhow!("更新のダウンロードをキャンセルしました"));
        }
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        written += read as u64;
        if let Some(total) = total.filter(|total| *total > 0) {
            let percent = (written as f64 / total as f64 * 100.0) as i32;
            if percent != last_percent {
                last_percent = percent;
                progress(written, Some(total));
            }
        } else {
            progress(written, None);
        }
    }
    file.flush()?;
    drop(file);
    if written == 0 {
        return Err(anyhow!("更新インストーラをダウンロードできませんでした"));
    }
    Ok(path)
}

/// サイレント更新を起動し、完了後にアプリを再起動する。
pub fn launch_installer(path: &Path) -> Result<()> {
    let app = std::env::current_exe().context("実行ファイルの場所を取得できませんでした")?;
    let directory = std::env::temp_dir().join("SlideVideoDivider-update");
    std::fs::create_dir_all(&directory).context("更新用フォルダを作成できませんでした")?;
    let script = directory.join("apply-update.cmd");
    // cmd.exe へ複雑な引用符付きコマンドを直接渡すと引数解釈が崩れるため、
    // 更新→再起動の手順をバッチファイルに書き出して実行する。
    let batch = format!(
        "@echo off\r\nstart \"\" /wait \"{}\" /SILENT /NORESTART /CLOSEAPPLICATIONS\r\nif errorlevel 1 exit /b 1\r\nstart \"\" \"{}\"\r\n",
        path.display(),
        app.display()
    );
    std::fs::write(&script, batch).context("更新用バッチファイルを作成できませんでした")?;
    let mut process = std::process::Command::new("cmd");
    process.arg("/C").arg(&script);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    process
        .spawn()
        .context("更新インストーラを起動できませんでした")?;
    Ok(())
}

fn is_newer(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
}

fn version_parts(text: &str) -> (u64, u64, u64) {
    let mut parts = text.trim().trim_start_matches(['v', 'V']).split('.');
    let mut next = || {
        parts
            .next()
            .and_then(|part| part.split(['-', '+']).next())
            .and_then(|part| part.parse::<u64>().ok())
            .unwrap_or(0)
    };
    (next(), next(), next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("v0.10.0", "0.9.0"));
    }
}
