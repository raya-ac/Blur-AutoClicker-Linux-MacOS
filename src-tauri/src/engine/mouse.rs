use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::rng::SmallRng;
use super::sleep_interruptible;

// --- Windows ---
#[cfg(target_os = "windows")]
mod platform {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
        MOUSEINPUT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
    };

    pub fn current_cursor_position() -> Option<(i32, i32)> {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut point) };
        if ok == 0 {
            None
        } else {
            Some((point.x, point.y))
        }
    }

    pub fn current_screen_size() -> Option<(i32, i32)> {
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if width <= 0 || height <= 0 {
            return None;
        }
        use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
        let dpi = unsafe { GetDpiForSystem() };
        let scale = dpi as f64 / 96.0;
        Some((
            (width as f64 / scale) as i32,
            (height as f64 / scale) as i32,
        ))
    }

    pub fn move_mouse(x: i32, y: i32) {
        unsafe { SetCursorPos(x, y) };
    }

    #[inline]
    fn make_input(flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn send_mouse_event(flags: u32) {
        let input = make_input(flags);
        unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    }

    pub fn send_batch(down: u32, up: u32, n: usize) {
        let mut inputs: Vec<INPUT> = Vec::with_capacity(n * 2);
        for _ in 0..n {
            inputs.push(make_input(down));
            inputs.push(make_input(up));
        }
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
    }

    pub fn get_button_flags(button: i32) -> (u32, u32) {
        match button {
            2 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            3 => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
            _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        }
    }
}

// --- macOS (CoreGraphics) ---
#[cfg(target_os = "macos")]
mod platform {
    use core_graphics::display::CGDisplay;
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    pub fn current_cursor_position() -> Option<(i32, i32)> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
        let event = CGEvent::new(source).ok()?;
        let loc = event.location();
        Some((loc.x as i32, loc.y as i32))
    }

    pub fn current_screen_size() -> Option<(i32, i32)> {
        let display = CGDisplay::main();
        let w = display.pixels_wide();
        let h = display.pixels_high();
        if w == 0 || h == 0 {
            return None;
        }
        let mode = display.display_mode()?;
        Some((mode.width() as i32, mode.height() as i32))
    }

    pub fn move_mouse(x: i32, y: i32) {
        let point = CGPoint::new(x as f64, y as f64);
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            if let Ok(event) = CGEvent::new_mouse_event(
                source,
                CGEventType::MouseMoved,
                point,
                CGMouseButton::Left,
            ) {
                event.post(CGEventTapLocation::HID);
            }
        }
    }

    fn cg_button(btn_id: u32) -> CGMouseButton {
        match btn_id {
            2 => CGMouseButton::Right,
            3 => CGMouseButton::Center,
            _ => CGMouseButton::Left,
        }
    }

    fn button_event_types(btn_id: u32) -> (CGEventType, CGEventType) {
        match btn_id {
            2 => (CGEventType::RightMouseDown, CGEventType::RightMouseUp),
            3 => (CGEventType::OtherMouseDown, CGEventType::OtherMouseUp),
            _ => (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp),
        }
    }

    // low byte = button (1/2/3), bit 8 = down/up
    pub fn send_mouse_event(flags: u32) {
        let btn_id = flags & 0xFF;
        let is_down = (flags >> 8) & 1 == 1;
        let cg_btn = cg_button(btn_id);
        let (down_type, up_type) = button_event_types(btn_id);
        let event_type = if is_down { down_type } else { up_type };

        let pos = super::current_cursor_position().unwrap_or((0, 0));
        let point = CGPoint::new(pos.0 as f64, pos.1 as f64);
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            if let Ok(event) = CGEvent::new_mouse_event(source, event_type, point, cg_btn) {
                event.post(CGEventTapLocation::HID);
            }
        }
    }

    pub fn send_batch(down: u32, up: u32, n: usize) {
        for _ in 0..n {
            send_mouse_event(down);
            send_mouse_event(up);
        }
    }

    pub fn get_button_flags(button: i32) -> (u32, u32) {
        let btn = match button {
            2 => 2u32,
            3 => 3u32,
            _ => 1u32,
        };
        (btn | 0x100, btn) // high bit = press
    }
}

// --- Linux (X11 + XTest) ---
#[cfg(target_os = "linux")]
mod platform {
    use x11::xlib;
    use x11::xtest;
    use std::ptr;

    fn with_display<T>(f: impl FnOnce(*mut xlib::Display) -> T) -> Option<T> {
        let display = unsafe { xlib::XOpenDisplay(ptr::null()) };
        if display.is_null() {
            return None;
        }
        let result = f(display);
        unsafe { xlib::XCloseDisplay(display) };
        Some(result)
    }

    pub fn current_cursor_position() -> Option<(i32, i32)> {
        with_display(|display| {
            let root = unsafe { xlib::XDefaultRootWindow(display) };
            let (mut root_ret, mut child_ret) = (0, 0);
            let (mut rx, mut ry, mut wx, mut wy) = (0, 0, 0, 0);
            let mut mask = 0;
            unsafe {
                xlib::XQueryPointer(
                    display, root, &mut root_ret, &mut child_ret,
                    &mut rx, &mut ry, &mut wx, &mut wy, &mut mask,
                );
            }
            (rx, ry)
        })
    }

