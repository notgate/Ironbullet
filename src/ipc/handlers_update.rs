use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{AppState, IpcResponse};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "notgate/Ironbullet";

/// Return current app version info (no network call)
pub fn get_app_info() -> IpcResponse {
    IpcResponse::ok(
        "app_info",
        serde_json::json!({
            "version": CURRENT_VERSION,
        }),
    )
}

/// Check GitHub for the latest release
pub fn check_for_updates(_state: Arc<Mutex<AppState>>, eval_js: impl Fn(String) + Send + 'static) {
    tokio::spawn(async move {
        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO
        );

        let client = reqwest::Client::new();
        let result = client
            .get(&url)
            .header("User-Agent", format!("ironbullet/{}", CURRENT_VERSION))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;

        let resp = match result {
            Ok(r) => r,
            Err(e) => {
                let resp = IpcResponse::err("update_check_result", format!("Network error: {}", e));
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }
        };

        if !resp.status().is_success() {
            let resp = IpcResponse::err(
                "update_check_result",
                format!("GitHub API returned {}", resp.status()),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                let resp = IpcResponse::err("update_check_result", format!("Parse error: {}", e));
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }
        };

        let latest_tag = body["tag_name"].as_str().unwrap_or("v0.0.0");
        let latest_version = latest_tag.trim_start_matches('v');
        let release_name = body["name"].as_str().unwrap_or(latest_tag);
        let release_notes = body["body"].as_str().unwrap_or("");
        let published_at = body["published_at"].as_str().unwrap_or("");

        // Find the platform-appropriate asset
        // On Windows: prefer a zip containing "windows", fallback to bare .exe
        // On Linux: prefer a zip containing "linux"
        #[cfg(target_os = "windows")]
        let platform_hint = "windows";
        #[cfg(not(target_os = "windows"))]
        let platform_hint = "linux";

        let download_url = body["assets"]
            .as_array()
            .and_then(|assets| {
                // First pass: zip containing platform hint
                assets
                    .iter()
                    .find_map(|a| {
                        let name = a["name"].as_str().unwrap_or("").to_lowercase();
                        if name.contains(platform_hint) && name.ends_with(".zip") {
                            a["browser_download_url"].as_str().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        // Second pass: bare .exe fallback (Windows only)
                        assets.iter().find_map(|a| {
                            let name = a["name"].as_str().unwrap_or("");
                            if name.ends_with(".exe") {
                                a["browser_download_url"].as_str().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                    })
            })
            .unwrap_or_default();

        let has_update = version_is_newer(latest_version, CURRENT_VERSION);

        let resp = IpcResponse::ok(
            "update_check_result",
            json!({
                "has_update": has_update,
                "current_version": CURRENT_VERSION,
                "latest_version": latest_version,
                "release_name": release_name,
                "release_notes": release_notes,
                "published_at": published_at,
                "download_url": download_url,
            }),
        );
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap()
        ));
    });
}

/// Download and install an update from the given URL
pub fn download_update(
    _state: Arc<Mutex<AppState>>,
    data: serde_json::Value,
    eval_js: impl Fn(String) + Send + 'static,
) {
    let url = data["url"].as_str().unwrap_or("").to_string();
    if url.is_empty() {
        let resp = IpcResponse::err("update_download_result", "No download URL".into());
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap()
        ));
        return;
    }

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        // Send progress: started
        let progress = IpcResponse::ok(
            "update_progress",
            json!({ "stage": "downloading", "percent": 0 }),
        );
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&progress).unwrap()
        ));

        let result = client
            .get(&url)
            .header("User-Agent", format!("ironbullet/{}", CURRENT_VERSION))
            .send()
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => {
                let resp =
                    IpcResponse::err("update_download_result", format!("Download failed: {}", e));
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }
        };

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        // Determine paths
        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                let resp = IpcResponse::err(
                    "update_download_result",
                    format!("Cannot find exe path: {}", e),
                );
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }
        };

        let update_path = current_exe.with_extension("update.exe");
        let backup_path = current_exe.with_extension("old.exe");

        // Download to temp file with progress
        let mut file = match tokio::fs::File::create(&update_path).await {
            Ok(f) => f,
            Err(e) => {
                let resp = IpcResponse::err(
                    "update_download_result",
                    format!("Cannot create temp file: {}", e),
                );
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }
        };

        use tokio::io::AsyncWriteExt;
        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        let mut last_pct = 0u8;
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let resp = IpcResponse::err(
                        "update_download_result",
                        format!("Download error: {}", e),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap()
                    ));
                    return;
                }
            };

            if let Err(e) = file.write_all(&chunk).await {
                let resp =
                    IpcResponse::err("update_download_result", format!("Write error: {}", e));
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&resp).unwrap()
                ));
                return;
            }

            downloaded += chunk.len() as u64;
            let pct = if total_size > 0 {
                ((downloaded as f64 / total_size as f64) * 100.0) as u8
            } else {
                50 // indeterminate
            };

            // Only send progress updates at each percentage point
            if pct != last_pct {
                last_pct = pct;
                let progress = IpcResponse::ok(
                    "update_progress",
                    json!({ "stage": "downloading", "percent": pct }),
                );
                eval_js(format!(
                    "window.__ipc_callback({})",
                    serde_json::to_string(&progress).unwrap()
                ));
            }
        }

        drop(file);

        // Send progress: installing
        let progress = IpcResponse::ok(
            "update_progress",
            json!({ "stage": "installing", "percent": 100 }),
        );
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&progress).unwrap()
        ));

        // If the download is a zip, extract the exe from it
        let exe_to_install = if url.ends_with(".zip") {
            let zip_data = match std::fs::read(&update_path) {
                Ok(d) => d,
                Err(e) => {
                    let resp = IpcResponse::err(
                        "update_download_result",
                        format!("Cannot read zip: {}", e),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap()
                    ));
                    return;
                }
            };

            let cursor = std::io::Cursor::new(zip_data);
            let mut archive = match zip::ZipArchive::new(cursor) {
                Ok(a) => a,
                Err(e) => {
                    let resp = IpcResponse::err(
                        "update_download_result",
                        format!("Cannot open zip: {}", e),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap()
                    ));
                    return;
                }
            };

            // Find the main executable in the zip (the one matching our binary name, not sidecar)
            let exe_name = current_exe
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ironbullet.exe")
                .to_string();

            let exe_idx = (0..archive.len()).find(|&i| {
                archive
                    .by_index(i)
                    .ok()
                    .map(|f| {
                        let name = f.name().to_lowercase();
                        let fname = std::path::Path::new(&name)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        fname == exe_name.to_lowercase()
                            || (fname.ends_with(".exe") && !fname.contains("sidecar"))
                    })
                    .unwrap_or(false)
            });

            let idx = match exe_idx {
                Some(i) => i,
                None => {
                    let resp = IpcResponse::err(
                        "update_download_result",
                        "No executable found in zip".into(),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap()
                    ));
                    return;
                }
            };

            let extracted_path = update_path.with_extension("extracted.exe");
            {
                let mut zip_file = archive.by_index(idx).unwrap();
                let mut out = match std::fs::File::create(&extracted_path) {
                    Ok(f) => f,
                    Err(e) => {
                        let resp = IpcResponse::err(
                            "update_download_result",
                            format!("Cannot create extracted file: {}", e),
                        );
                        eval_js(format!(
                            "window.__ipc_callback({})",
                            serde_json::to_string(&resp).unwrap()
                        ));
                        return;
                    }
                };
                use std::io::copy;
                if let Err(e) = copy(&mut zip_file, &mut out) {
                    let resp = IpcResponse::err(
                        "update_download_result",
                        format!("Extraction failed: {}", e),
                    );
                    eval_js(format!(
                        "window.__ipc_callback({})",
                        serde_json::to_string(&resp).unwrap()
                    ));
                    return;
                }
            }
            let _ = std::fs::remove_file(&update_path);
            extracted_path
        } else {
            update_path
        };

        // Rename current exe to .old, install new one
        if backup_path.exists() {
            let _ = std::fs::remove_file(&backup_path);
        }

        if let Err(e) = std::fs::rename(&current_exe, &backup_path) {
            let resp = IpcResponse::err(
                "update_download_result",
                format!(
                    "Cannot rename current exe: {} — try running as administrator",
                    e
                ),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        if let Err(e) = std::fs::rename(&exe_to_install, &current_exe) {
            // Restore backup
            let _ = std::fs::rename(&backup_path, &current_exe);
            let resp = IpcResponse::err(
                "update_download_result",
                format!("Cannot install update: {}", e),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        let resp = IpcResponse::ok("update_download_result", json!({ "success": true }));
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap()
        ));
    });
}

/// Simple semver comparison: returns true if `latest` > `current`
fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.').filter_map(|s| s.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(latest) > parse(current)
}
