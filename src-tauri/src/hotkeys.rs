use crate::AppHandle;
use crate::ClickerState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;

use crate::engine::worker::now_epoch_ms;
use crate::engine::worker::start_clicker_inner;
use crate::engine::worker::stop_clicker_inner;
use crate::engine::worker::toggle_clicker_inner;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub main_vk: i32,
    pub key_token: String,
}

pub fn register_hotkey_inner(app: &AppHandle, hotkey: String) -> Result<String, String> {
    let binding = parse_hotkey_binding(&hotkey)?;
    let state = app.state::<ClickerState>();
    state
        .suppress_hotkey_until_ms
        .store(now_epoch_ms().saturating_add(250), Ordering::SeqCst);
    state
        .suppress_hotkey_until_release
        .store(true, Ordering::SeqCst);
    *state.registered_hotkey.lock().unwrap() = Some(binding.clone());

    Ok(format_hotkey_binding(&binding))
}

pub fn normalize_hotkey(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace("control", "ctrl")
        .replace("command", "super")
        .replace("meta", "super")
        .replace("win", "super")
}

pub fn parse_hotkey_binding(hotkey: &str) -> Result<HotkeyBinding, String> {
    let normalized = normalize_hotkey(hotkey);
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_key = false;
    let mut main_key: Option<(i32, String)> = None;

    for token in normalized.split('+').map(str::trim) {
        if token.is_empty() {
            return Err(format!("Invalid hotkey '{hotkey}': found empty key token"));
        }

        match token {
            "alt" | "option" => alt = true,
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "super" | "command" | "cmd" | "meta" | "win" => super_key = true,
            _ => {
                if main_key
                    .replace(parse_hotkey_main_key(token, hotkey)?)
                    .is_some()
                {
                    return Err(format!(
                        "Invalid hotkey '{hotkey}': use modifiers first and only one main key"
                    ));
                }
            }
        }
    }

    let (main_vk, key_token) =
        main_key.ok_or_else(|| format!("Invalid hotkey '{hotkey}': missing main key"))?;

    Ok(HotkeyBinding {
        ctrl,
        alt,
        shift,
        super_key,
        main_vk,
        key_token,
    })
}

