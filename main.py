"""
 * Blur Auto Clicker - main.py
 * Copyright (C) 2026  [Blur009]
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * any later version.
 *
 * Made with Spite. (the emotion)
 *
"""
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (QApplication, QKeySequenceEdit, QCheckBox,
                               QComboBox, QPushButton, QSpinBox, QLabel,
                               QMainWindow, QGroupBox, QMessageBox, QTabWidget)
from PySide6.QtGui import QCursor
import os
import sys
import time
import threading
import ctypes
import requests
import atexit
from configparser import ConfigParser
from datetime import datetime
from os.path import exists
from src.Py.settings_manager import (load_settings, save_settings, reset_defaults,
                                     get_debug_mode, CONFIG_FILE, ensure_config_dir,
                                     CONFIG_DIR, is_first_launch, mark_launched)
from src.Private.Supabase import SUPABASE_URL, SUPABASE_KEY
from src.Py import rust_translation
from src.Py import update_checker
from src.Py import hotkey_manager
from src.Py import telemetry
from UI.ui_main_window import Ui_BlurAutoClicker as ui_main_window

# --- Constants ---
CURRENT_VERSION = "v2.1.2"
DEBUG_MODE = False

ctypes.windll.kernel32.SetConsoleMode(
    ctypes.windll.kernel32.GetStdHandle(-10), 7
)
ACTIVE_STYLE = "QPushButton { border: 2px solid #1aff22; border-radius: 4px; }"
INACTIVE_STYLE = "QPushButton { border: 2px solid #555; border-radius: 4px; }"

# --- DLL ---
if getattr(sys, 'frozen', False):
    base = getattr(sys, '_MEIPASS', os.path.dirname(os.path.abspath(__file__)))
else:
    base = os.path.dirname(os.path.abspath(__file__))

dll = ctypes.CDLL(os.path.join(base, "src", "Rust", "clicker_engine.dll"))
dll.get_click_count.restype = ctypes.c_int64
STATS_CB = ctypes.CFUNCTYPE(
    None, ctypes.c_int64, ctypes.c_double, ctypes.c_double)

# --- Supabase ---
_supabase_headers = {
    "apikey": SUPABASE_KEY,
    "Authorization": f"Bearer {SUPABASE_KEY}",
    "Content-Type": "application/json",
}

# -----------------------------------------------------------------------
# Helper functions and classes
# -----------------------------------------------------------------------


def log(message):
    if not DEBUG_MODE:
        return

    TIME_COLOR = "\033[96m"
    RESET = "\033[0m"

    if message.startswith("[") and "]" in message:
        split_idx = message.find("]") + 1
        print(f"{TIME_COLOR}{message[:split_idx]}{RESET}{message[split_idx:]}")
    else:
        print(message)


def current_time():
    return datetime.now().strftime('%H:%M:%S')


countdown_timer: QTimer | None = None


def start_countdown(seconds, tick_callback, finish_callback):
    global countdown_timer
    remaining = seconds

    if countdown_timer:
        countdown_timer.stop()

    def tick():
        nonlocal remaining
        if remaining > 0:
            tick_callback(remaining)
            remaining -= 1
        else:
            if countdown_timer:
                countdown_timer.stop()
            finish_callback()

    countdown_timer = QTimer()
    countdown_timer.timeout.connect(tick)
    countdown_timer.start(1000)
    tick()


