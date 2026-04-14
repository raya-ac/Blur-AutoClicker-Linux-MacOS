# Blur Auto Clicker

<div align="center">
    <img src="https://github.com/Blur009/Blur-AutoClicker/blob/main/public/V3.0.0_UI.png" width="600"/>
</div>
<p align="center"><em>An accuracy and performance focused auto clicker for Windows, macOS, and Linux</em></p>


## Why I made it

A lot of the most popular auto clickers like OP Auto Clicker and Speed Auto Clicker are pretty inaccurate at higher speeds. Setting CPS to 50 might give you 40.. or 60. Technically this is not an issue since they are still clicking _fast_, but I am a perfectionist and I wanted something that could actually click at the CPS I set it to, even at higher speeds. So I made this.

Many auto clickers also have 1 good feature but are missing the other features I want. Mine combines everything I have seen in other auto clickers, plus some of my own ideas.

Performance is a heavy focus too. The total RAM usage is around 50mb and I intend for it to never go above 100mb.

---

## Features

<div align="center">
    <img src="https://github.com/Blur009/Blur-AutoClicker/blob/main/public/30s_500cps_Speed_Test.png" width="600"/>
</div>
<p align="center"><em>Blur Auto Clicker reaching 500 CPS steadily (windows limit)</em></p>

Simple mode:
- On/off indicator (blur logo turns green when active)
- Left, right, middle mouse button
- Hold or toggle activation modes
- Customizable hotkeys

Advanced mode (everything in simple mode, plus):
- Adjustable click timing (duty cycle)
- Speed randomization (randomizes CPS within a percentage range)
- Corner and edge stop zones (stops clicking when the mouse hits a corner or screen edge, works as a failsafe)
- Click and time limits (stop after a set number of clicks or after a duration)
- Double click mode
- Position clicking (pick a screen coordinate, mouse moves there and clicks)
- Per second, minute, hour, or day intervals
- Stop zone overlay (transparent fullscreen overlay showing where the failsafe zones are)

Other:
- Click stats tracked locally (total clicks, time spent, CPU usage, sessions)

---

## Platform support

| Platform | Status | Click method | Notes |
|----------|--------|-------------|-------|
| Windows | Full support | SendInput | ~500 CPS practical max (OS timer resolution limit) |
| macOS | Full support | CoreGraphics CGEvent | Requires Accessibility permission in System Settings |
| Linux | Full support | X11 + XTest | Requires X11 (Wayland not supported yet) |

### macOS notes

The app needs Accessibility access to send clicks and read hotkey state. On first launch, go to System Settings > Privacy & Security > Accessibility and enable BlurAutoClicker. You'll need to re-toggle this permission after each rebuild during development since macOS ties it to the binary signature.

### Linux notes

You need X11 with the XTest extension. On Debian/Ubuntu, install the dependencies:

```
sudo apt install libx11-dev libxtst-dev
```

---

## Installation

### Windows
1. Download from releases
2. Run the installer
3. Default location: `%localappdata%/BlurAutoClicker/BlurAutoClicker.exe`

Config and stats are stored in `%appdata%/BlurAutoClicker`

### macOS
1. Download the `.app` from releases
2. Drag to Applications
3. Open, then grant Accessibility permission when prompted

Config and stats are stored in `~/Library/Application Support/BlurAutoClicker`

### Linux
1. Download the `.deb` or `.AppImage` from releases
2. Install and run

Config and stats are stored in `~/.local/share/BlurAutoClicker` (or `$XDG_DATA_HOME/BlurAutoClicker`)

---

## Building from source

Requires Node.js and Rust.

```bash
npm install
npx tauri build
```

The built app ends up in `src-tauri/target/release/bundle/`.

### Platform dependencies

Windows: no extra deps, everything comes through cargo.

macOS: Xcode command line tools. The build uses `core-graphics` and `core-foundation` crates.

Linux:
```bash
sudo apt install libx11-dev libxtst-dev libwebkit2gtk-4.1-dev libappindicator3-dev
```

---

## Technical details

The clicking engine runs on a dedicated thread separate from the UI. Timing uses a tight poll loop with platform specific timer resolution (1ms on Windows via `NtSetTimerResolution`, native thread scheduling on macOS/Linux). CPU usage is tracked per-thread using `QueryThreadCycleTime` on Windows and `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` on macOS/Linux.

The failsafe system (corner stop, edge stop) polls cursor position each cycle and compares against the configured zones. The overlay window shows these zones as colored rectangles on a transparent fullscreen layer.

Hotkey detection uses platform native APIs: `GetAsyncKeyState` on Windows, `CGEventSourceKeyState`/`CGEventSourceFlagsState` on macOS, and `XQueryKeymap`/`XQueryPointer` on Linux. This runs on its own thread polling every 12ms.

---

## License

GPL v3 https://www.gnu.org/licenses/gpl-3.0.en.html#license-text

## Support the project
Ko-fi: https://ko-fi.com/blur009

You can also star the repo and share it around. Thank you for the support!
