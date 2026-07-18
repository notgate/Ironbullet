#![windows_subsystem = "windows"]

mod ipc;

use std::sync::Arc;
use tokio::sync::Mutex;

use include_dir::{include_dir, Dir};
#[cfg(target_os = "windows")]
use raw_window_handle::HasWindowHandle;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::{Icon, WindowBuilder};
use wry::WebViewBuilder;

use ipc::{AppState, IpcCmd};
use ironbullet::config::{autosave_path, load_config, save_config};

/// Check if WebView2 runtime is installed (Windows only).
/// On Linux, check that WebKitGTK (required by wry) is actually installed.
/// Returns None if everything looks OK, or Some(install_instructions) if the
/// required library isn't found.
///
/// NOTE: This can only catch missing libs if wry was compiled with a
/// WEAK/optional link — otherwise the dynamic linker will crash before main().
/// The real protection is the start.sh launcher script packaged in the zip.
#[cfg(target_os = "linux")]
fn check_linux_webview() -> Option<String> {
    use std::path::Path;

    // wry 0.x links against webkit2gtk — check the most common install paths.
    let candidates = [
        // Ubuntu 22.04+ / Debian 12+
        "/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0",
        "/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.0.so.37",
        // Fedora / RHEL
        "/usr/lib64/libwebkit2gtk-4.1.so.0",
        "/usr/lib64/libwebkit2gtk-4.0.so.37",
        // Arch / generic
        "/usr/lib/libwebkit2gtk-4.1.so.0",
        "/usr/lib/libwebkit2gtk-4.0.so.37",
    ];

    for lib in &candidates {
        if Path::new(lib).exists() {
            return None; // Found — we're good
        }
    }

    // Also check via ldconfig cache (most distros)
    if std::process::Command::new("ldconfig")
        .args(["-p"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.contains("libwebkit2gtk"))
        .unwrap_or(false)
    {
        return None;
    }

    Some(
        "WebKitGTK is required to run IronBullet but was not found.\n\n\
         Install it for your distribution:\n\
         \n\
         • Ubuntu / Debian (22.04+):  sudo apt install libwebkit2gtk-4.1-0\n\
         • Ubuntu / Debian (20.04):   sudo apt install libwebkit2gtk-4.0-37\n\
         • Fedora / RHEL:             sudo dnf install webkit2gtk4.1\n\
         • Arch Linux:                sudo pacman -S webkit2gtk-4.1\n\
         • openSUSE:                  sudo zypper install libwebkit2gtk-4_1-0\n\
         \n\
         After installing, restart IronBullet.\n\
         Alternatively, use the start.sh launcher script from the zip."
            .into(),
    )
}

/// Checks known install locations for the EdgeWebView application directory.
#[cfg(target_os = "windows")]
fn check_webview2_available() -> bool {
    use std::path::Path;

    // Common WebView2 runtime install locations
    let candidate_dirs: &[&str] = &[
        r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
        r"C:\Program Files\Microsoft\EdgeWebView\Application",
        r"C:\Program Files (x86)\Microsoft\Edge\Application",
    ];
    for dir in candidate_dirs {
        if Path::new(dir).exists() {
            return true;
        }
    }

    // Check per-user WebView2 install location
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let user_path = format!(r"{}\Microsoft\EdgeWebView\Application", local_app_data);
        if Path::new(&user_path).exists() {
            return true;
        }
    }

    false
}

/// Native Win32 title bar drag via WM_NCHITTEST.
/// Returns HTCAPTION for the top 28px (title bar) so Windows handles
/// drag natively — no IPC round-trip, works even with webview dialog overlays.
#[cfg(target_os = "windows")]
mod win32_titlebar {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, GetWindowRect, SetWindowLongPtrW, GWLP_WNDPROC, HTCAPTION, HTCLIENT,
        WM_NCHITTEST, WNDPROC,
    };

    static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

    unsafe extern "system" fn custom_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let orig: WNDPROC = std::mem::transmute(ORIGINAL_WNDPROC.load(Ordering::Relaxed));
        if msg == WM_NCHITTEST {
            let result = CallWindowProcW(orig, hwnd, msg, wparam, lparam);
            if result == HTCLIENT as isize {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                let dpi = GetDpiForWindow(hwnd);
                let scale = dpi as f64 / 96.0;
                // 28 CSS px title bar, 3×38=114 CSS px chrome buttons on the right
                let title_h = (28.0 * scale) as i32;
                let buttons_w = (114.0 * scale) as i32;
                if y - rect.top < title_h && x < rect.right - buttons_w {
                    return HTCAPTION as isize;
                }
            }
            return result;
        }
        CallWindowProcW(orig, hwnd, msg, wparam, lparam)
    }

    pub unsafe fn install(hwnd: HWND) {
        let orig = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, custom_wndproc as *const () as isize);
        ORIGINAL_WNDPROC.store(orig, Ordering::Relaxed);
    }
}

