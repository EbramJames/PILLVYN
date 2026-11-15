use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::Notify;

#[cfg(target_os = "windows")]
static HOOK_NOTIFY: OnceLock<Arc<Notify>> = OnceLock::new();
#[cfg(target_os = "windows")]
static EXTERNAL_FOREGROUND_EVENT: AtomicBool = AtomicBool::new(false);

/// Starts a lightweight Win32 `SetWinEventHook` listener thread that signals `notify`
/// immediately on foreground window changes or minimize-end transitions.
pub fn spawn_foreground_hook(notify: Arc<Notify>) {
    #[cfg(target_os = "windows")]
    {
        let _ = HOOK_NOTIFY.set(notify);
        std::thread::Builder::new()
            .name("pillvyn-win-events".into())
            .spawn(|| unsafe {
                use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent};
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetMessageW, MSG, WINEVENT_OUTOFCONTEXT,
                };

                const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
                const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;

                let hook_fg = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(win_event_callback),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let hook_restore = SetWinEventHook(
                    EVENT_SYSTEM_MINIMIZEEND,
                    EVENT_SYSTEM_MINIMIZEEND,
                    None,
                    Some(win_event_callback),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {}

                if !hook_fg.is_invalid() {
                    let _ = UnhookWinEvent(hook_fg);
                }
                if !hook_restore.is_invalid() {
                    let _ = UnhookWinEvent(hook_restore);
                }
            })
            .ok();
    }

    #[cfg(not(target_os = "windows"))]
    let _ = notify;
}

/// Returns and clears whether another process just took Windows foreground focus.
pub fn take_external_foreground_flag() -> bool {
    #[cfg(target_os = "windows")]
    {
        EXTERNAL_FOREGROUND_EVENT.swap(false, Ordering::SeqCst)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn win_event_callback(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    hwnd: windows::Win32::Foundation::HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    if hwnd.0.is_null() || id_object != 0 {
        return;
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != 0 && pid != std::process::id() {
        EXTERNAL_FOREGROUND_EVENT.store(true, Ordering::SeqCst);
    }

    if let Some(notify) = HOOK_NOTIFY.get() {
        notify.notify_one();
    }
}

/// Queries the Windows shell notification state to detect exclusive Direct3D fullscreen,
/// busy fullscreen, or presentation mode accurately alongside window rect geometry.
pub fn is_fullscreen_notification_state() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
            QUNS_RUNNING_D3D_FULL_SCREEN,
        };
        if let Ok(state) = SHQueryUserNotificationState() {
            return state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_BUSY
                || state == QUNS_PRESENTATION_MODE;
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Returns true when the primary mouse button (left or right) is currently pressed.
pub fn is_mouse_button_down() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON,
        };
        let left = (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0;
        let right = (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0;
        left || right
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
