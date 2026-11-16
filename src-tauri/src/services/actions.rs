use crate::config::QuickActionItem;
use std::process::Command;

const ALLOWED_SYSTEM_TOOLS: &[&str] = &[
    "wt",
    "wt.exe",
    "cmd",
    "cmd.exe",
    "powershell",
    "powershell.exe",
    "pwsh",
    "pwsh.exe",
    "taskmgr",
    "taskmgr.exe",
    "calc",
    "calc.exe",
    "notepad",
    "notepad.exe",
    "explorer",
    "explorer.exe",
    "control",
    "control.exe",
    "mspaint",
    "mspaint.exe",
    "snippingtool",
    "snippingtool.exe",
    "devmgmt.msc",
    "services.msc",
    "perfmon.exe",
    "resmon.exe",
];

const ALLOWED_URI_PREFIXES: &[&str] = &[
    "ms-settings:",
    "ms-screenclip:",
    "ms-availablenetworks:",
    "ms-actioncenter:",
    "ms-clock:",
    "https://",
    "http://",
];

pub fn validate_quick_action(kind: &str, target: &str) -> Result<String, String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("Quick action target cannot be empty".into());
    }

    match kind.trim() {
        "system_tool" => {
            let lower = trimmed.to_ascii_lowercase();
            if !ALLOWED_SYSTEM_TOOLS.contains(&lower.as_str()) {
                return Err(format!(
                    "Unsupported system tool '{trimmed}'. Allowed examples: wt.exe, taskmgr.exe, calc.exe, notepad.exe, explorer.exe"
                ));
            }
            Ok(trimmed.to_string())
        }
        "uri" | "url" => {
            let lower = trimmed.to_ascii_lowercase();
            if !ALLOWED_URI_PREFIXES.iter().any(|p| lower.starts_with(p)) {
                return Err(format!(
                    "Unsupported URI '{trimmed}'. Use https://, http://, or ms-settings:..."
                ));
            }
            Ok(trimmed.to_string())
        }
        "app" => {
            if trimmed.contains('&')
                || trimmed.contains('|')
                || trimmed.contains(';')
                || trimmed.contains('`')
            {
                return Err("App path contains disallowed shell characters".into());
            }
            let path = std::path::Path::new(trimmed);
            if !path.is_file() {
                return Err(format!("Executable file not found: {trimmed}"));
            }
            Ok(trimmed.to_string())
        }
        other => Err(format!("Unknown quick action type: {other}")),
    }
}

pub fn execute_quick_action(item: &QuickActionItem) -> Result<(), String> {
    let target = validate_quick_action(&item.kind, &item.target)?;
    match item.kind.as_str() {
        "system_tool" => spawn_detached_exe(&target),
        "uri" | "url" => open_shell_target(&target),
        "app" => spawn_detached_exe(&target),
        other => Err(format!("Unknown quick action kind: {other}")),
    }
}

pub fn open_external_url(url: &str) -> Result<(), String> {
    let validated = validate_quick_action("uri", url)?;
    open_shell_target(&validated)
}

#[cfg(target_os = "windows")]
fn spawn_detached_exe(exe: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const DETACHED_PROCESS: u32 = 0x00000008;
    Command::new(exe)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to launch '{exe}': {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_detached_exe(_exe: &str) -> Result<(), String> {
    Err("Launching Windows tools requires Windows".into())
}

#[cfg(target_os = "windows")]
fn open_shell_target(target: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide_target = HSTRING::from(target);
    let res = unsafe { ShellExecuteW(None, w!("open"), &wide_target, None, None, SW_SHOWNORMAL) };
    if (res.0 as isize) <= 32 {
        return Err(format!("Windows shell could not open '{target}'"));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn open_shell_target(_target: &str) -> Result<(), String> {
    Err("Shell execution requires Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_action_blocks_dangerous_schemes_and_tools() {
        let bad_tool = QuickActionItem {
            id: "bad".into(),
            label: "Bad".into(),
            icon: "x".into(),
            kind: "system_tool".into(),
            target: "powershell -enc abc".into(),
        };
        assert!(execute_quick_action(&bad_tool).is_err());

        let bad_uri = QuickActionItem {
            id: "bad2".into(),
            label: "Bad2".into(),
            icon: "x".into(),
            kind: "uri".into(),
            target: "javascript:alert(1)".into(),
        };
        assert!(execute_quick_action(&bad_uri).is_err());

        assert_eq!(
            validate_quick_action("uri", "  ms-availablenetworks:  ").unwrap(),
            "ms-availablenetworks:"
        );
        assert!(open_external_url("javascript:alert(1)").is_err());
        assert!(open_external_url("").is_err());
    }
}