static GUI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/gui/build");

fn mime_for(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else {
        "application/octet-stream"
    }
}

enum Evt {
    EvalJs(String),
    DragWindow,
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    /// Open a panel as a real native OS window (wry/tao multi-window).
    FloatPanel(String),
}

/// Check if we should run in CLI mode (any --config or --help arg present)
fn is_cli_mode() -> bool {
    std::env::args().any(|a| a == "--config" || a == "-c" || a == "--help" || a == "-h")
}

/// Attach to parent console on Windows so CLI output is visible.
/// Required because #![windows_subsystem = "windows"] hides the console.
#[cfg(target_os = "windows")]
fn attach_console() {
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(
            windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_console() {}

fn main() {
    if is_cli_mode() {
        attach_console();
        run_cli();
    } else {
        run_gui();
    }
}

fn run_cli() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match ironbullet::cli::parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            eprintln!("run with --help for usage");
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let result = rt.block_on(ironbullet::cli::run(cli));
    ironbullet::sidecar::xray_pool::shutdown_all();
    ironbullet::sidecar::shadowsocks_pool::shutdown_all();
    if let Err(error) = result {
        eprintln!("error: {}", error);
        std::process::exit(1);
    }
}

/// Position window on screen. Computes physical window size from known logical
/// dimensions × DPI (avoids unreliable `outer_size()` on freshly-created windows).
///
/// 1. If saved position exists AND the window's center would be on a visible
///    monitor → restore it with PhysicalPosition.
/// 2. Otherwise → center on the primary monitor's work area (rcWork, excludes
///    taskbar) — the same approach as WPF's WindowStartupLocation.CenterScreen.
fn position_window(
    window: &tao::window::Window,
    saved_x: Option<i32>,
    saved_y: Option<i32>,
    logical_w: f64,
    logical_h: f64,
) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONULL,
            MONITOR_DEFAULTTOPRIMARY,
        };

        // Physical window size from known logical dimensions
        let scale = window.scale_factor();
        let win_w = (logical_w * scale) as i32;
        let win_h = (logical_h * scale) as i32;

        // Try restoring saved position — validate that the window's CENTER
        // would land on a visible monitor (not just the top-left corner)
        if let (Some(x), Some(y)) = (saved_x, saved_y) {
            let cx = x + win_w / 2;
            let cy = y + win_h / 2;
            let center_visible = unsafe {
                !MonitorFromPoint(POINT { x: cx, y: cy }, MONITOR_DEFAULTTONULL).is_null()
            };
            if center_visible {
                window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
                return;
            }
        }

        // Center on primary monitor's work area.
        // (0, 0) is always on the primary monitor in Windows coordinate space.
        unsafe {
            let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            if !hmon.is_null() {
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(hmon, &mut info) != 0 {
                    let rc = info.rcWork;
                    let work_w = rc.right - rc.left;
                    let work_h = rc.bottom - rc.top;
                    let x = rc.left + (work_w - win_w) / 2;
                    let y = rc.top + (work_h - win_h) / 2;
                    window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
                }
            }
        }
        return;
    }

    // Non-Windows fallback
    #[cfg(not(target_os = "windows"))]
    {
        if let (Some(x), Some(y)) = (saved_x, saved_y) {
            window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
        } else if let Some(monitor) = window
            .primary_monitor()
            .or_else(|| window.current_monitor())
        {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let scale = window.scale_factor();
            let win_w = (logical_w * scale) as i32;
            let win_h = (logical_h * scale) as i32;
            let x = mon_pos.x + ((mon_size.width as i32 - win_w) / 2);
            let y = mon_pos.y + ((mon_size.height as i32 - win_h) / 2);
            window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
        }
    }
}