# -----------------------------------------------------------------------
# UI Widgets class
# -----------------------------------------------------------------------
# fmt: off
class UIWidgets:
    def __init__(self, ui):
        f = ui.findChild

        # Top Section
        self.clickerstatus                       = f(QPushButton,    "ClickerStatusButton")
        self.key_sequence                        = f(QKeySequenceEdit, "KeySequence")
        self.activation_type_combobox            = f(QComboBox,      "ActivationTypeComboBox")
        self.click_speed_input                   = f(QSpinBox,       "ClicksSpeedInput")
        self.click_interval_combobox             = f(QComboBox,      "ClickIntervalComboBox")
        self.mouse_button_combobox               = f(QComboBox,      "MouseButtonComboBox")
        self.duty_cycle_input                    = f(QSpinBox,       "DutyCycleInput")
        self.speed_variation_checkbox            = f(QCheckBox,      "SpeedVariationCheckBox")
        self.speed_variation_input               = f(QSpinBox,       "SpeedVariationInput")
        self.tabs                                = f(QTabWidget,     "Tabs")

        # Limits
        self.time_limit_checkbox                 = f(QCheckBox,      "TimeLimitCheckBox")
        self.time_limit_input                    = f(QSpinBox,       "TimeLimitInput")
        self.time_limit_combobox                 = f(QComboBox,      "TimeComboBox")
        self.click_limit_checkbox                = f(QCheckBox,      "ClickLimitCheckBox")
        self.click_limit_input                   = f(QSpinBox,       "ClickLimitInput")

        # Position
        self.position_options_checkbox           = f(QGroupBox,      "PositionGroupBox")
        self.pos_x_input                         = f(QSpinBox,       "PosXInput")
        self.pos_y_input                         = f(QSpinBox,       "PosYInput")
        self.pick_position_button                = f(QPushButton,    "PickPositionButton")
        self.click_offset_input                  = f(QSpinBox,       "OffsetInput")
        self.click_offset_checkbox               = f(QCheckBox,      "OffsetCheckBox")
        self.click_offset_chance_input           = f(QSpinBox,       "OffsetChanceInput")
        self.click_offset_chance_input_checkbox  = f(QCheckBox,      "OffsetChanceCheckBox")
        self.click_offset_smoothing_checkbox     = f(QCheckBox,      "SmoothingCheckBox")

        # Other
        self.btn_reset                           = f(QPushButton,    "ResetSettingsButton")
        self.telemetry_checkbox                  = f(QCheckBox,      "TelemetryCheckBox")
        self.telemetry_popup                     = f(QCheckBox,      "TelemetryPopup")

        # Labels
        self.version_label                       = f(QLabel,         "VersionLabel")
        self.update_status_label                 = f(QLabel,         "UpdateStatusLabel")
        self.advanced_options_checkbox           = f(QCheckBox,      "AdvancedOptionsCheckBox")
        self.local_average_cpu_usage             = f(QLabel,         "AverageCPUUsageText")
        self.local_total_clicks                  = f(QLabel,         "TotalClicksText")
        self.local_total_time                    = f(QLabel,         "TimeSpentClickingText")
# fmt: on

# -----------------------------------------------------------------------
# Main application logic
# -----------------------------------------------------------------------


