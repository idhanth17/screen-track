//! Windows foreground-app + idle capture via Win32.

use super::Snapshot;
use crate::model::Foreground;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, FALSE, MAX_PATH};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub fn snapshot() -> Snapshot {
    Snapshot {
        foreground: foreground(),
        idle_ms: idle_ms(),
    }
}

fn foreground() -> Option<Foreground> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid).ok()?;

        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        res.ok()?;

        let full = String::from_utf16_lossy(&buf[..size as usize]);
        let process = std::path::Path::new(&full)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(full);

        // Privacy-first: window title is intentionally not captured in the MVP.
        Some(Foreground { process, title: None })
    }
}

fn idle_ms() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii).as_bool() {
            let now = GetTickCount();
            now.wrapping_sub(lii.dwTime) as u64
        } else {
            0
        }
    }
}
