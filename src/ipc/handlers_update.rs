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

        let update_path = current_exe.with_extension("update.download");

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

        if !url.to_ascii_lowercase().ends_with(".zip") {
            let resp = IpcResponse::err(
                "update_download_result",
                "Update asset must be a release bundle (.zip) containing Ironbullet and reqflow-sidecar".into(),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        let bundle_path = current_exe.with_extension("update.zip");
        if let Err(e) = std::fs::rename(&update_path, &bundle_path) {
            let resp = IpcResponse::err(
                "update_download_result",
                format!("Cannot stage update bundle: {}", e),
            );
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        if let Err(e) = stage_bundle_update(&current_exe, &bundle_path) {
            let _ = std::fs::remove_file(&bundle_path);
            let resp = IpcResponse::err("update_download_result", e);
            eval_js(format!(
                "window.__ipc_callback({})",
                serde_json::to_string(&resp).unwrap()
            ));
            return;
        }

        let resp = IpcResponse::ok(
            "update_download_result",
            json!({ "success": true, "staged": true }),
        );
        eval_js(format!(
            "window.__ipc_callback({})",
            serde_json::to_string(&resp).unwrap()
        ));
    });
}

fn stage_bundle_update(
    current_exe: &std::path::Path,
    bundle_path: &std::path::Path,
) -> Result<(), String> {
    let main_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Cannot determine main executable name".to_string())?;
    #[cfg(target_os = "windows")]
    let sidecar_name = "reqflow-sidecar.exe";
    #[cfg(not(target_os = "windows"))]
    let sidecar_name = "reqflow-sidecar";

    let file =
        std::fs::File::open(bundle_path).map_err(|e| format!("Cannot read update bundle: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Cannot open update bundle: {e}"))?;
    let mut main_found = false;
    let mut sidecar_found = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("Cannot inspect update bundle: {e}"))?;
        let name = entry.name();
        if name == main_name {
            main_found = true;
        }
        if name == sidecar_name {
            sidecar_found = true;
        }
    }
    if !main_found || !sidecar_found {
        return Err(format!(
            "Update bundle must contain {main_name} and {sidecar_name} at its root"
        ));
    }

    let install_dir = current_exe
        .parent()
        .ok_or_else(|| "Cannot determine install directory".to_string())?;
    let pid = std::process::id();
    spawn_update_helper(pid, current_exe, bundle_path, install_dir, sidecar_name)
}

#[cfg(target_os = "windows")]
fn spawn_update_helper(
    pid: u32,
    current_exe: &std::path::Path,
    bundle_path: &std::path::Path,
    install_dir: &std::path::Path,
    _sidecar_name: &str,
) -> Result<(), String> {
    let helper = std::env::temp_dir().join(format!("ironbullet-update-{pid}.cmd"));
    let escape = |path: &std::path::Path| path.display().to_string().replace('"', "\"\"");
    let script = format!(
        "@echo off\r\n:wait\r\ntasklist /FI \"PID eq {pid}\" /NH | findstr /C:\"{pid}\" >nul\r\nif not errorlevel 1 (timeout /t 1 /nobreak >nul & goto wait)\r\npowershell -NoProfile -ExecutionPolicy Bypass -Command \"Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force\"\r\nstart \"\" \"{}\"\r\ndel /q \"{}\"\r\ndel /q \"%~f0\"\r\n",
        escape(bundle_path), escape(install_dir), escape(current_exe), escape(bundle_path)
    );
    std::fs::write(&helper, script).map_err(|e| format!("Cannot create update helper: {e}"))?;
    std::process::Command::new("cmd")
        .args(["/C", &helper.display().to_string()])
        .spawn()
        .map_err(|e| format!("Cannot start update helper: {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_update_helper(
    pid: u32,
    current_exe: &std::path::Path,
    bundle_path: &std::path::Path,
    install_dir: &std::path::Path,
    sidecar_name: &str,
) -> Result<(), String> {
    let helper = std::env::temp_dir().join(format!("ironbullet-update-{pid}.sh"));
    let quote = |path: &std::path::Path| {
        format!(
            "'{}'",
            path.display().to_string().replace('\'', "'\\\"'\\\"'")
        )
    };
    let script = format!(
        "#!/bin/sh\nwhile kill -0 {pid} 2>/dev/null; do sleep 1; done\nunzip -oq {bundle} -d {dir}\nchmod +x {main} {sidecar}\n{main} &\nrm -f {bundle} \"$0\"\n",
        bundle = quote(bundle_path),
        dir = quote(install_dir),
        main = quote(current_exe),
        sidecar = quote(&install_dir.join(sidecar_name)),
    );
    std::fs::write(&helper, script).map_err(|e| format!("Cannot create update helper: {e}"))?;
    std::process::Command::new("sh")
        .arg(&helper)
        .spawn()
        .map_err(|e| format!("Cannot start update helper: {e}"))?;
    Ok(())
}

/// Returns true only when a valid SemVer release is newer than the current build.
fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |value: &str| semver::Version::parse(value.trim_start_matches('v'));
    match (parse(latest), parse(current)) {
        (Ok(latest), Ok(current)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::version_is_newer;

    #[test]
    fn version_comparison_honors_prerelease_semver() {
        assert!(version_is_newer("v0.6.2-rc.2", "0.6.2-rc.1"));
        assert!(version_is_newer("0.6.2", "0.6.2-rc.2"));
        assert!(!version_is_newer("0.6.2-rc.1", "0.6.2-rc.1"));
        assert!(!version_is_newer("0.6.2-rc.1", "0.6.2"));
        assert!(!version_is_newer("not-a-version", "0.6.2"));
    }
}