if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = QMainWindow()
    ui = ui_main_window()
    ui.setupUi(window)
    ui_widgets = UIWidgets(window)

    clicker_active = False
    show_high_speed_warn = True

    # -----------------------------------------------------------------------
    # Load settings
    # -----------------------------------------------------------------------
    config = ConfigParser(interpolation=None)

    shortcut_string = load_settings(
        ui_widgets, config, log=lambda m: log(f"[{current_time()}] {m}"))
    keybind_hotkey = shortcut_string.lower().replace("meta", "win")

    if exists("config.ini"):
        DEBUG_MODE = get_debug_mode(config)

    if not CONFIG_DIR.exists():
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)

    for attr in vars(ui_widgets).values():
        if isinstance(attr, (QCheckBox, QGroupBox)):
            attr.toggled.emit(attr.isChecked())

    def show_data_collection_popup():
        popup = QMessageBox()
        popup.setWindowTitle("Anonymous Telemetry Collection")
        popup.setText(
            "Blur Auto Clicker collects anonymous usage data to help improve the app.\n\n"
            "No personal information is collected. You can opt out at any time in settings."
        )
        popup.setIcon(QMessageBox.Icon.Information)
        popup.setStandardButtons(QMessageBox.StandardButton.Ok)
        accept_telemetry_checkbox = QCheckBox(
            "I accept the anonymous telemetry")
        popup.setCheckBox(accept_telemetry_checkbox)
        ok_button = popup.button(QMessageBox.StandardButton.Ok)
        ok_button.setEnabled(False)
        countdown = [5]
        ok_button.setText(f"OK ({countdown[0]})")
        timer = QTimer(popup)
        timer.setInterval(1000)

        def update_ok_button():
            if not accept_telemetry_checkbox.isChecked():
                countdown[0] -= 1
                if countdown[0] > 0:
                    ok_button.setText(f"OK ({countdown[0]})")
                else:
                    timer.stop()
                    ok_button.setText("OK")
                    ok_button.setEnabled(True)
            if accept_telemetry_checkbox.isChecked():
                timer.stop()
                ok_button.setText("Thank You <3")
                ok_button.setEnabled(True)

        timer.timeout.connect(update_ok_button)
        timer.start()
        popup.exec()

        if accept_telemetry_checkbox.isChecked():
            ui_widgets.telemetry_checkbox.setChecked(True)
        else:
            ui_widgets.telemetry_checkbox.setChecked(False)

    if is_first_launch(config):
        show_data_collection_popup()
        mark_launched(config)

    # -----------------------------------------------------------------------
    # Window / UI
    # -----------------------------------------------------------------------

    def toggle_window_size(advanced_visible):
        size = (430, 410) if advanced_visible else (430, 190)
        window.setMaximumSize(*size)
        window.setMinimumSize(*size)
        window.adjustSize()

    ui_widgets.advanced_options_checkbox.toggled.connect(toggle_window_size)
    toggle_window_size(ui_widgets.advanced_options_checkbox.isChecked())
    ui_widgets.advanced_options_checkbox.toggled.connect(ui.Tabs.setEnabled)
    ui.Tabs.setEnabled(ui_widgets.advanced_options_checkbox.isChecked())

    # -----------------------------------------------------------------------
    # Session tracking and summary
    # -----------------------------------------------------------------------
    # Session totals (accumulates across runs until app closes)
    def log_func(m): return log(f"[{current_time()}] {m}")
    _session = {"clicks": 0, "elapsed": 0.0, "cpu_samples": []}

    def log_session_summary():
        if _session["clicks"] == 0:
            log(f"[{current_time()}] No session Data")
            return "No Session Data"

        total_clicks = _session["clicks"]
        total_elapsed = _session["elapsed"]
        avg_cpu = sum(_session["cpu_samples"]) / \
            len(_session["cpu_samples"]) if _session["cpu_samples"] else 0.0

        log(f"[{current_time()}] ---- Session Summary ----")
        log(f"[{current_time()}] Total Clicks : {total_clicks}")
        log(f"[{current_time()}] Total Time   : {total_elapsed:.2f}s")
        log(f"[{current_time()}] Total Avg CPU: {avg_cpu:.1f}%")
        log(f"[{current_time()}] -------------------------")

    def format_time(seconds: float) -> str:
        if seconds >= 3600:
            return f"{seconds / 3600:.1f}h"
        elif seconds >= 60:
            return f"{seconds / 60:.1f}m"
        else:
            return f"{round(seconds, 2)}s"

    def update_local_statistics(clicks, elapsed, avg_cpu):
        total_clicks = config.getint("Settings", "Total_Clicks", fallback=0)
        total_time = config.getfloat("Settings", "Total_Time", fallback=0.0)
        cpu_average = config.getfloat(
            "Settings", "CPU_Average_Usage", fallback=0.0)
        total_sessions = config.getint(
            "Settings", "Total_Sessions", fallback=0)

        new_total_clicks = total_clicks + clicks
        new_total_time = total_time + elapsed
        new_sessions = total_sessions + 1
        new_cpu_avg = (cpu_average * total_sessions + avg_cpu) / new_sessions

        config.set("Settings", "Total_Clicks", str(int(new_total_clicks)))
        config.set("Settings", "Total_Time", str(round(new_total_time, 2)))
        config.set("Settings", "CPU_Average_Usage", str(round(new_cpu_avg, 2)))
        config.set("Settings", "Total_Sessions", str(int(new_sessions)))

        ui_widgets.local_total_clicks.setText(str(int(new_total_clicks)))
        ui_widgets.local_total_time.setText(format_time(new_total_time))
        ui_widgets.local_average_cpu_usage.setText(f"{round(new_cpu_avg, 2)}%")

        save_settings(ui_widgets, config, keybind_hotkey=hotkey_manager.get_keybind(),
                      debug_mode=DEBUG_MODE, log=log_func)
    # -----------------------------------------------------------------------
    # Supabase integration
    # -----------------------------------------------------------------------

    def send_stats(clicks, elapsed, avg_cpu=None):
        payload = {
            "clicks": clicks,
            "time": round(elapsed, 2),
        }
        if avg_cpu is not None:
            payload["avg_cpu"] = round(avg_cpu, 2)

        try:
            response = requests.post(
                f"{SUPABASE_URL}/rest/v1/quick_updates",
                headers=_supabase_headers,
                json=payload,
                timeout=10,
            )
            if response.ok:
                log(f"[{current_time()}] Stats sent to Supabase")
            else:
                log(f"[{current_time()}] Supabase error: {response.text}")
        except requests.RequestException as e:
            log(f"[{current_time()}] Supabase request failed: {e}")

    # -----------------------------------------------------------------------
    # Clicker logic
    # -----------------------------------------------------------------------

    def on_stop(clicks, elapsed, avg_cpu):
        _session["clicks"] += clicks
        _session["elapsed"] += elapsed
        _session["cpu_samples"].append(avg_cpu)

        log(f"[{current_time()}] Stopping Session and sending data...")

        log_session_summary()
        update_local_statistics(clicks, elapsed, avg_cpu)
        cpu = avg_cpu if avg_cpu > 0.0 else None
        threading.Thread(target=send_stats, args=(
            clicks, elapsed, cpu)).start()

    _cb_ref = STATS_CB(on_stop)
    dll.set_stats_callback(_cb_ref)

    def on_clicker_finished():
        global clicker_active
        if clicker_active:
            clicker_active = False
            ui_widgets.clickerstatus.setText("Off")
            ui_widgets.clickerstatus.setDefault(False)
            ui_widgets.btn_reset.setDefault(True)
            log(f"[{current_time()}] Clicker finished: Limit reached.")

    def advanced_mode_enabled():
        return ui_widgets.advanced_options_checkbox.isChecked()

    def raw_click_speed():
        return float(ui_widgets.click_speed_input.value())

    def click_speed_variation():
        return ui_widgets.speed_variation_input.value() if (
            advanced_mode_enabled() and ui_widgets.speed_variation_checkbox.isChecked()) else 0

    def duty_cycle():
        return ui_widgets.duty_cycle_input.value()

    def mouse_button():
        button_map = {"Left Click": "left",
                      "Right Click": "right", "Middle Click": "middle"}
        return button_map.get(
            ui_widgets.mouse_button_combobox.currentText(), "left")

    def click_limit_enabled() -> bool:
        return advanced_mode_enabled() and ui_widgets.click_limit_checkbox.isChecked()

    def click_amount_limit():
        if advanced_mode_enabled() and click_limit_enabled():
            return ui_widgets.click_limit_input.value()
        return 0

    def time_limit_enabled() -> bool:
        return advanced_mode_enabled() and ui_widgets.time_limit_checkbox.isChecked()

    def click_time_limit():
        time_multipliers = {"seconds": 1, "minutes": 60,
                            "hours": 3600, "days": 86400}
        if advanced_mode_enabled() and time_limit_enabled():
            amount = max(float(ui_widgets.time_limit_input.value()), 1)
            unit = ui_widgets.time_limit_combobox.currentText().lower()
            time_limit = amount * time_multipliers.get(unit, 1)
        else:
            time_limit = 0

        return time_limit

    def get_screen_limits(pos):
        if pos != (0, 0):
            sw = app.primaryScreen().size().width()
            sh = app.primaryScreen().size().height()
            x, y = pos
            if x < 0 or x >= sw or y < 0 or y >= sh:
                ui_widgets.pos_x_input.setValue(0)
                ui_widgets.pos_y_input.setValue(0)
                pos = (0, 0)
                log(f"[{current_time()}] Position out of bounds, reset to (0, 0)")

    def click_unit():
        unit_map = {"Second": "second", "Minute": "minute",
                    "Hour": "hour", "Day": "day"}
        return unit_map.get(ui_widgets.click_interval_combobox.currentText(), "second")

    def click_position_enabled() -> bool:
        return advanced_mode_enabled() and ui_widgets.position_options_checkbox.isChecked()

    def click_position():
        if click_position_enabled():
            pos = (ui_widgets.pos_x_input.value(),
                   ui_widgets.pos_y_input.value())
        else:
            pos = (0, 0)

        get_screen_limits(pos)
        return pos

    def click_position_offset():
        if advanced_mode_enabled() and click_position_enabled():
            return ui_widgets.click_offset_input.value()
        else:
            return 0

    def click_position_offset_chance():
        if advanced_mode_enabled() and click_position_enabled():
            return ui_widgets.click_offset_chance_input.value()
        else:
            return 0

    def click_position_smoothing_enabled() -> bool:
        if advanced_mode_enabled() and click_position_enabled():
            return ui_widgets.click_offset_smoothing_checkbox.isChecked()
        else:
            return False

    def toggle_clicker_start_stop():
        global clicker_active
        clicker_active = not clicker_active
        if not clicker_active:
            ui_widgets.clickerstatus.setText("Off")
            ui_widgets.clickerstatus.setStyleSheet(INACTIVE_STYLE)
            rust_translation.stop_clicker()
            log(f"[{current_time()}] Clicker stopped")
        else:
            ui_widgets.clickerstatus.setText("On")
            ui_widgets.clickerstatus.setStyleSheet(ACTIVE_STYLE)
            ui_widgets.btn_reset.setDefault(False)
            clicker_engine_settings = {
                "click_amount": raw_click_speed(),
                "click_unit": click_unit(),
                "click_variation": click_speed_variation(),
                "click_limit": click_amount_limit(),
                "click_duty_cycle": duty_cycle(),
                "click_time_limit": click_time_limit(),
                "click_button": mouse_button(),
                "click_position": click_position(),
                "click_position_offset": click_position_offset(),
                "click_position_offset_chance": click_position_offset_chance(),
                "click_position_smoothing": click_position_smoothing_enabled(),
            }

            rust_translation.start_clicker(
                clicker_engine_settings, on_clicker_finished)
            log(f"[{current_time()}] Clicker started")

    hotkey_manager.set_toggle_callback(toggle_clicker_start_stop)

    # -----------------------------------------------------------------------
    # Click speed limit / warning
    # -----------------------------------------------------------------------

    def set_click_speed_limit():
        limits = {"second": 500, "minute": 30000,
                  "hour": 1800000, "day": 43200000}
        ui_widgets.click_speed_input.setMaximum(
            limits.get(ui_widgets.click_interval_combobox.currentText().lower(), 1000))

    set_click_speed_limit()
    ui_widgets.click_interval_combobox.currentIndexChanged.connect(
        set_click_speed_limit)

    def click_speed_warn():
        # Indirectly Irrelivant because Click Speed limit is currently hard capped at 500
        global show_high_speed_warn

        if not show_high_speed_warn:
            return
        cps = max(float(ui_widgets.click_speed_input.value()), 1)
        divisors = {"second": 1, "minute": 60, "hour": 3600, "day": 86400}
        unit = ui_widgets.click_interval_combobox.currentText().lower()

        if cps / divisors.get(unit, 1) >= 501:
            warning = QMessageBox()
            warning.setWindowTitle("High Click Speed")
            warning.setText(
                "Warning: Click speed is very high, this may cause issues.")
            warning.setIcon(QMessageBox.Icon.Warning)
            warning.setWindowIcon(window.windowIcon())
            dont_show_checkbox = QCheckBox("Do not show again")
            warning.setCheckBox(dont_show_checkbox)
            warning.exec()
            if dont_show_checkbox.isChecked():
                show_high_speed_warn = False

    ui_widgets.click_speed_input.valueChanged.connect(click_speed_warn)

    # -----------------------------------------------------------------------
    # Position picker
    # -----------------------------------------------------------------------

    def start_position_picker():
        def on_tick(seconds_left):
            ui_widgets.pick_position_button.setText(
                f"Picking Cursor position in {seconds_left - 1}s")

        def on_finish():
            pos = QCursor.pos()
            ui_widgets.pos_x_input.setValue(pos.x())
            ui_widgets.pos_y_input.setValue(pos.y())
            ui_widgets.pick_position_button.setText("Pick Position")
            log(f"Position picked: {pos.x()}, {pos.y()}, success")

        start_countdown(4, on_tick, on_finish)

    ui_widgets.pick_position_button.clicked.connect(start_position_picker)

    # -----------------------------------------------------------------------
    # Wire up signals
    # -----------------------------------------------------------------------
    ui_widgets.update_status_label.setVisible(False)
    ui_widgets.version_label.setText(CURRENT_VERSION)
    ui_widgets.btn_reset.clicked.connect(lambda: reset_defaults(
        ui_widgets, log=lambda m: log(f"[{current_time()}] {m}")))
    ui_widgets.btn_reset.clicked.connect(lambda: ui.Tabs.setCurrentIndex(0))
    ui_widgets.key_sequence.keySequenceChanged.connect(
        hotkey_manager.on_keybind_changed)
    ui_widgets.activation_type_combobox.currentIndexChanged.connect(
        hotkey_manager.set_keybind_mode)
    ui_widgets.local_total_clicks.setText(
        config.get("Settings", "Total_Clicks", fallback="0"))
    ui_widgets.local_total_time.setText(format_time(
        config.getfloat("Settings", "Total_Time", fallback=0.0)))
    ui_widgets.local_average_cpu_usage.setText(config.get(
        "Settings", "CPU_Average_Usage", fallback="0") + "%")
    ui_widgets.clickerstatus.setStyleSheet(INACTIVE_STYLE)

    # -----------------------------------------------------------------------
    # Initialize managers
    # -----------------------------------------------------------------------

    update_checker.initialize(ui_widgets, log_func, CURRENT_VERSION)

    update_checker.check_for_updates()
    if update_checker.get_update_available():
        ui_widgets.update_status_label.setVisible(True)
        ui_widgets.update_status_label.setText(
            '<html><head/><body><p><span style=" color:#1aff22;">Updates Available! Check my GitHub (Blur009)</span></p></body></html>'
        )
    telemetry.initialize(ui_widgets, log_func, CURRENT_VERSION)
    hotkey_manager.initialize(ui_widgets, log_func)
    hotkey_manager.set_keybind(keybind_hotkey)
    hotkey_manager.set_keybind_mode()

    # -----------------------------------------------------------------------
    # Save on exit
    # -----------------------------------------------------------------------

    def exit_handler():
        t = None
        if SUPABASE_URL and ui_widgets.telemetry_checkbox.isChecked():
            t = threading.Thread(target=telemetry.send_telemetry_data)
            t.start()
        save_settings(ui_widgets, config, keybind_hotkey=hotkey_manager.get_keybind(),
                      debug_mode=DEBUG_MODE, log=lambda m: log(f"[{current_time()}] {m}"))
        if t:
            t.join(timeout=6)

    atexit.register(exit_handler)
    window.show()
    sys.exit(app.exec())
