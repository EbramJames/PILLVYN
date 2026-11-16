pub mod activities;

use crate::commands::AppState;
use crate::config::PillvynConfig;
use crate::island::activities::now_ms;
use crate::services::win_hooks::is_mouse_button_down;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

pub const ISLAND_CANVAS_WIDTH: f64 = 560.0;
pub const ISLAND_CANVAS_HEIGHT: f64 = 500.0;
pub const ISLAND_CANVAS_TOP_PAD: f64 = 14.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HitboxRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for HitboxRect {
    fn default() -> Self {
        Self {
            x: 140.0,
            y: 12.0,
            width: 280.0,
            height: 44.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IslandWindowState {
    pub hitbox: HitboxRect,
    pub force_click_through: bool,
    pub is_visible: bool,
}

impl Default for IslandWindowState {
    fn default() -> Self {
        Self {
            hitbox: HitboxRect::default(),
            force_click_through: false,
            is_visible: true,
        }
    }
}

pub fn list_monitors(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary_name = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());

    let Ok(monitors) = app.available_monitors() else {
        return Vec::new();
    };

    monitors
        .into_iter()
        .enumerate()
        .map(|(idx, m)| {
            let name = m
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", idx + 1));
            let pos = m.position();
            let size = m.size();
            let is_primary = primary_name.as_ref().map_or(idx == 0, |p| p == &name);
            MonitorInfo {
                name,
                width: size.width,
                height: size.height,
                x: pos.x,
                y: pos.y,
                scale_factor: m.scale_factor(),
                is_primary,
            }
        })
        .collect()
}

/// Computes the target physical window `(x, y)` and clamped logical `(x_offset, top_offset)`
/// for a given monitor so the capsule always remains visible inside screen bounds and lifts cleanly
/// when expanded near the bottom edge.
pub fn compute_island_window_position(
    m: &MonitorInfo,
    top_offset: i32,
    x_offset: i32,
    expanded: bool,
) -> ((i32, i32), (i32, i32)) {
    let scale = m.scale_factor.max(1.0);
    let mon_w_logical = (m.width as f64) / scale;
    let mon_h_logical = (m.height as f64) / scale;

    let max_x = ((mon_w_logical / 2.0) - 120.0).max(0.0).round() as i32;
    let max_top = (mon_h_logical - 68.0).max(0.0).round() as i32;

    let clamped_x = x_offset.clamp(-max_x, max_x);
    let clamped_top = top_offset.clamp(0, max_top);
    let effective_top = if expanded {
        let expanded_max_top = (mon_h_logical - 392.0).max(0.0).round() as i32;
        clamped_top.min(expanded_max_top)
    } else {
        clamped_top
    };

    let phys_w = (ISLAND_CANVAS_WIDTH * scale).round() as i32;
    let top_pad_phys = (ISLAND_CANVAS_TOP_PAD * scale).round() as i32;
    let top_offset_phys = (effective_top as f64 * scale).round() as i32;
    let x_offset_phys = (clamped_x as f64 * scale).round() as i32;

    let pos_x = m.x + ((m.width as i32 - phys_w) / 2) + x_offset_phys;
    let pos_y = m.y + top_offset_phys - top_pad_phys;
    ((pos_x, pos_y), (clamped_x, clamped_top))
}

pub fn apply_window_geometry(window: &WebviewWindow, cfg: &PillvynConfig) -> Result<(), String> {
    let app = window.app_handle();
    let expanded = app
        .try_state::<AppState>()
        .and_then(|st| st.island_expanded.lock().ok().map(|g| *g))
        .unwrap_or(false);
    let monitors = list_monitors(app);
    let target = cfg
        .appearance
        .monitor_name
        .as_ref()
        .and_then(|wanted| monitors.iter().find(|m| &m.name == wanted))
        .or_else(|| monitors.iter().find(|m| m.is_primary))
        .or_else(|| monitors.first());

    let _ = window.set_size(LogicalSize::new(ISLAND_CANVAS_WIDTH, ISLAND_CANVAS_HEIGHT));

    if let Some(m) = target {
        let ((pos_x, pos_y), _) = compute_island_window_position(
            m,
            cfg.appearance.top_offset,
            cfg.appearance.x_offset,
            expanded,
        );
        window
            .set_position(PhysicalPosition::new(pos_x, pos_y))
            .map_err(|e| format!("Failed to position island window: {e}"))?;
    }

    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    enforce_tool_window(window);
    Ok(())
}

/// Strips `WS_EX_APPWINDOW` and forces `WS_EX_TOOLWINDOW` on frameless overlay/popup windows
/// so Windows Explorer never flashes temporary taskbar buttons during show/hide/resize calls.
#[cfg(target_os = "windows")]
pub(crate) fn enforce_tool_window(window: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    if let Ok(hwnd_raw) = window.hwnd() {
        let hwnd = HWND(hwnd_raw.0 as *mut _);
        unsafe {
            let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let next = (cur | (WS_EX_TOOLWINDOW.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
            if next != cur {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn enforce_tool_window(_window: &WebviewWindow) {}

/// Starts a very low-overhead cursor hit-test loop that keeps transparent padding
/// around the pill click-through (`WS_EX_TRANSPARENT`) while making the capsule itself interactive.
pub fn spawn_click_through_controller(app: AppHandle, state: Arc<Mutex<IslandWindowState>>) {
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut last_ignoring: Option<bool> = None;
        let mut prev_mouse_down = false;

        loop {
            let (hitbox, force_ct, visible) = match state.lock() {
                Ok(guard) => (guard.hitbox, guard.force_click_through, guard.is_visible),
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    continue;
                }
            };

            if !visible {
                prev_mouse_down = false;
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }

            let Some(window) = app.get_webview_window("island") else {
                std::thread::sleep(std::time::Duration::from_millis(250));
                continue;
            };

            let mut cursor_known = false;
            let should_ignore = if force_ct {
                true
            } else if let (Ok(win_pos), Ok(scale)) =
                (window.outer_position(), window.scale_factor())
            {
                let mut pt = POINT::default();
                let ok = unsafe { GetCursorPos(&mut pt).is_ok() };
                if ok {
                    cursor_known = true;
                    let local_x = (pt.x - win_pos.x) as f64 / scale.max(1.0);
                    let local_y = (pt.y - win_pos.y) as f64 / scale.max(1.0);
                    let margin = 6.0;
                    let inside = local_x >= hitbox.x - margin
                        && local_x <= hitbox.x + hitbox.width + margin
                        && local_y >= hitbox.y - margin
                        && local_y <= hitbox.y + hitbox.height + margin;
                    !inside
                } else {
                    false
                }
            } else {
                false
            };

            if last_ignoring != Some(should_ignore)
                && window.set_ignore_cursor_events(should_ignore).is_ok()
            {
                last_ignoring = Some(should_ignore);
            }

            // Detect fresh clicks strictly outside the Expanded pill so clicking anywhere on
            // another window or desktop collapses the island immediately (unless pinned).
            let mouse_down = is_mouse_button_down();
            if cursor_known && should_ignore && mouse_down && !prev_mouse_down {
                if let Some(app_state) = app.try_state::<AppState>() {
                    let expanded = app_state
                        .island_expanded
                        .lock()
                        .map(|g| *g)
                        .unwrap_or(false);
                    let pinned = app_state.island_pinned.lock().map(|g| *g).unwrap_or(false);
                    let last_exp = app_state.last_expanded_ms.lock().map(|g| *g).unwrap_or(0);
                    let dismiss_enabled = app_state
                        .config
                        .lock()
                        .map(|c| c.appearance.click_outside_dismiss)
                        .unwrap_or(true);
                    if expanded
                        && !pinned
                        && dismiss_enabled
                        && now_ms().saturating_sub(last_exp) > 350
                    {
                        let _ = app.emit("pillvyn://dismiss-expanded", "outside_click");
                    }
                }
            }
            prev_mouse_down = mouse_down;

            std::thread::sleep(std::time::Duration::from_millis(38));
        }
    });
}
