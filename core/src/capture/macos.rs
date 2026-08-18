//! macOS foreground-app + idle capture.
//!
//! - Foreground app: AppKit `NSWorkspace.frontmostApplication` → its
//!   `localizedName` (e.g. "Google Chrome"), falling back to the bundle id.
//! - Idle: CoreGraphics `CGEventSourceSecondsSinceLastEventType`, i.e. seconds
//!   since the last HID input of any kind.
//!
//! Foreground monitoring on macOS also requires the app to be granted
//! **Accessibility** permission (System Settings → Privacy & Security →
//! Accessibility); the onboarding UX must guide the user through granting it.
//!
//! NOTE: this file compiles only on macOS (`cfg(target_os = "macos")`), so it is
//! not exercised by the Windows dev build — it must be compiled and verified on
//! a Mac before the macOS release.

use super::Snapshot;
use crate::model::Foreground;

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

// Force AppKit to be linked (and thus loaded) so `NSWorkspace` resolves even in
// the headless daemon; and CoreGraphics for the idle timer.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

// kCGEventSourceStateHIDSystemState = 1; kCGAnyInputEventType = ~0.
const HID_SYSTEM_STATE: i32 = 1;
const ANY_INPUT_EVENT: u32 = 0xFFFF_FFFF;

pub fn snapshot() -> Snapshot {
    Snapshot {
        foreground: foreground(),
        idle_ms: idle_ms(),
    }
}

fn idle_ms() -> u64 {
    unsafe {
        let secs = CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT_EVENT);
        if secs.is_finite() && secs > 0.0 {
            (secs * 1000.0) as u64
        } else {
            0
        }
    }
}

fn foreground() -> Option<Foreground> {
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let name: *mut Object = msg_send![app, localizedName];
        let process = nsstring_to_string(name).or_else(|| {
            let bundle: *mut Object = msg_send![app, bundleIdentifier];
            nsstring_to_string(bundle)
        })?;
        // Privacy-first: window title is intentionally not captured (parity with Windows).
        Some(Foreground { process, title: None })
    }
}

/// Copy an `NSString*`'s UTF-8 contents into an owned `String`.
unsafe fn nsstring_to_string(ns: *mut Object) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![ns, UTF8String];
    if utf8.is_null() {
        return None;
    }
    let s = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