// We use a unified virtual key code scheme across platforms.
// On Windows these map directly to VK_ constants.
// On macOS/Linux we use the same integer values — the platform-specific
// `is_vk_down` translates them to the native key check.
pub fn parse_hotkey_main_key(token: &str, original_hotkey: &str) -> Result<(i32, String), String> {
    let lower = token.trim().to_lowercase();

    // These constants match the Windows VK_ values, which we use as a
    // cross-platform keycode vocabulary. The is_vk_down() function on
    // each platform knows how to map them.
    const VK_SPACE: i32 = 0x20;
    const VK_TAB: i32 = 0x09;
    const VK_RETURN: i32 = 0x0D;
    const VK_BACK: i32 = 0x08;
    const VK_DELETE: i32 = 0x2E;
    const VK_INSERT: i32 = 0x2D;
    const VK_HOME: i32 = 0x24;
    const VK_END: i32 = 0x23;
    const VK_PRIOR: i32 = 0x21;  // page up
    const VK_NEXT: i32 = 0x22;   // page down
    const VK_UP: i32 = 0x26;
    const VK_DOWN: i32 = 0x28;
    const VK_LEFT: i32 = 0x25;
    const VK_RIGHT: i32 = 0x27;
    const VK_ESCAPE: i32 = 0x1B;
    const VK_F1: i32 = 0x70;
    const VK_OEM_102: i32 = 0xE2;
    const VK_OEM_1: i32 = 0xBA;    // ;
    const VK_OEM_2: i32 = 0xBF;    // /
    const VK_OEM_3: i32 = 0xC0;    // `
    const VK_OEM_4: i32 = 0xDB;    // [
    const VK_OEM_5: i32 = 0xDC;    // backslash
    const VK_OEM_6: i32 = 0xDD;    // ]
    const VK_OEM_7: i32 = 0xDE;    // '
    const VK_OEM_MINUS: i32 = 0xBD;
    const VK_OEM_PLUS: i32 = 0xBB; // = key
    const VK_OEM_COMMA: i32 = 0xBC;
    const VK_OEM_PERIOD: i32 = 0xBE;

    let mapped = match lower.as_str() {
        "<" | ">" | "intlbackslash" | "oem102" | "nonusbackslash" => {
            Some((VK_OEM_102, String::from("IntlBackslash")))
        }
        "space" | "spacebar" => Some((VK_SPACE, String::from("space"))),
        "tab" => Some((VK_TAB, String::from("tab"))),
        "enter" => Some((VK_RETURN, String::from("enter"))),
        "backspace" => Some((VK_BACK, String::from("backspace"))),
        "delete" => Some((VK_DELETE, String::from("delete"))),
        "insert" => Some((VK_INSERT, String::from("insert"))),
        "home" => Some((VK_HOME, String::from("home"))),
        "end" => Some((VK_END, String::from("end"))),
        "pageup" => Some((VK_PRIOR, String::from("pageup"))),
        "pagedown" => Some((VK_NEXT, String::from("pagedown"))),
        "up" => Some((VK_UP, String::from("up"))),
        "down" => Some((VK_DOWN, String::from("down"))),
        "left" => Some((VK_LEFT, String::from("left"))),
        "right" => Some((VK_RIGHT, String::from("right"))),
        "esc" | "escape" => Some((VK_ESCAPE, String::from("escape"))),
        "/" | "slash" => Some((VK_OEM_2, String::from("/"))),
        "\\" | "backslash" => Some((VK_OEM_5, String::from("\\"))),
        ";" | "semicolon" => Some((VK_OEM_1, String::from(";"))),
        "'" | "quote" => Some((VK_OEM_7, String::from("'"))),
        "[" | "bracketleft" => Some((VK_OEM_4, String::from("["))),
        "]" | "bracketright" => Some((VK_OEM_6, String::from("]"))),
        "-" | "minus" => Some((VK_OEM_MINUS, String::from("-"))),
        "=" | "equal" => Some((VK_OEM_PLUS, String::from("="))),
        "`" | "backquote" => Some((VK_OEM_3, String::from("`"))),
        "," | "comma" => Some((VK_OEM_COMMA, String::from(","))),
        "." | "period" => Some((VK_OEM_PERIOD, String::from("."))),
        _ => None,
    };

    if let Some(binding) = mapped {
        return Ok(binding);
    }

    if lower.starts_with('f') && lower.len() <= 3 {
        if let Ok(number) = lower[1..].parse::<i32>() {
            let vk = match number {
                1..=24 => VK_F1 + (number - 1),
                _ => -1,
            };
            if vk >= 0 {
                return Ok((vk, lower));
            }
        }
    }

    if let Some(letter) = lower.strip_prefix("key") {
        if letter.len() == 1 {
            return parse_hotkey_main_key(letter, original_hotkey);
        }
    }

    if let Some(digit) = lower.strip_prefix("digit") {
        if digit.len() == 1 {
            return parse_hotkey_main_key(digit, original_hotkey);
        }
    }

    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch.to_ascii_uppercase() as i32, lower));
        }
        if ch.is_ascii_digit() {
            return Ok((ch as i32, lower));
        }
    }

    Err(format!(
        "Couldn't recognize '{token}' as a valid key in '{original_hotkey}'"
    ))
}

pub fn format_hotkey_binding(binding: &HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();

    if binding.ctrl {
        parts.push(String::from("ctrl"));
    }
    if binding.alt {
        parts.push(String::from("alt"));
    }
    if binding.shift {
        parts.push(String::from("shift"));
    }
    if binding.super_key {
        parts.push(String::from("super"));
    }

    parts.push(binding.key_token.clone());
    parts.join("+")
}

