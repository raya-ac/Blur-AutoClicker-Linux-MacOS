use crate::app_state::ClickerState;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

static LAST_ZONE_SHOW: Mutex<Option<Instant>> = Mutex::new(None);
pub static OVERLAY_THREAD_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE,
    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMNCRP_DISABLED};

pub fn init_overlay(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| "Overlay window not found".to_string())?;

    log::info!("[Overlay] Running one-time init...");

    // Hide first so nothing flashes on screen during setup.
    let _ = window.hide();

    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    let _ = window.set_decorations(false);

    #[cfg(target_os = "windows")]
    {
        let _ = window.set_fullscreen(true);
        apply_win32_styles(&window)?;
    }

    // macOS fullscreen modes (both native and simple) add an opaque background
    // that kills transparency. Instead, manually size the window to cover the
    // whole screen and rely on transparent:true + macOSPrivateApi.
    #[cfg(target_os = "macos")]
    {
        if let Ok(Some(monitor)) = app.primary_monitor() {
            let scale = monitor.scale_factor();
            let w = (monitor.size().width as f64 / scale) as u32;
            let h = (monitor.size().height as f64 / scale) as u32;
            let _ = window.set_position(tauri::LogicalPosition::new(0.0, 0.0));
            let _ = window.set_size(tauri::LogicalSize::new(w, h));
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = window.set_fullscreen(true);
    }

    log::info!("[Overlay] Init complete — window configured but hidden");
    Ok(())
}

pub fn show_overlay(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<ClickerState>();
    if !state.settings_initialized.load(Ordering::SeqCst) {
        return Ok(());
    }
    {
        let settings = state.settings.lock().unwrap();
        if !settings.show_stop_overlay {
            return Ok(());
        }
    }

    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| "Overlay window not found".to_string())?;

    #[cfg(target_os = "windows")]
    {
        let visible = window.is_visible().unwrap_or(false);
        if !visible {
            let hwnd = get_hwnd(&window)?;
            unsafe { ShowWindow(hwnd, 4) };
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = window.show();
    }

    *LAST_ZONE_SHOW.lock().unwrap() = Some(Instant::now());

    // Get screen dimensions
    let monitor = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No primary monitor found".to_string())?;

    let scale = monitor.scale_factor(); // Adjust for display scaling
    let sw = (monitor.size().width as f64 / scale) as u32;
    let sh = (monitor.size().height as f64 / scale) as u32;

    let settings = state.settings.lock().unwrap();
    let _ = window.emit(
        "zone-data",
        serde_json::json!({
            "edgeStopEnabled": settings.edge_stop_enabled,
            "edgeStopTop": settings.edge_stop_top,
            "edgeStopRight": settings.edge_stop_right,
            "edgeStopBottom": settings.edge_stop_bottom,
            "edgeStopLeft": settings.edge_stop_left,
            "cornerStopEnabled": settings.corner_stop_enabled,
            "cornerStopTL": settings.corner_stop_tl,
            "cornerStopTR": settings.corner_stop_tr,
            "cornerStopBL": settings.corner_stop_bl,
            "cornerStopBR": settings.corner_stop_br,
            "screenWidth": sw,
            "screenHeight": sh,
            "_showDisabledEdges": !settings.edge_stop_enabled,
            "_showDisabledCorners": !settings.corner_stop_enabled,
        }),
    );

    Ok(())
}

// ---- Background timer ----

pub fn check_auto_hide(app: &AppHandle) {
    let mut last = LAST_ZONE_SHOW.lock().unwrap();
    if let Some(instant) = *last {
        if instant.elapsed() >= Duration::from_secs(3) {
            // ↑ auto-hide after timer

            *last = None;
            if let Some(window) = app.get_webview_window("overlay") {
                log::info!("[Overlay] Auto-hide: hiding window");
                #[cfg(target_os = "windows")]
                {
                    if let Ok(hwnd) = get_hwnd(&window) {
                        unsafe { ShowWindow(hwnd, 0) };
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = window.hide();
                }
            }
        }
    }
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), String> {
    *LAST_ZONE_SHOW.lock().unwrap() = None;
    if let Some(window) = app.get_webview_window("overlay") {
        #[cfg(target_os = "windows")]
        {
            if let Ok(hwnd) = get_hwnd(&window) {
                unsafe { ShowWindow(hwnd, 0) };
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window.hide();
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_hwnd(window: &tauri::WebviewWindow) -> Result<isize, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(w) => Ok(w.hwnd.get()),
        _ => Err("Not a Win32 window".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn apply_win32_styles(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = get_hwnd(window)?;

    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        SetWindowLongW(hwnd, GWL_STYLE, ((style as u32) | 0x8000_0000) as i32);

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let new_ex =
            ((ex as u32) | 0x0800_0000 | 0x0000_0080 | 0x0000_0020 | 0x0000_0008) & !0x0004_0000;
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex as i32);

        let policy = DWMNCRP_DISABLED;
        DwmSetWindowAttribute(
            hwnd,
            2,
            &policy as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }

    log::info!("[Overlay] Win32 styles applied");
    Ok(())
}