    pub fn current_screen_size() -> Option<(i32, i32)> {
        with_display(|display| {
            let screen = unsafe { xlib::XDefaultScreen(display) };
            let w = unsafe { xlib::XDisplayWidth(display, screen) };
            let h = unsafe { xlib::XDisplayHeight(display, screen) };
            (w, h)
        })
    }

    pub fn move_mouse(x: i32, y: i32) {
        with_display(|display| {
            let root = unsafe { xlib::XDefaultRootWindow(display) };
            unsafe {
                xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, x, y);
                xlib::XFlush(display);
            }
        });
    }

    fn x11_button(button: i32) -> u32 {
        match button {
            2 => 3, // X11: 3 = right click
            3 => 2, // X11: 2 = middle click
            _ => 1,
        }
    }

    // low byte = X11 button, bit 8 = press/release
    pub fn send_mouse_event(flags: u32) {
        let x11_btn = (flags & 0xFF) as u32;
        let is_press = (flags >> 8) & 1 == 1;
        with_display(|display| {
            unsafe {
                xtest::XTestFakeButtonEvent(display, x11_btn, if is_press { 1 } else { 0 }, 0);
                xlib::XFlush(display);
            }
        });
    }

    pub fn send_batch(down: u32, up: u32, n: usize) {
        with_display(|display| {
            let btn_down = (down & 0xFF) as u32;
            for _ in 0..n {
                unsafe {
                    xtest::XTestFakeButtonEvent(display, btn_down, 1, 0);
                    xtest::XTestFakeButtonEvent(display, btn_down, 0, 0);
                }
            }
            unsafe { xlib::XFlush(display) };
        });
    }

    pub fn get_button_flags(button: i32) -> (u32, u32) {
        let btn = x11_button(button);
        (btn | 0x100, btn)
    }
}

// --- public API (delegates to platform) ---

pub fn current_cursor_position() -> Option<(i32, i32)> {
    platform::current_cursor_position()
}

pub fn current_screen_size() -> Option<(i32, i32)> {
    platform::current_screen_size()
}

#[inline]
pub fn get_cursor_pos() -> (i32, i32) {
    current_cursor_position().unwrap_or((0, 0))
}

#[inline]
pub fn move_mouse(x: i32, y: i32) {
    platform::move_mouse(x, y);
}

#[inline]
pub fn send_mouse_event(flags: u32) {
    platform::send_mouse_event(flags);
}

pub fn send_batch(down: u32, up: u32, n: usize, _hold_ms: u32) {
    platform::send_batch(down, up, n);
}

pub fn send_clicks(
    down: u32,
    up: u32,
    count: usize,
    hold_ms: u32,
    use_double_click_gap: bool,
    double_click_delay_ms: u32,
    running: &Arc<AtomicBool>,
) {
    if count == 0 {
        return;
    }

    if !use_double_click_gap && count > 1 && hold_ms == 0 {
        send_batch(down, up, count, hold_ms);
        return;
    }

    for index in 0..count {
        if !running.load(Ordering::SeqCst) {
            return;
        }

        send_mouse_event(down);
        if hold_ms > 0 {
            sleep_interruptible(Duration::from_millis(hold_ms as u64), running);
        }
        send_mouse_event(up);

        if index + 1 < count && use_double_click_gap && double_click_delay_ms > 0 {
            sleep_interruptible(Duration::from_millis(double_click_delay_ms as u64), running);
        }
    }
}

#[inline]
pub fn get_button_flags(button: i32) -> (u32, u32) {
    platform::get_button_flags(button)
}

#[inline]
pub fn ease_in_out_quad(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[inline]
pub fn cubic_bezier(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

pub fn smooth_move(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: u64,
    rng: &mut SmallRng,
) {
    if duration_ms < 5 {
        move_mouse(end_x, end_y);
        return;
    }

    let (sx, sy) = (start_x as f64, start_y as f64);
    let (ex, ey) = (end_x as f64, end_y as f64);
    let (dx, dy) = (ex - sx, ey - sy);
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 1.0 {
        return;
    }

    let (perp_x, perp_y) = (-dy / distance, dx / distance);
    let sign = |b: bool| if b { 1.0f64 } else { -1.0 };
    let o1 = (rng.next_f64() * 0.3 + 0.15) * distance * sign(rng.next_f64() >= 0.5);
    let o2 = (rng.next_f64() * 0.3 + 0.15) * distance * sign(rng.next_f64() >= 0.5);
    let cp1x = sx + dx * 0.33 + perp_x * o1;
    let cp1y = sy + dy * 0.33 + perp_y * o1;
    let cp2x = sx + dx * 0.66 + perp_x * o2;
    let cp2y = sy + dy * 0.66 + perp_y * o2;

    let steps = (duration_ms as usize).clamp(10, 200);
    let step_dur = Duration::from_millis(duration_ms / steps as u64);

    for i in 0..=steps {
        let t = ease_in_out_quad(i as f64 / steps as f64);
        move_mouse(
            cubic_bezier(t, sx, cp1x, cp2x, ex) as i32,
            cubic_bezier(t, sy, cp1y, cp2y, ey) as i32,
        );
        if i < steps {
            std::thread::sleep(step_dur);
        }
    }
}