pub fn start_hotkey_listener(app: AppHandle) {
    std::thread::spawn(move || {
        let mut was_pressed = false;

        loop {
            let binding = {
                let state = app.state::<ClickerState>();
                let val = state.registered_hotkey.lock().unwrap().clone();
                val
            };

            let currently_pressed = binding
                .as_ref()
                .map(is_hotkey_binding_pressed)
                .unwrap_or(false);

            let suppress_until = app
                .state::<ClickerState>()
                .suppress_hotkey_until_ms
                .load(Ordering::SeqCst);
            let suppress_until_release = app
                .state::<ClickerState>()
                .suppress_hotkey_until_release
                .load(Ordering::SeqCst);
            let hotkey_capture_active = app
                .state::<ClickerState>()
                .hotkey_capture_active
                .load(Ordering::SeqCst);

            if hotkey_capture_active {
                was_pressed = currently_pressed;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if suppress_until_release {
                if currently_pressed {
                    was_pressed = true;
                    std::thread::sleep(Duration::from_millis(12));
                    continue;
                }

                app.state::<ClickerState>()
                    .suppress_hotkey_until_release
                    .store(false, Ordering::SeqCst);
                was_pressed = false;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if now_epoch_ms() < suppress_until {
                was_pressed = currently_pressed;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if currently_pressed && !was_pressed {
                handle_hotkey_pressed(&app);
            } else if !currently_pressed && was_pressed {
                handle_hotkey_released(&app);
            }

            was_pressed = currently_pressed;
            std::thread::sleep(Duration::from_millis(12));
        }
    });
}

pub fn handle_hotkey_pressed(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let m = state.settings.lock().unwrap().mode.clone();
        m
    };

    if mode == "Toggle" {
        let _ = toggle_clicker_inner(app);
    } else if mode == "Hold" {
        let _ = start_clicker_inner(app);
    }
}

pub fn handle_hotkey_released(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let m = state.settings.lock().unwrap().mode.clone();
        m
    };

    if mode == "Hold" {
        let _ = stop_clicker_inner(app, Some(String::from("Stopped from hold hotkey")));
    }
}

// =============================================================================
// Platform-specific key state polling
// =============================================================================

#[cfg(target_os = "windows")]
pub fn is_vk_down(vk: i32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(target_os = "macos")]
pub fn is_vk_down(vk: i32) -> bool {
    // Use raw CoreGraphics FFI for key state and modifier flags.
    // The core-graphics crate doesn't expose CGEventSourceFlagsState
    // or CGEventSourceKeyState, so we link them directly.
    extern "C" {
        fn CGEventSourceFlagsState(stateID: i32) -> u64;
        fn CGEventSourceKeyState(stateID: i32, key: u16) -> bool;
    }

    const CG_EVENT_SOURCE_STATE_COMBINED: i32 = 0;

    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_SHIFT: i32 = 0x10;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;

    // Modifier flags from CGEventFlags
    const K_CG_EVENT_FLAG_CONTROL: u64 = 0x00040000;
    const K_CG_EVENT_FLAG_ALTERNATE: u64 = 0x00080000;
    const K_CG_EVENT_FLAG_SHIFT: u64 = 0x00020000;
    const K_CG_EVENT_FLAG_COMMAND: u64 = 0x00100000;

    // Modifier keys — check via CGEventSourceFlagsState
    match vk {
        VK_CONTROL => {
            let flags = unsafe { CGEventSourceFlagsState(CG_EVENT_SOURCE_STATE_COMBINED) };
            return flags & K_CG_EVENT_FLAG_CONTROL != 0;
        }
        VK_MENU => {
            let flags = unsafe { CGEventSourceFlagsState(CG_EVENT_SOURCE_STATE_COMBINED) };
            return flags & K_CG_EVENT_FLAG_ALTERNATE != 0;
        }
        VK_SHIFT => {
            let flags = unsafe { CGEventSourceFlagsState(CG_EVENT_SOURCE_STATE_COMBINED) };
            return flags & K_CG_EVENT_FLAG_SHIFT != 0;
        }
        VK_LWIN | VK_RWIN => {
            let flags = unsafe { CGEventSourceFlagsState(CG_EVENT_SOURCE_STATE_COMBINED) };
            return flags & K_CG_EVENT_FLAG_COMMAND != 0;
        }
        _ => {}
    }

    // Regular keys — translate to macOS keycode, check via CGEventSourceKeyState
    if let Some(mac_keycode) = vk_to_macos_keycode(vk) {
        return unsafe { CGEventSourceKeyState(CG_EVENT_SOURCE_STATE_COMBINED, mac_keycode) };
    }

    false
}

#[cfg(target_os = "macos")]
fn vk_to_macos_keycode(vk: i32) -> Option<u16> {
    // Map our VK codes (Windows-style values) to macOS CGKeyCode values
    let code: u16 = match vk {
        0x41..=0x5A => {
            // A-Z
            let letter = (vk - 0x41) as u8;
            match letter {
                0 => 0x00,   // A
                1 => 0x0B,   // B
                2 => 0x08,   // C
                3 => 0x02,   // D
                4 => 0x0E,   // E
                5 => 0x03,   // F
                6 => 0x05,   // G
                7 => 0x04,   // H
                8 => 0x22,   // I
                9 => 0x26,   // J
                10 => 0x28,  // K
                11 => 0x25,  // L
                12 => 0x2E,  // M
                13 => 0x2D,  // N
                14 => 0x1F,  // O
                15 => 0x23,  // P
                16 => 0x0C,  // Q
                17 => 0x0F,  // R
                18 => 0x01,  // S
                19 => 0x11,  // T
                20 => 0x20,  // U
                21 => 0x09,  // V
                22 => 0x0D,  // W
                23 => 0x07,  // X
                24 => 0x10,  // Y
                25 => 0x06,  // Z
                _ => return None,
            }
        }
        0x30..=0x39 => {
            // 0-9
            match vk {
                0x30 => 0x1D, // 0
                0x31 => 0x12, // 1
                0x32 => 0x13, // 2
                0x33 => 0x14, // 3
                0x34 => 0x15, // 4
                0x35 => 0x17, // 5
                0x36 => 0x16, // 6
                0x37 => 0x1A, // 7
                0x38 => 0x1C, // 8
                0x39 => 0x19, // 9
                _ => return None,
            }
        }
        0x70..=0x87 => {
            // F1-F24
            let f_num = vk - 0x70;
            match f_num {
                0 => 0x7A,   // F1
                1 => 0x78,   // F2
                2 => 0x63,   // F3
                3 => 0x76,   // F4
                4 => 0x60,   // F5
                5 => 0x61,   // F6
                6 => 0x62,   // F7
                7 => 0x64,   // F8
                8 => 0x65,   // F9
                9 => 0x6D,   // F10
                10 => 0x67,  // F11
                11 => 0x6F,  // F12
                12 => 0x69,  // F13
                13 => 0x6B,  // F14
                14 => 0x71,  // F15
                _ => return None,
            }
        }
        0x20 => 0x31,  // space
        0x09 => 0x30,  // tab
        0x0D => 0x24,  // return/enter
        0x08 => 0x33,  // backspace
        0x2E => 0x75,  // delete (forward)
        0x1B => 0x35,  // escape
        0x26 => 0x7E,  // up
        0x28 => 0x7D,  // down
        0x25 => 0x7B,  // left
        0x27 => 0x7C,  // right
        0x24 => 0x73,  // home
        0x23 => 0x77,  // end
        0x21 => 0x74,  // page up
        0x22 => 0x79,  // page down
        0xBA => 0x29,  // ; (semicolon)
        0xBF => 0x2C,  // / (slash)
        0xDC => 0x2A,  // backslash
        0xDE => 0x27,  // ' (quote)
        0xDB => 0x21,  // [ (left bracket)
        0xDD => 0x1E,  // ] (right bracket)
        0xBD => 0x1B,  // - (minus)
        0xBB => 0x18,  // = (equal)
        0xC0 => 0x32,  // ` (backtick)
        0xBC => 0x2B,  // , (comma)
        0xBE => 0x2F,  // . (period)
        _ => return None,
    };
    Some(code)
}

#[cfg(target_os = "linux")]
pub fn is_vk_down(vk: i32) -> bool {
    use x11::xlib;
    use std::ptr;

    let display = unsafe { xlib::XOpenDisplay(ptr::null()) };
    if display.is_null() {
        return false;
    }

    // Modifier keys — check via XQueryPointer mask
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_SHIFT: i32 = 0x10;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;

    let is_modifier = matches!(vk, VK_CONTROL | VK_MENU | VK_SHIFT | VK_LWIN | VK_RWIN);
    if is_modifier {
        let root = unsafe { xlib::XDefaultRootWindow(display) };
        let (mut rr, mut cr) = (0, 0);
        let (mut rx, mut ry, mut wx, mut wy) = (0, 0, 0, 0);
        let mut mask: u32 = 0;
        unsafe {
            xlib::XQueryPointer(
                display, root, &mut rr, &mut cr,
                &mut rx, &mut ry, &mut wx, &mut wy, &mut mask,
            );
            xlib::XCloseDisplay(display);
        }
        return match vk {
            VK_CONTROL => mask & xlib::ControlMask != 0,
            VK_SHIFT => mask & xlib::ShiftMask != 0,
            VK_MENU => mask & xlib::Mod1Mask != 0,
            VK_LWIN | VK_RWIN => mask & xlib::Mod4Mask != 0,
            _ => false,
        };
    }

    // Regular keys — use XQueryKeymap
    let x11_keycode = vk_to_x11_keycode(display, vk);
    let result = if let Some(kc) = x11_keycode {
        let mut keymap: [u8; 32] = [0; 32];
        unsafe { xlib::XQueryKeymap(display, keymap.as_mut_ptr() as *mut i8) };
        let byte = kc as usize / 8;
        let bit = kc as usize % 8;
        byte < 32 && (keymap[byte] & (1 << bit)) != 0
    } else {
        false
    };

    unsafe { xlib::XCloseDisplay(display) };
    result
}

#[cfg(target_os = "linux")]
fn vk_to_x11_keycode(display: *mut x11::xlib::Display, vk: i32) -> Option<u32> {
    use x11::xlib;

    // Map VK code to X11 keysym, then convert to keycode via the display
    let keysym: u64 = match vk {
        0x41..=0x5A => {
            // A-Z -> XK_a..XK_z (lowercase keysyms)
            (vk - 0x41 + 0x61) as u64
        }
        0x30..=0x39 => vk as u64, // 0-9
        0x70..=0x87 => {
            // F1-F24
            (0xFFBE + (vk - 0x70)) as u64
        }
        0x20 => 0x0020,  // space
        0x09 => 0xFF09,  // tab
        0x0D => 0xFF0D,  // return
        0x08 => 0xFF08,  // backspace
        0x2E => 0xFFFF,  // delete
        0x2D => 0xFF63,  // insert
        0x1B => 0xFF1B,  // escape
        0x26 => 0xFF52,  // up
        0x28 => 0xFF54,  // down
        0x25 => 0xFF51,  // left
        0x27 => 0xFF53,  // right
        0x24 => 0xFF50,  // home
        0x23 => 0xFF57,  // end
        0x21 => 0xFF55,  // page up
        0x22 => 0xFF56,  // page down
        0xBA => 0x003B,  // semicolon
        0xBF => 0x002F,  // slash
        0xDC => 0x005C,  // backslash
        0xDE => 0x0027,  // quote
        0xDB => 0x005B,  // [
        0xDD => 0x005D,  // ]
        0xBD => 0x002D,  // minus
        0xBB => 0x003D,  // equal
        0xC0 => 0x0060,  // grave/backtick
        0xBC => 0x002C,  // comma
        0xBE => 0x002E,  // period
        _ => return None,
    };

    let keycode = unsafe { xlib::XKeysymToKeycode(display, keysym) };
    if keycode == 0 {
        None
    } else {
        Some(keycode as u32)
    }
}

pub fn is_hotkey_binding_pressed(binding: &HotkeyBinding) -> bool {
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_SHIFT: i32 = 0x10;
    const VK_LWIN: i32 = 0x5B;

    let ctrl_down = is_vk_down(VK_CONTROL);
    let alt_down = is_vk_down(VK_MENU);
    let shift_down = is_vk_down(VK_SHIFT);
    let super_down = is_vk_down(VK_LWIN);

    if ctrl_down != binding.ctrl
        || alt_down != binding.alt
        || shift_down != binding.shift
        || super_down != binding.super_key
    {
        return false;
    }

    is_vk_down(binding.main_vk)
}