fn run_gui() {
    // Linux WebView compatibility fixes — must be set before GTK/GDK init.
    //
    // Known causes of a gray/blank WebView on Linux:
    //   1. NVIDIA GPU on Wayland (GNOME 46+): WebKitGTK's EGL/Wayland renderer
    //      is broken with NVIDIA. Force XWayland via GDK_BACKEND=x11.
    //   2. WebKit GPU compositing unavailable (VMs, software GL, some Mesa builds).
    //   3. DMABUF renderer (webkit2gtk 2.40+) broken on certain AMD/Intel stacks.
    //   4. WebKit sandbox process fails to launch (apparmor, seccomp, certain NVIDIA
    //      driver versions) — WEBKIT_FORCE_SANDBOX=0 disables the sandbox entirely.
    //
    // All vars are only set if not already in the environment so the user
    // can override any of them from the shell if needed.
    #[cfg(target_os = "linux")]
    {
        // Force XWayland — the single most effective fix for NVIDIA + Wayland
        if std::env::var("GDK_BACKEND").is_err() {
            unsafe {
                std::env::set_var("GDK_BACKEND", "x11");
            }
        }
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            }
        }
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
        // Disable the WebKit sandbox. On some NVIDIA driver versions and certain
        // kernel security configurations the sandbox helper process fails to start,
        // which leaves the WebView rendering a solid gray surface. Setting this to 0
        // skips the sandbox entirely and resolves the remaining gray-screen cases
        // that GDK_BACKEND=x11 alone does not fix.
        if std::env::var("WEBKIT_FORCE_SANDBOX").is_err() {
            unsafe {
                std::env::set_var("WEBKIT_FORCE_SANDBOX", "0");
            }
        }
    }

    // Clean up old binary from previous update
    if let Ok(exe) = std::env::current_exe() {
        let old = exe.with_extension("old.exe");
        if old.exists() {
            let _ = std::fs::remove_file(&old);
        }
    }

    let cfg = load_config();

    // Start tokio runtime in background
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let event_loop: EventLoop<Evt> = tao::event_loop::EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let mut wb = WindowBuilder::new()
        .with_title(format!("Ironbullet {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size(tao::dpi::LogicalSize::new(
            cfg.window_width,
            cfg.window_height,
        ))
        .with_decorations(false);

    // The canonical mark is embedded so release archives do not depend on a
    // loose runtime file for native taskbar/window identity.
    {
        let bytes = include_bytes!("../assets/brand/ironbullet-icon.png");
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = Icon::from_rgba(rgba.into_raw(), w, h) {
                wb = wb.with_window_icon(Some(icon));
            }
        }
    }

    let window = wb.build(&event_loop).expect("Failed to create window");

    // Restore saved position or center on work area (WPF CenterScreen style)
    position_window(
        &window,
        cfg.window_x,
        cfg.window_y,
        cfg.window_width,
        cfg.window_height,
    );

    // Windows-specific: disable rounded corners + install native title bar drag
    #[cfg(target_os = "windows")]
    {
        if let Ok(handle) = window.window_handle() {
            if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
                let hwnd = win32.hwnd.get() as *mut std::ffi::c_void;
                // Disable Windows 11 rounded corners
                use windows_sys::Win32::Graphics::Dwm::{
                    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
                };
                let preference: u32 = DWMWCP_DONOTROUND as u32;
                unsafe {
                    DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                        &preference as *const _ as *const _,
                        std::mem::size_of_val(&preference) as u32,
                    );
                    // Native title bar drag — works even with webview dialog overlays
                    win32_titlebar::install(hwnd);
                }
            }
        }
    }

    let state = Arc::new(Mutex::new(AppState::new()));
    let ipc_proxy = proxy.clone();
    let ipc_state = state.clone();

    // ── WebView2 availability check (Windows only) ──────────────────────────────
    #[cfg(target_os = "windows")]
    {
        let wv2_available = check_webview2_available();
        if !wv2_available {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            let title: Vec<u16> = "IronBullet \u{2014} Missing Dependency\0"
                .encode_utf16()
                .collect();
            let msg: Vec<u16> = "WebView2 runtime was not found.\n\nIronBullet requires the Microsoft Edge WebView2 runtime.\nPlease install it from:\nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703\n\nAfter installing, restart IronBullet.\0".encode_utf16().collect();
            unsafe {
                #[allow(clippy::zero_ptr)]
                MessageBoxW(
                    std::ptr::null_mut(),
                    msg.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
            std::process::exit(1);
        }
    }

    // ── Linux WebKitGTK availability check ───────────────────────────────────
    // NOTE: If WebKitGTK is NOT installed and the binary is dynamically linked,
    // the dynamic linker will crash *before* this code runs. The start.sh
    // launcher script (included in the Linux zip) is the primary protection.
    // This check only fires when the libs are found by linker but have some
    // issue at the GTK level, or on statically-linked builds.
    #[cfg(target_os = "linux")]
    {
        if let Some(msg) = check_linux_webview() {
            eprintln!("IronBullet: {}", msg);
            // Try to show a GTK dialog — best effort, no hard dep on GTK dialog lib
            let _ = std::process::Command::new("zenity")
                .args([
                    "--error",
                    "--title=IronBullet — Missing Dependency",
                    &format!("--text={}", msg),
                ])
                .status();
            std::process::exit(1);
        }
    }

    let webview = WebViewBuilder::new()
        .with_devtools(true)
        .with_custom_protocol("ironbullet".into(), move |_wv, req| {
            use std::borrow::Cow;
            let uri = req.uri().to_string();
            let path = uri.strip_prefix("ironbullet://localhost").unwrap_or(&uri);
            let path = if path.is_empty() || path == "/" {
                "/index.html"
            } else {
                path
            };
            let path = path.trim_start_matches('/');
            // Strip query string
            let path = path.split('?').next().unwrap_or(path);
            let (body, mime): (Cow<'static, [u8]>, &str) = match GUI_DIR.get_file(path) {
                Some(f) => (Cow::Borrowed(f.contents()), mime_for(path)),
                None => match GUI_DIR.get_file("index.html") {
                    Some(f) => (Cow::Borrowed(f.contents()), "text/html"),
                    None => (Cow::Borrowed(b"404 Not Found"), "text/plain"),
                },
            };
            wry::http::Response::builder()
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "no-cache, no-store, must-revalidate")
                .body(body)
                .unwrap()
        })
        .with_url("ironbullet://localhost/")
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let body = req.body();
            let proxy = ipc_proxy.clone();

            // Handle window chrome commands directly
            if let Ok(cmd) = serde_json::from_str::<IpcCmd>(body) {
                match cmd.cmd.as_str() {
                    "drag" => {
                        let _ = proxy.send_event(Evt::DragWindow);
                        return;
                    }
                    "minimize" => {
                        let _ = proxy.send_event(Evt::MinimizeWindow);
                        return;
                    }
                    "maximize" => {
                        let _ = proxy.send_event(Evt::MaximizeWindow);
                        return;
                    }
                    "close" => {
                        let _ = proxy.send_event(Evt::CloseWindow);
                        return;
                    }
                    "float_panel_native" => {
                        // Open a panel as a real native OS window
                        let panel_id = cmd
                            .data
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !panel_id.is_empty() {
                            let _ = proxy.send_event(Evt::FloatPanel(panel_id));
                        }
                        return;
                    }
                    _ => {}
                }

                // Spawn IPC handlers async to prevent blocking the webview
                let eval_proxy = proxy.clone();
                let state_clone = ipc_state.clone();
                tokio::spawn(async move {
                    ipc::handle_ipc_cmd(&cmd, &state_clone, move |js| {
                        let _ = eval_proxy.send_event(Evt::EvalJs(js));
                    });
                });
            }
        })
        // NOTE: with_drag_drop_handler is NOT used here. On Windows (WebView2),
        // registering a custom IDropTarget replaces WebView2's built-in one entirely,
        // breaking internal browser drag-and-drop (block palette, tab reordering).
        // File import uses IPC browse_file dialogs instead (issue #51).
        .build(&window)
        .expect("Failed to create webview");

    // Track panel sub-windows: (native window, webview, panel_id)
    // These are real OS windows created when the user floats a panel.
    let mut panel_windows: Vec<(tao::window::Window, wry::WebView, String)> = Vec::new();

    event_loop.run(move |event, elwt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // Check if it's a panel sub-window being closed
                if let Some(idx) = panel_windows
                    .iter()
                    .position(|(w, _, _)| w.id() == window_id)
                {
                    let (_, _, panel_id) = panel_windows.remove(idx);
                    // Tell main window to re-dock this panel to bottom
                    let js = format!(
                        "if (window.__ibPanelClosed) window.__ibPanelClosed('{}')",
                        panel_id
                    );
                    let _ = webview.evaluate_script(&js);
                } else {
                    // Main window close — redirect to frontend for unsaved-changes prompt
                    let _ = webview.evaluate_script(
                        "window.dispatchEvent(new Event('native-close-requested'))",
                    );
                }
            }
            Event::UserEvent(evt) => match evt {
                Evt::EvalJs(js) => {
                    // Broadcast to main window AND all open panel sub-windows
                    let _ = webview.evaluate_script(&js);
                    for (_, pw, _) in &panel_windows {
                        let _ = pw.evaluate_script(&js);
                    }
                }
                Evt::FloatPanel(panel_id) => {
                    // Focus existing panel window if already open
                    if let Some((pw, _, _)) =
                        panel_windows.iter().find(|(_, _, id)| id == &panel_id)
                    {
                        pw.set_focus();
                        return;
                    }

                    let label = match panel_id.as_str() {
                        "debugger" => "Debugger",
                        "code" => "Code View",
                        "data" => "Data / Proxy",
                        "jobs" => "Jobs",
                        "network" => "Network",
                        "variables" => "Variables",
                        "response-viewer" => "Response Viewer",
                        "inspector" => "Inspect",
                        _ => "Panel",
                    };

                    let panel_window = match WindowBuilder::new()
                        .with_title(format!("IronBullet — {}", label))
                        .with_inner_size(tao::dpi::LogicalSize::new(800.0_f64, 500.0_f64))
                        .with_decorations(true)
                        .build(elwt)
                    {
                        Ok(w) => w,
                        Err(e) => {
                            eprintln!("[float panel] window error: {}", e);
                            return;
                        }
                    };

                    let url = format!("ironbullet://localhost/?panel={}", panel_id);
                    let sub_state = state.clone();
                    let sub_proxy = proxy.clone();

                    let panel_webview = match WebViewBuilder::new()
                        .with_devtools(true)
                        .with_custom_protocol("ironbullet".into(), move |_wv, req| {
                            use std::borrow::Cow;
                            let uri = req.uri().to_string();
                            let path = uri.strip_prefix("ironbullet://localhost").unwrap_or(&uri);
                            let path = if path.is_empty() || path == "/" {
                                "/index.html"
                            } else {
                                path
                            };
                            let path = path.trim_start_matches('/');
                            let path = path.split('?').next().unwrap_or(path);
                            let (body, mime): (Cow<'static, [u8]>, &str) =
                                match GUI_DIR.get_file(path) {
                                    Some(f) => (Cow::Borrowed(f.contents()), mime_for(path)),
                                    None => match GUI_DIR.get_file("index.html") {
                                        Some(f) => (Cow::Borrowed(f.contents()), "text/html"),
                                        None => (Cow::Borrowed(b"404 Not Found"), "text/plain"),
                                    },
                                };
                            wry::http::Response::builder()
                                .header("Content-Type", mime)
                                .header("Access-Control-Allow-Origin", "*")
                                .header("Cache-Control", "no-cache, no-store, must-revalidate")
                                .body(body)
                                .unwrap()
                        })
                        .with_url(&url)
                        .with_ipc_handler(move |req: wry::http::Request<String>| {
                            let body = req.body();
                            if let Ok(cmd) = serde_json::from_str::<IpcCmd>(body) {
                                let eval_proxy = sub_proxy.clone();
                                let state_clone = sub_state.clone();
                                tokio::spawn(async move {
                                    ipc::handle_ipc_cmd(&cmd, &state_clone, move |js| {
                                        let _ = eval_proxy.send_event(Evt::EvalJs(js));
                                    });
                                });
                            }
                        })
                        .build(&panel_window)
                    {
                        Ok(wv) => wv,
                        Err(e) => {
                            eprintln!("[float panel] webview error: {}", e);
                            return;
                        }
                    };

                    // Notify main window that this panel is now in a native window
                    let notify_js = format!(
                        "if (window.__ibPanelOpened) window.__ibPanelOpened('{}')",
                        panel_id
                    );
                    let _ = webview.evaluate_script(&notify_js);

                    panel_windows.push((panel_window, panel_webview, panel_id));
                }
                Evt::DragWindow => {
                    let _ = window.drag_window();
                }
                Evt::MinimizeWindow => {
                    window.set_minimized(true);
                }
                Evt::MaximizeWindow => {
                    window.set_maximized(!window.is_maximized());
                }
                Evt::CloseWindow => {
                    // Save window position & size before closing (same as CloseRequested)
                    let size = window.inner_size();
                    let pos = window.outer_position().ok();
                    let scale = window.scale_factor();
                    let rt = tokio::runtime::Handle::try_current();
                    if let Ok(handle) = rt {
                        let state = state.clone();
                        handle.spawn(async move {
                            let mut s = state.lock().await;
                            s.config.window_width = size.width as f64 / scale;
                            s.config.window_height = size.height as f64 / scale;
                            if let Some(pos) = pos {
                                s.config.window_x = Some(pos.x);
                                s.config.window_y = Some(pos.y);
                            }
                            save_config(&s.config);
                            // Clean up autosave file on normal exit — prevents "unsaved session" dialog on next launch
                            let _ = std::fs::remove_file(autosave_path());
                        });
                    }
                    ironbullet::sidecar::xray_pool::shutdown_all();
                    ironbullet::sidecar::shadowsocks_pool::shutdown_all();
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::LoopDestroyed => {
                ironbullet::sidecar::xray_pool::shutdown_all();
                ironbullet::sidecar::shadowsocks_pool::shutdown_all();
            }
            _ => {}
        }
    });
}
