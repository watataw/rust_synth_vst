use crate::engine::LogicalCable;
use crate::i18n::Language;
use crate::physics::{CablePhysics, Vec2, CABLE_NODES};
use crate::ui::state::{
    ActiveCableDrag, ActiveDrawerDrag, ActiveKnobDrag, ActiveModuleDrag, ActivePanDrag,
    ContextMenuState, ModuleKind, ModuleVisual, PortDirection, RackTheme, RackUiState,
    RightDrawerTab, MODULE_HEIGHT, RACK_LEFT_MARGIN, RACK_TOP_Y, ROW_STRIDE, SYNTH_PRESETS,
};
use crate::ModularParams;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::{
    Align, Baseline, Color, FontId, LineCap, LineJoin, Paint, Path, Solidity,
};
use std::cell::Cell;
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const JACK_HIT_RADIUS: f32 = 14.0;
const MENU_WIDTH: f32 = 195.0;
const MENU_ITEM_H: f32 = 27.0;
const DRAWER_WIDTH: f32 = 264.0;
const DRAWER_TAB_W: f32 = 30.0;
const DRAWER_TAB_H: f32 = 112.0;
const RESIZE_GRIP_SIZE: f32 = 22.0;

// =============================================================================
// GESTION NATIF WINDOWS (Vrai redimensionnement HWND + Fullscreen dans le DAW)
// =============================================================================
#[cfg(target_os = "windows")]
mod win32_window {
    use std::cell::RefCell;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: Rect,
        rc_work: Rect,
        dw_flags: u32,
    }

    struct SavedWindowState {
        root_hwnd: isize,
        style: isize,
        rect: Rect,
    }

    thread_local! {
        static SAVED_FULLSCREEN: RefCell<Option<SavedWindowState>> = const { RefCell::new(None) };
    }

    const GA_PARENT: u32 = 1;
    const GA_ROOT: u32 = 2;
    const GWL_STYLE: i32 = -16;
    const WS_OVERLAPPEDWINDOW: isize = 0x00CF0000;
    const WS_POPUP: isize = 0x80000000u32 as isize;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const MONITOR_DEFAULTTONEAREST: u32 = 0x00000002;

    #[link(name = "user32")]
    extern "system" {
        fn GetActiveWindow() -> isize;
        fn GetAncestor(hwnd: isize, ga_flags: u32) -> isize;
        fn GetParent(hwnd: isize) -> isize;
        fn GetWindowRect(hwnd: isize, lp_rect: *mut Rect) -> i32;
        fn GetClientRect(hwnd: isize, lp_rect: *mut Rect) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            hwnd_insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
        fn GetWindowLongPtrW(hwnd: isize, n_index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: isize, n_index: i32, dw_new_long: isize) -> isize;
        fn MonitorFromWindow(hwnd: isize, dw_flags: u32) -> isize;
        fn GetMonitorInfoW(hmonitor: isize, lpmi: *mut MonitorInfo) -> i32;
    }

    pub fn is_fullscreen() -> bool {
        SAVED_FULLSCREEN.with(|s| s.borrow().is_some())
    }

    /// Redimensionne réellement le cadre Windows du plugin VST3 et ses conteneurs DAW.
    pub fn resize_host_window(new_client_w: i32, new_client_h: i32) {
        if is_fullscreen() {
            return;
        }
        unsafe {
            let active = GetActiveWindow();
            if active == 0 {
                return;
            }
            let root = GetAncestor(active, GA_ROOT);
            let target = if root != 0 { root } else { active };

            let mut win_rect = Rect::default();
            let mut cli_rect = Rect::default();
            if GetWindowRect(target, &mut win_rect) != 0 && GetClientRect(target, &mut cli_rect) != 0 {
                let border_w = (win_rect.right - win_rect.left) - (cli_rect.right - cli_rect.left);
                let border_h = (win_rect.bottom - win_rect.top) - (cli_rect.bottom - cli_rect.top);
                SetWindowPos(
                    target,
                    0,
                    0,
                    0,
                    new_client_w + border_w.max(0),
                    new_client_h + border_h.max(0),
                    SWP_NOMOVE | SWP_NOZORDER,
                );
            }

            // Met aussi à jour les conteneurs intermédiaires éventuels
            let mut cur = active;
            for _ in 0..4 {
                if cur == 0 || cur == target {
                    break;
                }
                SetWindowPos(cur, 0, 0, 0, new_client_w, new_client_h, SWP_NOMOVE | SWP_NOZORDER);
                let p = GetParent(cur);
                if p == 0 {
                    cur = GetAncestor(cur, GA_PARENT);
                } else {
                    cur = p;
                }
            }
        }
    }

    pub fn toggle_fullscreen() -> bool {
        unsafe {
            let currently_full = is_fullscreen();
            if currently_full {
                SAVED_FULLSCREEN.with(|cell| {
                    if let Some(saved) = cell.borrow_mut().take() {
                        SetWindowLongPtrW(saved.root_hwnd, GWL_STYLE, saved.style);
                        SetWindowPos(
                            saved.root_hwnd,
                            0,
                            saved.rect.left,
                            saved.rect.top,
                            saved.rect.right - saved.rect.left,
                            saved.rect.bottom - saved.rect.top,
                            SWP_NOZORDER | SWP_FRAMECHANGED,
                        );
                    }
                });
                false
            } else {
                let active = GetActiveWindow();
                if active == 0 {
                    return false;
                }
                let root = GetAncestor(active, GA_ROOT);
                let target = if root != 0 { root } else { active };

                let mut old_rect = Rect::default();
                GetWindowRect(target, &mut old_rect);
                let old_style = GetWindowLongPtrW(target, GWL_STYLE);

                let mon = MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST);
                let mut mi = MonitorInfo {
                    cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                    rc_monitor: Rect::default(),
                    rc_work: Rect::default(),
                    dw_flags: 0,
                };
                if GetMonitorInfoW(mon, &mut mi) != 0 {
                    SAVED_FULLSCREEN.with(|cell| {
                        *cell.borrow_mut() = Some(SavedWindowState {
                            root_hwnd: target,
                            style: old_style,
                            rect: old_rect,
                        });
                    });

                    let new_style = (old_style & !WS_OVERLAPPEDWINDOW) | WS_POPUP;
                    SetWindowLongPtrW(target, GWL_STYLE, new_style);
                    SetWindowPos(
                        target,
                        0,
                        mi.rc_monitor.left,
                        mi.rc_monitor.top,
                        mi.rc_monitor.right - mi.rc_monitor.left,
                        mi.rc_monitor.bottom - mi.rc_monitor.top,
                        SWP_NOZORDER | SWP_FRAMECHANGED,
                    );
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod win32_window {
    pub fn is_fullscreen() -> bool {
        false
    }
    pub fn resize_host_window(_w: i32, _h: i32) {}
    pub fn toggle_fullscreen() -> bool {
        false
    }
}

struct ThemePalette {
    canvas_bg: Color,
    cabinet_bg: Color,
    wood_grain: Option<Color>,
    rail_bg: Color,
    rail_highlight: Color,
    module_top: Color,
    module_bottom: Color,
    module_border: Color,
    screw_bg: Color,
    screw_slot: Color,
    header_bg: Color,
    title_text: Color,
    knob_track: Color,
    knob_label: Color,
    port_nut: Color,
    port_label_in: Color,
    port_label_out: Color,
    screen_bg: Color,
    drawer_bg: Color,
    card_bg: Color,
    card_border: Color,
    ui_text: Color,
    ui_subtext: Color,
}

impl ThemePalette {
    fn from_theme(theme: RackTheme) -> Self {
        match theme {
            RackTheme::Current => Self {
                canvas_bg: Color::rgb(18, 20, 24),
                cabinet_bg: Color::rgb(24, 27, 32),
                wood_grain: None,
                rail_bg: Color::rgb(88, 95, 105),
                rail_highlight: Color::rgb(130, 138, 150),
                module_top: Color::rgb(52, 56, 63),
                module_bottom: Color::rgb(35, 38, 44),
                module_border: Color::rgb(82, 88, 98),
                screw_bg: Color::rgb(132, 138, 148),
                screw_slot: Color::rgb(28, 30, 34),
                header_bg: Color::rgb(20, 22, 27),
                title_text: Color::rgb(245, 248, 252),
                knob_track: Color::rgb(20, 22, 26),
                knob_label: Color::rgb(195, 202, 214),
                port_nut: Color::rgb(178, 185, 195),
                port_label_in: Color::rgb(180, 188, 200),
                port_label_out: Color::rgb(240, 244, 250),
                screen_bg: Color::rgb(12, 15, 20),
                drawer_bg: Color::rgba(24, 27, 33, 248),
                card_bg: Color::rgb(35, 39, 47),
                card_border: Color::rgb(64, 71, 84),
                ui_text: Color::rgb(245, 248, 252),
                ui_subtext: Color::rgb(150, 160, 175),
            },
            RackTheme::Dark => Self {
                canvas_bg: Color::rgb(9, 10, 13),
                cabinet_bg: Color::rgb(13, 15, 19),
                wood_grain: None,
                rail_bg: Color::rgb(48, 52, 60),
                rail_highlight: Color::rgb(78, 84, 96),
                module_top: Color::rgb(28, 31, 37),
                module_bottom: Color::rgb(18, 20, 24),
                module_border: Color::rgb(52, 58, 68),
                screw_bg: Color::rgb(85, 92, 102),
                screw_slot: Color::rgb(14, 16, 20),
                header_bg: Color::rgb(11, 13, 16),
                title_text: Color::rgb(240, 244, 250),
                knob_track: Color::rgb(10, 12, 15),
                knob_label: Color::rgb(168, 176, 190),
                port_nut: Color::rgb(135, 142, 154),
                port_label_in: Color::rgb(155, 164, 178),
                port_label_out: Color::rgb(235, 240, 248),
                screen_bg: Color::rgb(7, 9, 12),
                drawer_bg: Color::rgba(14, 16, 21, 250),
                card_bg: Color::rgb(24, 27, 34),
                card_border: Color::rgb(46, 52, 64),
                ui_text: Color::rgb(240, 244, 250),
                ui_subtext: Color::rgb(135, 145, 160),
            },
            RackTheme::Light => Self {
                canvas_bg: Color::rgb(214, 220, 228),
                cabinet_bg: Color::rgb(198, 205, 214),
                wood_grain: None,
                rail_bg: Color::rgb(145, 154, 166),
                rail_highlight: Color::rgb(235, 240, 246),
                module_top: Color::rgb(242, 245, 249),
                module_bottom: Color::rgb(222, 228, 236),
                module_border: Color::rgb(168, 176, 188),
                screw_bg: Color::rgb(155, 164, 176),
                screw_slot: Color::rgb(75, 82, 92),
                header_bg: Color::rgb(42, 48, 58),
                title_text: Color::rgb(250, 252, 255),
                knob_track: Color::rgb(185, 192, 204),
                knob_label: Color::rgb(44, 50, 60),
                port_nut: Color::rgb(150, 158, 170),
                port_label_in: Color::rgb(44, 50, 60),
                port_label_out: Color::rgb(240, 244, 250),
                screen_bg: Color::rgb(18, 22, 28),
                drawer_bg: Color::rgba(236, 240, 246, 250),
                card_bg: Color::rgb(250, 252, 255),
                card_border: Color::rgb(185, 194, 206),
                ui_text: Color::rgb(28, 34, 44),
                ui_subtext: Color::rgb(92, 102, 116),
            },
            RackTheme::VintageWood => Self {
                canvas_bg: Color::rgb(36, 23, 15),
                cabinet_bg: Color::rgb(74, 44, 25),
                wood_grain: Some(Color::rgba(42, 22, 10, 85)),
                rail_bg: Color::rgb(168, 138, 72),
                rail_highlight: Color::rgb(224, 194, 118),
                module_top: Color::rgb(236, 225, 202),
                module_bottom: Color::rgb(214, 198, 168),
                module_border: Color::rgb(142, 118, 82),
                screw_bg: Color::rgb(212, 175, 55),
                screw_slot: Color::rgb(72, 54, 20),
                header_bg: Color::rgb(52, 34, 22),
                title_text: Color::rgb(248, 238, 216),
                knob_track: Color::rgb(176, 158, 128),
                knob_label: Color::rgb(56, 38, 24),
                port_nut: Color::rgb(206, 172, 74),
                port_label_in: Color::rgb(56, 38, 24),
                port_label_out: Color::rgb(248, 238, 216),
                screen_bg: Color::rgb(22, 28, 22),
                drawer_bg: Color::rgba(48, 30, 19, 250),
                card_bg: Color::rgb(68, 43, 28),
                card_border: Color::rgb(138, 104, 62),
                ui_text: Color::rgb(248, 238, 218),
                ui_subtext: Color::rgb(204, 182, 150),
            },
        }
    }
}

struct DrawerModuleEntry {
    kind: ModuleKind,
    y: f32,
    h: f32,
}

pub struct RackCanvas {
    state: Arc<Mutex<RackUiState>>,
    params: Arc<ModularParams>,
    font_regular: Cell<Option<FontId>>,
    font_bold: Cell<Option<FontId>>,
    last_size: Cell<(f32, f32)>,
    window_resize_drag: Cell<Option<(f32, f32, f32, f32)>>,
}

impl RackCanvas {
    pub fn new(
        cx: &mut Context,
        state: Arc<Mutex<RackUiState>>,
        params: Arc<ModularParams>,
    ) -> Handle<'_, Self> {
        let timer = cx.add_timer(Duration::from_millis(16), None, |cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                cx.needs_redraw();
            }
        });
        cx.start_timer(timer);

        Self {
            state,
            params,
            font_regular: Cell::new(None),
            font_bold: Cell::new(None),
            last_size: Cell::new((1360.0, 620.0)),
            window_resize_drag: Cell::new(None),
        }
            .build(cx, |_| {})
    }

    fn ensure_fonts(&self, canvas: &mut Canvas) {
        if self.font_regular.get().is_none() {
            let reg = canvas
                .add_font("C:\\Windows\\Fonts\\segoeui.ttf")
                .or_else(|_| canvas.add_font("C:\\Windows\\Fonts\\arial.ttf"))
                .ok();
            let bold = canvas
                .add_font("C:\\Windows\\Fonts\\segoeuib.ttf")
                .or_else(|_| canvas.add_font("C:\\Windows\\Fonts\\arialbd.ttf"))
                .ok()
                .or(reg);
            self.font_regular.set(reg);
            self.font_bold.set(bold);
        }
    }

    fn text_paint(&self, color: Color, size: f32, bold: bool, align: Align, baseline: Baseline) -> Paint {
        let mut paint = Paint::color(color);
        paint.set_font_size(size);
        paint.set_text_align(align);
        paint.set_text_baseline(baseline);
        if let Some(fid) = if bold {
            self.font_bold.get()
        } else {
            self.font_regular.get()
        } {
            paint.set_font(&[fid]);
        }
        paint
    }

    fn drawer_panel_x(win_w: f32, drawer_anim: f32) -> f32 {
        win_w - DRAWER_WIDTH * drawer_anim
    }

    fn drawer_tabs_rects(
        win_w: f32,
        win_h: f32,
        drawer_anim: f32,
    ) -> [(RightDrawerTab, (f32, f32, f32, f32)); 2] {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        let tab_x = panel_x - DRAWER_TAB_W;
        let total_h = DRAWER_TAB_H * 2.0 + 8.0;
        let start_y = ((win_h - total_h) * 0.5).max(56.0);
        [
            (
                RightDrawerTab::Modules,
                (tab_x, start_y, DRAWER_TAB_W, DRAWER_TAB_H),
            ),
            (
                RightDrawerTab::Presets,
                (tab_x, start_y + DRAWER_TAB_H + 8.0, DRAWER_TAB_W, DRAWER_TAB_H),
            ),
        ]
    }

    fn fullscreen_button_rect(win_w: f32, drawer_anim: f32) -> (f32, f32, f32, f32) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        (panel_x - 244.0, 10.0, 114.0, 30.0)
    }

    fn settings_button_rect(win_w: f32, drawer_anim: f32) -> (f32, f32, f32, f32) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        (panel_x - 122.0, 10.0, 110.0, 30.0)
    }

    fn settings_popover_rect(win_w: f32, drawer_anim: f32) -> (f32, f32, f32, f32) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        (panel_x - 266.0, 46.0, 254.0, 294.0)
    }

    /// Calcule les positions verticales des cartes DSP et FX dans le drawer Modules (avec scroll).
    fn layout_module_drawer_entries(scroll: f32) -> (f32, Vec<DrawerModuleEntry>, f32, f32) {
        let card_h = 54.0;
        let card_stride = 60.0;
        let dsp_header_y = 56.0 - scroll;
        let mut entries = Vec::with_capacity(ModuleKind::ALL.len());

        let mut cur_y = dsp_header_y + 22.0;
        for &kind in &ModuleKind::DSP_MODULES {
            entries.push(DrawerModuleEntry {
                kind,
                y: cur_y,
                h: card_h,
            });
            cur_y += card_stride;
        }

        cur_y += 10.0;
        let fx_header_y = cur_y;
        cur_y += 22.0;
        for &kind in &ModuleKind::FX_MODULES {
            entries.push(DrawerModuleEntry {
                kind,
                y: cur_y,
                h: card_h,
            });
            cur_y += card_stride;
        }

        let total_content_h = (cur_y + scroll) + 24.0;
        (dsp_header_y, entries, fx_header_y, total_content_h)
    }

    fn preset_card_rect(panel_x: f32, index: usize) -> (f32, f32, f32, f32) {
        let card_x = panel_x + 12.0;
        let card_y = 58.0 + index as f32 * 76.0;
        let card_w = DRAWER_WIDTH - 24.0;
        (card_x, card_y, card_w, 68.0)
    }

    fn trash_zone_rect(win_w: f32, win_h: f32) -> (f32, f32, f32, f32) {
        (win_w - 260.0, win_h - 68.0, 238.0, 52.0)
    }

    fn confirm_modal_rects(win_w: f32, win_h: f32) -> ((f32, f32, f32, f32), (f32, f32, f32, f32), (f32, f32, f32, f32)) {
        let mw = 416.0;
        let mh = 176.0;
        let mx = (win_w - mw) * 0.5;
        let my = (win_h - mh) * 0.5;
        let cancel_btn = (mx + 24.0, my + mh - 50.0, 160.0, 34.0);
        let confirm_btn = (mx + mw - 204.0, my + mh - 50.0, 180.0, 34.0);
        ((mx, my, mw, mh), cancel_btn, confirm_btn)
    }

    fn hit_test_port(state: &RackUiState, screen_x: f32, screen_y: f32) -> Option<(usize, PortDirection, usize, Vec2)> {
        let world_cursor = state.screen_to_world(screen_x, screen_y);
        let hit_r = JACK_HIT_RADIUS / state.zoom.sqrt();
        for m in &state.modules {
            for p in &m.ports {
                let pos = Vec2::new(m.x + p.local_x, m.world_top_y() + p.local_y);
                if pos.distance(world_cursor) <= hit_r {
                    return Some((m.slot_id, p.direction, p.index, pos));
                }
            }
        }
        None
    }

    fn compute_target_row_and_x(state: &RackUiState, world_pos: Vec2, ignore_slot: Option<usize>) -> (usize, f32) {
        let max_existing_row = state.modules.iter().map(|m| m.row).max().unwrap_or(0);
        let rel_y = (world_pos.y - RACK_TOP_Y).max(0.0);
        let raw_row = (rel_y / ROW_STRIDE).floor() as usize;
        let target_row = raw_row.min(max_existing_row + 1);

        for m in &state.modules {
            if Some(m.slot_id) == ignore_slot || m.row != target_row {
                continue;
            }
            let mid = m.x + m.kind.width() * 0.5;
            if world_pos.x < mid {
                return (target_row, m.x - 2.0);
            }
        }

        let end_x = state
            .modules
            .iter()
            .filter(|m| m.row == target_row && Some(m.slot_id) != ignore_slot)
            .map(|m| m.x + m.kind.width() + 2.0)
            .fold(RACK_LEFT_MARGIN, f32::max);

        (target_row, end_x)
    }

    fn midi_note_name(v_oct: f32) -> String {
        let midi = (v_oct * 12.0 + 60.0).round() as i32;
        let clamped = midi.clamp(0, 127);
        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let note = names[(clamped % 12) as usize];
        let octave = (clamped / 12) - 1;
        format!("{}{}", note, octave)
    }

    fn draw_module_visualizer(
        &self,
        canvas: &mut Canvas,
        m: &ModuleVisual,
        state: &RackUiState,
        palette: &ThemePalette,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let sx = x + 14.0;
        let sy = y + 48.0;
        let sw = w - 28.0;
        let sh = 74.0;

        let mut screen = Path::new();
        screen.rounded_rect(sx, sy, sw, sh, 4.0);
        canvas.fill_path(&screen, &Paint::color(palette.screen_bg));

        let mut bezel = Paint::color(Color::rgb(68, 75, 86));
        bezel.set_line_width(1.0);
        canvas.stroke_path(&screen, &bezel);

        let mut grid_paint = Paint::color(Color::rgba(255, 255, 255, 16));
        grid_paint.set_line_width(1.0);
        let mut mid_line = Path::new();
        mid_line.move_to(sx + 4.0, sy + sh * 0.5);
        mid_line.line_to(sx + sw - 4.0, sy + sh * 0.5);
        canvas.stroke_path(&mid_line, &grid_paint);

        let (ar, ag, ab) = m.kind.accent_color();
        let accent = Color::rgb(ar, ag, ab);

        match m.kind {
            ModuleKind::MidiToCv => {
                let v_oct = state.port_signal_value(m.slot_id, 0);
                let gate_on = state.port_signal_value(m.slot_id, 1) > 2.0;
                let vel = (state.port_signal_value(m.slot_id, 3) / 5.0).clamp(0.0, 1.0);
                let note_str = if gate_on {
                    Self::midi_note_name(v_oct)
                } else {
                    "---".to_string()
                };

                let note_paint = self.text_paint(
                    if gate_on { accent } else { Color::rgb(95, 105, 120) },
                    24.0,
                    true,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(sx + 12.0, sy + 28.0, &note_str, &note_paint);

                let mut badge = Path::new();
                badge.rounded_rect(sx + sw - 48.0, sy + 12.0, 38.0, 18.0, 3.0);
                canvas.fill_path(
                    &badge,
                    &Paint::color(if gate_on {
                        Color::rgb(46, 213, 115)
                    } else {
                        Color::rgb(32, 38, 46)
                    }),
                );
                let g_txt = self.text_paint(
                    if gate_on { Color::rgb(12, 16, 20) } else { Color::rgb(110, 120, 132) },
                    9.5,
                    true,
                    Align::Center,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(sx + sw - 29.0, sy + 21.0, "GATE", &g_txt);

                let mut vel_bg = Path::new();
                vel_bg.rounded_rect(sx + 10.0, sy + 52.0, sw - 20.0, 10.0, 2.0);
                canvas.fill_path(&vel_bg, &Paint::color(Color::rgb(26, 32, 40)));

                if vel > 0.01 {
                    let mut vel_fill = Path::new();
                    vel_fill.rounded_rect(sx + 10.0, sy + 52.0, (sw - 20.0) * vel, 10.0, 2.0);
                    canvas.fill_path(&vel_fill, &Paint::color(accent));
                }
            }

            ModuleKind::Vco => {
                let coarse = m.knobs[0].param.load();
                let fine = m.knobs[1].param.load();
                let pw = m.knobs[3].param.load();
                let freq = 261.63 * 2.0f32.powf((coarse + fine) / 12.0);

                let hz_label = if freq >= 1000.0 {
                    format!("{:.2} kHz", freq / 1000.0)
                } else {
                    format!("{:.1} Hz", freq)
                };
                let hz_paint = self.text_paint(
                    Color::rgb(175, 188, 204),
                    9.5,
                    true,
                    Align::Right,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(sx + sw - 6.0, sy + 4.0, &hz_label, &hz_paint);

                let cycles = (1.5 + ((coarse + 24.0) / 48.0) * 2.5).clamp(1.5, 4.0);
                let mut sq_path = Path::new();
                let mut saw_path = Path::new();
                let steps = 80;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let phase = (u * cycles).fract();
                    let px = sx + 6.0 + u * (sw - 12.0);

                    let sq_y = if phase < pw { -1.0 } else { 1.0 };
                    let py_sq = sy + sh * 0.54 + sq_y * 19.0;
                    if i == 0 {
                        sq_path.move_to(px, py_sq);
                    } else {
                        sq_path.line_to(px, py_sq);
                    }

                    let saw_y = 1.0 - 2.0 * phase;
                    let py_saw = sy + sh * 0.54 + saw_y * 17.0;
                    if i == 0 {
                        saw_path.move_to(px, py_saw);
                    } else {
                        saw_path.line_to(px, py_saw);
                    }
                }

                let mut sq_paint = Paint::color(Color::rgba(90, 195, 255, 130));
                sq_paint.set_line_width(1.5);
                canvas.stroke_path(&sq_path, &sq_paint);

                let mut saw_paint = Paint::color(accent);
                saw_paint.set_line_width(2.2);
                canvas.stroke_path(&saw_path, &saw_paint);
            }

            ModuleKind::Vcf => {
                let cutoff = m.knobs[0].param.load();
                let res = m.knobs[1].param.load();
                let cut_norm = ((cutoff.ln() - 20.0f32.ln()) / (16000.0f32.ln() - 20.0f32.ln()))
                    .clamp(0.05, 0.95);

                let mut curve = Path::new();
                let steps = 64;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let px = sx + 6.0 + u * (sw - 12.0);
                    let ratio = u / cut_norm;
                    let res_peak = (-((u - cut_norm) * 14.0).powi(2)).exp() * res * 26.0;
                    let drop = if ratio > 1.0 {
                        ((ratio - 1.0) * 5.2).min(1.0) * 34.0
                    } else {
                        0.0
                    };
                    let py = (sy + 28.0 - res_peak + drop).clamp(sy + 6.0, sy + sh - 6.0);
                    if i == 0 {
                        curve.move_to(px, py);
                    } else {
                        curve.line_to(px, py);
                    }
                }

                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                canvas.stroke_path(&curve, &stroke);

                let cx_pos = sx + 6.0 + cut_norm * (sw - 12.0);
                let mut marker = Path::new();
                marker.move_to(cx_pos, sy + 6.0);
                marker.line_to(cx_pos, sy + sh - 6.0);
                let mut m_paint = Paint::color(Color::rgba(255, 255, 255, 45));
                m_paint.set_line_width(1.0);
                canvas.stroke_path(&marker, &m_paint);
            }

            ModuleKind::Adsr => {
                let atk = m.knobs[0].param.load();
                let dec = m.knobs[1].param.load();
                let sus = m.knobs[2].param.load();
                let rel = m.knobs[3].param.load();

                let pad_x = 8.0;
                let usable_w = sw - pad_x * 2.0;
                let top_y = sy + 10.0;
                let bot_y = sy + sh - 10.0;
                let h_span = bot_y - top_y;

                let a_w = (0.08 + (atk / 2.0).sqrt() * 0.24) * usable_w;
                let d_w = (0.08 + (dec / 3.0).sqrt() * 0.24) * usable_w;
                let r_w = (0.08 + (rel / 4.0).sqrt() * 0.24) * usable_w;
                let s_w = (usable_w - a_w - d_w - r_w).max(14.0);

                let p0 = (sx + pad_x, bot_y);
                let p1 = (p0.0 + a_w, top_y);
                let p2 = (p1.0 + d_w, bot_y - sus * h_span);
                let p3 = (p2.0 + s_w, p2.1);
                let p4 = (p3.0 + r_w, bot_y);

                let mut env_path = Path::new();
                env_path.move_to(p0.0, p0.1);
                env_path.line_to(p1.0, p1.1);
                env_path.line_to(p2.0, p2.1);
                env_path.line_to(p3.0, p3.1);
                env_path.line_to(p4.0, p4.1);

                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                stroke.set_line_join(LineJoin::Round);
                canvas.stroke_path(&env_path, &stroke);

                let env_norm = (state.port_signal_value(m.slot_id, 0) / 5.0).clamp(0.0, 1.0);
                if env_norm > 0.005 {
                    let dot_y = bot_y - env_norm * h_span;
                    let mut bar = Path::new();
                    bar.rect(sx + sw - 12.0, dot_y, 5.0, bot_y - dot_y);
                    canvas.fill_path(&bar, &Paint::color(accent));
                }
            }

            ModuleKind::Lfo => {
                let rate = m.knobs[0].param.load();
                let wave_idx = m.knobs[1].param.load().round() as i32;
                let unipolar = m.knobs[2].param.load() > 0.5;
                let live_phase = state.port_signal_value(m.slot_id, 3).clamp(0.0, 1.0);
                let live_v = state.port_signal_value(m.slot_id, 0);

                let right_edge_x = sx + sw - 10.0;
                let left_edge_x = sx + 6.0;
                let wave_w = right_edge_x - left_edge_x;

                let mut wave_path = Path::new();

                if wave_idx == 4 {
                    if let Some(hist) = state.lfo_histories.get(&m.slot_id) {
                        let count = hist.len();
                        for (i, &v_sample) in hist.iter().enumerate() {
                            let u = if count > 1 {
                                i as f32 / (count - 1) as f32
                            } else {
                                1.0
                            };
                            let px = left_edge_x + u * wave_w;
                            let norm_y = if unipolar {
                                (v_sample / 5.0).clamp(0.0, 1.0)
                            } else {
                                ((v_sample + 5.0) / 10.0).clamp(0.0, 1.0)
                            };
                            let py = sy + sh - 10.0 - norm_y * (sh - 20.0);
                            if i == 0 {
                                wave_path.move_to(px, py);
                            } else {
                                wave_path.line_to(px, py);
                            }
                        }
                    }
                } else {
                    let cycles = (1.2 + (rate / 40.0).sqrt() * 2.8).clamp(1.2, 4.0);
                    let steps = 84;
                    for i in 0..=steps {
                        let u = i as f32 / steps as f32;
                        let px = left_edge_x + u * wave_w;
                        let t = (live_phase - (1.0 - u) * cycles).rem_euclid(1.0);

                        let raw_bipolar = match wave_idx {
                            0 => (t * TAU).sin(),
                            1 => {
                                if t < 0.5 {
                                    4.0 * t - 1.0
                                } else {
                                    3.0 - 4.0 * t
                                }
                            }
                            2 => 2.0 * t - 1.0,
                            _ => {
                                if t < 0.5 {
                                    1.0
                                } else {
                                    -1.0
                                }
                            }
                        };

                        let volts = if unipolar {
                            (raw_bipolar + 1.0) * 2.5
                        } else {
                            raw_bipolar * 5.0
                        };
                        let norm_y = if unipolar {
                            (volts / 5.0).clamp(0.0, 1.0)
                        } else {
                            ((volts + 5.0) / 10.0).clamp(0.0, 1.0)
                        };
                        let py = sy + sh - 10.0 - norm_y * (sh - 20.0);

                        if i == 0 {
                            wave_path.move_to(px, py);
                        } else {
                            wave_path.line_to(px, py);
                        }
                    }
                }

                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                stroke.set_line_join(LineJoin::Round);
                canvas.stroke_path(&wave_path, &stroke);

                let norm_y = if unipolar {
                    (live_v / 5.0).clamp(0.0, 1.0)
                } else {
                    ((live_v + 5.0) / 10.0).clamp(0.0, 1.0)
                };
                let led_y = sy + sh - 10.0 - norm_y * (sh - 20.0);

                let mut halo = Path::new();
                halo.circle(right_edge_x, led_y, 6.5);
                canvas.fill_path(&halo, &Paint::color(Color::rgba(ar, ag, ab, 95)));

                let mut live_dot = Path::new();
                live_dot.circle(right_edge_x, led_y, 3.8);
                canvas.fill_path(&live_dot, &Paint::color(Color::rgb(255, 255, 255)));
            }

            ModuleKind::Vca => {
                let level = m.knobs[0].param.load();
                let exp_blend = m.knobs[2].param.load();

                let mut curve = Path::new();
                let steps = 40;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let lin = (level + u).clamp(0.0, 1.0);
                    let exp = lin * lin;
                    let g = lin * (1.0 - exp_blend) + exp * exp_blend;
                    let px = sx + 8.0 + u * (sw - 32.0);
                    let py = sy + sh - 10.0 - g * (sh - 20.0);
                    if i == 0 {
                        curve.move_to(px, py);
                    } else {
                        curve.line_to(px, py);
                    }
                }
                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                canvas.stroke_path(&curve, &stroke);

                let out_lvl = (state.port_signal_level(m.slot_id, 0) / 5.0).clamp(0.0, 1.0);
                let bar_h = out_lvl * (sh - 16.0);
                let mut meter = Path::new();
                meter.rounded_rect(sx + sw - 16.0, sy + sh - 8.0 - bar_h, 8.0, bar_h, 2.0);
                canvas.fill_path(&meter, &Paint::color(accent));
            }

            ModuleKind::AudioOutput => {
                let lvl_l = (state.port_signal_level(m.slot_id, 0) / 5.0).clamp(0.0, 1.0);
                let lvl_r = (state.port_signal_level(m.slot_id, 1) / 5.0).clamp(0.0, 1.0);
                let val_l = (state.port_signal_value(m.slot_id, 0) / 5.0).clamp(-1.0, 1.0);

                let mut wave = Path::new();
                let steps = 48;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let env = (u * PI).sin();
                    let amp = (lvl_l * 0.7 + val_l.abs() * 0.3) * env * 24.0;
                    let px = sx + 8.0 + u * (sw - 42.0);
                    let py = sy + sh * 0.5 + (u * TAU * 3.0).sin() * amp;
                    if i == 0 {
                        wave.move_to(px, py);
                    } else {
                        wave.line_to(px, py);
                    }
                }
                let mut w_paint = Paint::color(accent);
                w_paint.set_line_width(2.0);
                canvas.stroke_path(&wave, &w_paint);

                for (idx, lvl) in [lvl_l, lvl_r].iter().enumerate() {
                    let bx = sx + sw - 24.0 + idx as f32 * 9.0;
                    let bh = lvl * (sh - 16.0);
                    let mut bar = Path::new();
                    bar.rounded_rect(bx, sy + sh - 8.0 - bh, 6.0, bh, 2.0);
                    let col = if *lvl > 0.92 {
                        Color::rgb(235, 77, 75)
                    } else {
                        accent
                    };
                    canvas.fill_path(&bar, &Paint::color(col));
                }
            }

            ModuleKind::Overdrive | ModuleKind::Distortion => {
                let drive = m.knobs.first().map(|k| k.param.load()).unwrap_or(0.5);
                let gain = 1.0 + drive * 7.0;
                let mut curve = Path::new();
                let steps = 60;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let x_in = u * 2.0 - 1.0;
                    let shaped = if m.kind == ModuleKind::Overdrive {
                        (x_in * gain).tanh()
                    } else {
                        (x_in * gain * 1.4).sin().clamp(-0.92, 0.92)
                    };
                    let px = sx + 8.0 + u * (sw - 16.0);
                    let py = sy + sh * 0.5 - shaped * (sh * 0.38);
                    if i == 0 {
                        curve.move_to(px, py);
                    } else {
                        curve.line_to(px, py);
                    }
                }
                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                canvas.stroke_path(&curve, &stroke);
            }

            ModuleKind::Chorus | ModuleKind::Phaser => {
                let rate = m.knobs.first().map(|k| k.param.load()).unwrap_or(0.5);
                let depth = m.knobs.get(1).map(|k| k.param.load()).unwrap_or(0.6);
                let waves = if m.kind == ModuleKind::Chorus { 2 } else { 3 };
                for w_idx in 0..waves {
                    let mut p = Path::new();
                    let phase_off = w_idx as f32 * 1.25;
                    let steps = 54;
                    for i in 0..=steps {
                        let u = i as f32 / steps as f32;
                        let px = sx + 8.0 + u * (sw - 16.0);
                        let freq = 1.5 + rate * 2.5;
                        let py = sy + sh * 0.5 + (u * TAU * freq + phase_off).sin() * depth * 22.0;
                        if i == 0 {
                            p.move_to(px, py);
                        } else {
                            p.line_to(px, py);
                        }
                    }
                    let alpha_w = if w_idx == 0 { 245 } else { 130 };
                    let mut stroke = Paint::color(Color::rgba(ar, ag, ab, alpha_w));
                    stroke.set_line_width(if w_idx == 0 { 2.1 } else { 1.4 });
                    canvas.stroke_path(&p, &stroke);
                }
            }
        }
    }

    fn draw_module(
        &self,
        canvas: &mut Canvas,
        m: &ModuleVisual,
        state: &RackUiState,
        palette: &ThemePalette,
    ) {
        let x = m.x;
        let y = m.world_top_y();
        let w = m.kind.width();
        let h = MODULE_HEIGHT;
        let (ar, ag, ab) = m.kind.accent_color();
        let (mod_title, mod_sub, _) = state.tr().module_info(state.language, m.kind);

        // 1. Façade du module
        let mut panel = Path::new();
        panel.rounded_rect(x, y, w, h, 5.0);
        let bg = Paint::linear_gradient(
            x,
            y,
            x + w,
            y + h,
            palette.module_top,
            palette.module_bottom,
        );
        canvas.fill_path(&panel, &bg);

        let mut border = Paint::color(palette.module_border);
        border.set_line_width(1.2);
        canvas.stroke_path(&panel, &border);

        let grain_col = if state.theme == RackTheme::Light || state.theme == RackTheme::VintageWood {
            Color::rgba(0, 0, 0, 6)
        } else {
            Color::rgba(255, 255, 255, 5)
        };
        let mut grain_paint = Paint::color(grain_col);
        grain_paint.set_line_width(1.0);
        let mut gy = y + 8.0;
        while gy < y + h - 8.0 {
            let mut line = Path::new();
            line.move_to(x + 2.0, gy);
            line.line_to(x + w - 2.0, gy);
            canvas.stroke_path(&line, &grain_paint);
            gy += 4.0;
        }

        // 2. Vis de montage
        for (sx, sy) in [(10.0, 10.0), (w - 10.0, 10.0), (10.0, h - 10.0), (w - 10.0, h - 10.0)] {
            let mut screw = Path::new();
            screw.circle(x + sx, y + sy, 4.0);
            canvas.fill_path(&screw, &Paint::color(palette.screw_bg));

            let mut slot = Path::new();
            slot.move_to(x + sx - 2.3, y + sy - 2.3);
            slot.line_to(x + sx + 2.3, y + sy + 2.3);
            let mut slot_paint = Paint::color(palette.screw_slot);
            slot_paint.set_line_width(1.2);
            canvas.stroke_path(&slot, &slot_paint);
        }

        // 3. Bandeau titre traduit
        let mut header = Path::new();
        header.rounded_rect(x + 16.0, y + 7.0, w - 32.0, 34.0, 4.0);
        canvas.fill_path(&header, &Paint::color(palette.header_bg));

        let mut accent_strip = Path::new();
        accent_strip.rounded_rect(x + 16.0, y + 7.0, 4.5, 34.0, 2.0);
        canvas.fill_path(&accent_strip, &Paint::color(Color::rgb(ar, ag, ab)));

        let title_paint = self.text_paint(
            palette.title_text,
            13.0,
            true,
            Align::Center,
            Baseline::Top,
        );
        let _ = canvas.fill_text(x + w * 0.5 + 2.0, y + 10.0, mod_title, &title_paint);

        let sub_paint = self.text_paint(
            Color::rgb(ar, ag, ab),
            8.5,
            true,
            Align::Center,
            Baseline::Top,
        );
        let _ = canvas.fill_text(x + w * 0.5 + 2.0, y + 26.5, mod_sub, &sub_paint);

        // 4. Écran visuel
        self.draw_module_visualizer(canvas, m, state, palette, x, y, w);

        // 5. Potentiomètres
        let active_knob = state.knob_drag.as_ref().and_then(|kd| {
            if kd.slot_id == m.slot_id {
                Some(kd.knob_idx)
            } else {
                None
            }
        });

        for (k_idx, knob) in m.knobs.iter().enumerate() {
            let kx = x + knob.local_x;
            let ky = y + knob.local_y;
            let val = knob.param.load();
            let norm = ((val - knob.min) / (knob.max - knob.min)).clamp(0.0, 1.0);
            let is_active = active_knob == Some(k_idx);

            let start_angle = 0.75 * PI;
            let end_angle = 2.25 * PI;
            let active_angle = start_angle + norm * (end_angle - start_angle);

            let mut track = Path::new();
            track.arc(kx, ky, knob.radius + 4.5, start_angle, end_angle, Solidity::Hole);
            let mut track_paint = Paint::color(palette.knob_track);
            track_paint.set_line_width(3.0);
            track_paint.set_line_cap(LineCap::Round);
            canvas.stroke_path(&track, &track_paint);

            let mut active_arc = Path::new();
            active_arc.arc(kx, ky, knob.radius + 4.5, start_angle, active_angle, Solidity::Hole);
            let mut active_paint = Paint::color(Color::rgb(ar, ag, ab));
            active_paint.set_line_width(3.2);
            active_paint.set_line_cap(LineCap::Round);
            canvas.stroke_path(&active_arc, &active_paint);

            let mut body = Path::new();
            body.circle(kx, ky, knob.radius);
            let body_grad = Paint::radial_gradient(
                kx - 3.5,
                ky - 3.5,
                2.0,
                knob.radius,
                Color::rgb(52, 56, 64),
                Color::rgb(17, 19, 23),
            );
            canvas.fill_path(&body, &body_grad);

            let ix = kx + active_angle.cos() * (knob.radius - 2.5);
            let iy = ky + active_angle.sin() * (knob.radius - 2.5);
            let mut pointer = Path::new();
            pointer.move_to(kx + active_angle.cos() * 3.5, ky + active_angle.sin() * 3.5);
            pointer.line_to(ix, iy);
            let mut p_paint = Paint::color(Color::rgb(250, 252, 255));
            p_paint.set_line_width(2.2);
            p_paint.set_line_cap(LineCap::Round);
            canvas.stroke_path(&pointer, &p_paint);

            let label_text = if is_active {
                knob.formatted_value()
            } else {
                knob.label.to_string()
            };
            let lbl_paint = self.text_paint(
                if is_active {
                    Color::rgb(ar, ag, ab)
                } else {
                    palette.knob_label
                },
                9.5,
                true,
                Align::Center,
                Baseline::Top,
            );
            let _ = canvas.fill_text(kx, ky + knob.radius + 6.5, &label_text, &lbl_paint);
        }

        // 6. Prises Jack 3.5mm + LEDs + halo au survol
        for port in &m.ports {
            let px = x + port.local_x;
            let py = y + port.local_y;
            let is_hovered = state.hovered_port == Some((m.slot_id, port.direction, port.index));

            if port.direction == PortDirection::Output {
                let mut out_plate = Path::new();
                out_plate.rounded_rect(px - 19.0, py - 15.0, 38.0, 44.0, 4.0);
                canvas.fill_path(&out_plate, &Paint::color(palette.header_bg));
            }

            if is_hovered {
                let mut hover_ring = Path::new();
                hover_ring.circle(px, py, 13.0);
                let mut hr_paint = Paint::color(Color::rgb(ar, ag, ab));
                hr_paint.set_line_width(1.8);
                canvas.stroke_path(&hover_ring, &hr_paint);
            }

            let mut nut = Path::new();
            nut.circle(px, py, 10.0);
            canvas.fill_path(&nut, &Paint::color(palette.port_nut));

            let mut hole = Path::new();
            hole.circle(px, py, 5.0);
            canvas.fill_path(&hole, &Paint::color(Color::rgb(10, 12, 15)));

            if port.direction == PortDirection::Output {
                let level = (state.port_signal_level(m.slot_id, port.index) / 5.0).clamp(0.0, 1.0);
                let mut led = Path::new();
                led.circle(px + 13.0, py - 10.0, 3.0);
                let led_color = if level > 0.02 {
                    Color::rgba(ar, ag, ab, (80.0 + level * 175.0) as u8)
                } else {
                    Color::rgb(35, 40, 48)
                };
                canvas.fill_path(&led, &Paint::color(led_color));
            }

            let plbl = self.text_paint(
                if port.direction == PortDirection::Output {
                    palette.port_label_out
                } else {
                    palette.port_label_in
                },
                9.0,
                true,
                Align::Center,
                Baseline::Top,
            );
            let _ = canvas.fill_text(px, py + 13.0, port.name, &plbl);
        }
    }

    fn draw_cable(canvas: &mut Canvas, physics: &CablePhysics, alpha: u8) {
        let mut path = Path::new();
        path.move_to(physics.pos[0].x, physics.pos[0].y);

        for i in 1..(CABLE_NODES - 1) {
            let mid_x = (physics.pos[i].x + physics.pos[i + 1].x) * 0.5;
            let mid_y = (physics.pos[i].y + physics.pos[i + 1].y) * 0.5;
            path.quad_to(physics.pos[i].x, physics.pos[i].y, mid_x, mid_y);
        }
        let last = physics.pos[CABLE_NODES - 1];
        path.line_to(last.x, last.y);

        let mut outer = Paint::color(Color::rgba(14, 16, 20, alpha));
        outer.set_line_width(7.2);
        outer.set_line_cap(LineCap::Round);
        outer.set_line_join(LineJoin::Round);
        canvas.stroke_path(&path, &outer);

        let (r, g, b) = physics.color;
        let mut inner = Paint::color(Color::rgba(r, g, b, alpha));
        inner.set_line_width(4.6);
        inner.set_line_cap(LineCap::Round);
        inner.set_line_join(LineJoin::Round);
        canvas.stroke_path(&path, &inner);

        let mut highlight = Paint::color(Color::rgba(255, 255, 255, alpha / 4));
        highlight.set_line_width(1.3);
        highlight.set_line_cap(LineCap::Round);
        canvas.stroke_path(&path, &highlight);

        for end_pt in [physics.pos[0], last] {
            let mut plug_outer = Path::new();
            plug_outer.circle(end_pt.x, end_pt.y, 7.2);
            canvas.fill_path(&plug_outer, &Paint::color(Color::rgb(26, 28, 33)));

            let mut plug_ring = Path::new();
            plug_ring.circle(end_pt.x, end_pt.y, 4.3);
            canvas.fill_path(&plug_ring, &Paint::color(Color::rgb(r, g, b)));
        }
    }

    fn draw_top_bar_and_settings(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
    ) {
        let tr = state.tr();

        // Indicateur de Zoom en haut à gauche
        let zoom_str = format!("ZOOM {:.0}%  ({})", state.zoom * 100.0, tr.zoom_hint);
        let z_paint = self.text_paint(
            palette.ui_subtext,
            10.5,
            true,
            Align::Left,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(20.0, 24.0, &zoom_str, &z_paint);

        // Bouton FULLSCREEN à côté de SETTINGS
        let is_full = win32_window::is_fullscreen();
        let (fx, fy, fw, fh) = Self::fullscreen_button_rect(win_w, state.drawer_anim);
        let mut f_btn = Path::new();
        f_btn.rounded_rect(fx, fy, fw, fh, 6.0);
        canvas.fill_path(
            &f_btn,
            &Paint::color(if is_full {
                Color::rgb(46, 204, 113)
            } else {
                palette.card_bg
            }),
        );
        let mut fb_border = Paint::color(palette.card_border);
        fb_border.set_line_width(1.2);
        canvas.stroke_path(&f_btn, &fb_border);

        let f_txt = self.text_paint(
            if is_full {
                Color::rgb(12, 16, 20)
            } else {
                palette.ui_text
            },
            10.5,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let f_label = if is_full {
            tr.exit_fullscreen_btn
        } else {
            tr.fullscreen_btn
        };
        let _ = canvas.fill_text(fx + fw * 0.5, fy + fh * 0.5, f_label, &f_txt);

        // Bouton SETTINGS en haut à droite
        let (sx, sy, sw, sh) = Self::settings_button_rect(win_w, state.drawer_anim);
        let mut btn = Path::new();
        btn.rounded_rect(sx, sy, sw, sh, 6.0);
        canvas.fill_path(
            &btn,
            &Paint::color(if state.settings_open {
                Color::rgb(52, 152, 219)
            } else {
                palette.card_bg
            }),
        );
        let mut b_border = Paint::color(palette.card_border);
        b_border.set_line_width(1.2);
        canvas.stroke_path(&btn, &b_border);

        let btn_txt = self.text_paint(
            if state.settings_open {
                Color::rgb(255, 255, 255)
            } else {
                palette.ui_text
            },
            10.5,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(sx + sw * 0.5, sy + sh * 0.5, tr.settings_btn, &btn_txt);

        // Panneau déroulant Settings (Langue + Thèmes + Délai Info-bulle)
        if state.settings_open {
            let (px, py, pw, ph) = Self::settings_popover_rect(win_w, state.drawer_anim);
            let mut pop = Path::new();
            pop.rounded_rect(px, py, pw, ph, 7.0);
            canvas.fill_path(&pop, &Paint::color(palette.drawer_bg));

            let mut p_border = Paint::color(palette.card_border);
            p_border.set_line_width(1.4);
            canvas.stroke_path(&pop, &p_border);

            // Section 1 : Langue (EN / FR)
            let l_hdr = self.text_paint(palette.ui_subtext, 9.2, true, Align::Left, Baseline::Top);
            let _ = canvas.fill_text(px + 14.0, py + 10.0, tr.settings_language, &l_hdr);

            let lang_w = (pw - 32.0) * 0.5;
            for (i, (lang_opt, lang_lbl)) in [(Language::En, "English"), (Language::Fr, "Français")]
                .iter()
                .enumerate()
            {
                let bx = px + 12.0 + i as f32 * (lang_w + 8.0);
                let by = py + 27.0;
                let selected = state.language == *lang_opt;
                let mut l_btn = Path::new();
                l_btn.rounded_rect(bx, by, lang_w, 26.0, 4.0);
                canvas.fill_path(
                    &l_btn,
                    &Paint::color(if selected {
                        Color::rgb(52, 152, 219)
                    } else {
                        palette.card_bg
                    }),
                );
                let lt = self.text_paint(
                    if selected {
                        Color::rgb(255, 255, 255)
                    } else {
                        palette.ui_text
                    },
                    11.0,
                    selected,
                    Align::Center,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(bx + lang_w * 0.5, by + 13.0, lang_lbl, &lt);
            }

            // Section 2 : Thème
            let t_hdr = self.text_paint(palette.ui_subtext, 9.2, true, Align::Left, Baseline::Top);
            let _ = canvas.fill_text(px + 14.0, py + 64.0, tr.settings_theme, &t_hdr);

            for (idx, &theme_opt) in RackTheme::ALL.iter().enumerate() {
                let item_y = py + 81.0 + idx as f32 * 32.0;
                let selected = state.theme == theme_opt;

                let mut row_bg = Path::new();
                row_bg.rounded_rect(px + 12.0, item_y, pw - 24.0, 27.0, 4.0);
                canvas.fill_path(
                    &row_bg,
                    &Paint::color(if selected {
                        Color::rgb(52, 152, 219)
                    } else {
                        palette.card_bg
                    }),
                );

                let row_txt = self.text_paint(
                    if selected {
                        Color::rgb(255, 255, 255)
                    } else {
                        palette.ui_text
                    },
                    10.8,
                    selected,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(
                    px + 20.0,
                    item_y + 13.5,
                    tr.theme_label(theme_opt),
                    &row_txt,
                );
            }

            // Section 3 : Délai info-bulle au survol (200ms / 500ms / 1000ms / 1500ms)
            let d_hdr = self.text_paint(palette.ui_subtext, 9.2, true, Align::Left, Baseline::Top);
            let _ = canvas.fill_text(px + 14.0, py + 218.0, tr.settings_tooltip_delay, &d_hdr);

            let delays: [u64; 4] = [200, 500, 1000, 1500];
            let pill_w = (pw - 36.0) / 4.0;
            for (i, &ms) in delays.iter().enumerate() {
                let bx = px + 12.0 + i as f32 * (pill_w + 4.0);
                let by = py + 236.0;
                let selected = state.tooltip_delay_ms == ms;

                let mut p_btn = Path::new();
                p_btn.rounded_rect(bx, by, pill_w, 26.0, 4.0);
                canvas.fill_path(
                    &p_btn,
                    &Paint::color(if selected {
                        Color::rgb(52, 152, 219)
                    } else {
                        palette.card_bg
                    }),
                );

                let lbl = if ms >= 1000 {
                    format!("{:.1}s", ms as f32 / 1000.0)
                } else {
                    format!("{}ms", ms)
                };
                let pt = self.text_paint(
                    if selected {
                        Color::rgb(255, 255, 255)
                    } else {
                        palette.ui_text
                    },
                    10.2,
                    selected,
                    Align::Center,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(bx + pill_w * 0.5, by + 13.0, &lbl, &pt);
            }
        }
    }

    fn draw_right_drawers(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let tr = state.tr();
        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);

        // 1. Onglets verticaux : MODULES & PRESETS
        for (tab_kind, (tx, ty, tw, th)) in Self::drawer_tabs_rects(win_w, win_h, state.drawer_anim) {
            let is_active = state.active_drawer == Some(tab_kind);
            let mut tab = Path::new();
            tab.rounded_rect(tx, ty, tw + 4.0, th, 6.0);
            canvas.fill_path(
                &tab,
                &Paint::color(if is_active {
                    Color::rgb(52, 152, 219)
                } else {
                    palette.card_bg
                }),
            );

            let mut tab_border = Paint::color(palette.card_border);
            tab_border.set_line_width(1.2);
            canvas.stroke_path(&tab, &tab_border);

            canvas.save();
            canvas.translate(tx + tw * 0.5, ty + th * 0.5);
            canvas.rotate(-PI * 0.5);
            let label = match tab_kind {
                RightDrawerTab::Modules => tr.tab_modules,
                RightDrawerTab::Presets => tr.tab_presets,
            };
            let tab_paint = self.text_paint(
                if is_active {
                    Color::rgb(255, 255, 255)
                } else {
                    palette.ui_text
                },
                10.5,
                true,
                Align::Center,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(0.0, 0.0, label, &tab_paint);
            canvas.restore();
        }

        if state.drawer_anim <= 0.01 {
            return;
        }

        // 2. Fond du drawer
        let mut bg = Path::new();
        bg.rect(panel_x, 0.0, DRAWER_WIDTH, win_h);
        canvas.fill_path(&bg, &Paint::color(palette.drawer_bg));

        let mut sep = Path::new();
        sep.move_to(panel_x, 0.0);
        sep.line_to(panel_x, win_h);
        let mut sep_paint = Paint::color(palette.card_border);
        sep_paint.set_line_width(1.5);
        canvas.stroke_path(&sep, &sep_paint);

        match state.active_drawer.unwrap_or(RightDrawerTab::Modules) {
            RightDrawerTab::Modules => {
                let (dsp_hdr_y, entries, fx_hdr_y, _) =
                    Self::layout_module_drawer_entries(state.drawer_scroll);

                canvas.save();
                canvas.scissor(panel_x, 50.0, DRAWER_WIDTH, (win_h - 54.0).max(10.0));

                // En-tête section DSP
                let sec_paint = self.text_paint(
                    Color::rgb(52, 172, 224),
                    9.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, dsp_hdr_y + 4.0, tr.section_dsp, &sec_paint);

                // En-tête section FX / Effets
                let fx_sec_paint = self.text_paint(
                    Color::rgb(255, 121, 63),
                    9.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, fx_hdr_y + 4.0, tr.section_fx, &fx_sec_paint);

                let card_x = panel_x + 12.0;
                let card_w = DRAWER_WIDTH - 24.0;
                for entry in &entries {
                    let (ar, ag, ab) = entry.kind.accent_color();
                    let (m_title, m_sub, m_desc) = tr.module_info(state.language, entry.kind);

                    let mut card = Path::new();
                    card.rounded_rect(card_x, entry.y, card_w, entry.h, 5.0);
                    canvas.fill_path(&card, &Paint::color(palette.card_bg));

                    let mut c_border = Paint::color(palette.card_border);
                    c_border.set_line_width(1.0);
                    canvas.stroke_path(&card, &c_border);

                    let mut strip = Path::new();
                    strip.rounded_rect(card_x, entry.y, 5.0, entry.h, 2.0);
                    canvas.fill_path(&strip, &Paint::color(Color::rgb(ar, ag, ab)));

                    let t_paint = self.text_paint(
                        palette.ui_text,
                        11.8,
                        true,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(card_x + 13.0, entry.y + 7.0, m_title, &t_paint);

                    let badge_paint = self.text_paint(
                        Color::rgb(ar, ag, ab),
                        8.4,
                        true,
                        Align::Right,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(card_x + card_w - 9.0, entry.y + 8.5, m_sub, &badge_paint);

                    let desc_paint = self.text_paint(
                        palette.ui_subtext,
                        9.6,
                        false,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(card_x + 13.0, entry.y + 28.0, m_desc, &desc_paint);
                }

                canvas.restore();

                // Bandeau supérieur fixe du drawer Modules
                let mut top_mask = Path::new();
                top_mask.rect(panel_x + 1.0, 0.0, DRAWER_WIDTH - 1.0, 50.0);
                canvas.fill_path(&top_mask, &Paint::color(palette.drawer_bg));

                let hdr_paint = self.text_paint(
                    palette.ui_text,
                    12.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, 12.0, tr.drawer_modules_title, &hdr_paint);

                let hint_paint = self.text_paint(
                    palette.ui_subtext,
                    9.8,
                    false,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, 30.0, tr.drawer_modules_hint, &hint_paint);
            }

            RightDrawerTab::Presets => {
                let hdr_paint = self.text_paint(
                    palette.ui_text,
                    12.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, 12.0, tr.drawer_presets_title, &hdr_paint);

                let hint_paint = self.text_paint(
                    palette.ui_subtext,
                    9.8,
                    false,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, 30.0, tr.drawer_presets_hint, &hint_paint);

                for idx in 0..SYNTH_PRESETS.len() {
                    let (cx, cy, cw, ch) = Self::preset_card_rect(panel_x, idx);
                    let (p_name, p_cat, p_desc) = tr.preset_info(state.language, idx);

                    let mut card = Path::new();
                    card.rounded_rect(cx, cy, cw, ch, 5.0);
                    canvas.fill_path(&card, &Paint::color(palette.card_bg));

                    let mut c_border = Paint::color(palette.card_border);
                    c_border.set_line_width(1.0);
                    canvas.stroke_path(&card, &c_border);

                    let mut strip = Path::new();
                    strip.rounded_rect(cx, cy, 5.0, ch, 2.0);
                    canvas.fill_path(&strip, &Paint::color(Color::rgb(52, 172, 224)));

                    let t_paint = self.text_paint(
                        palette.ui_text,
                        11.6,
                        true,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(cx + 13.0, cy + 8.0, p_name, &t_paint);

                    let cat_paint = self.text_paint(
                        Color::rgb(52, 172, 224),
                        8.5,
                        true,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(cx + 13.0, cy + 26.0, p_cat, &cat_paint);

                    let desc_paint = self.text_paint(
                        palette.ui_subtext,
                        9.4,
                        false,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(cx + 13.0, cy + 43.0, p_desc, &desc_paint);
                }
            }
        }
    }

    fn draw_port_hover_toast(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let (Some((slot_id, dir, port_idx)), Some(since)) = (state.hovered_port, state.hover_since)
        else {
            return;
        };

        if (since.elapsed().as_millis() as u64) < state.tooltip_delay_ms {
            return;
        }

        let Some(module) = state.modules.iter().find(|m| m.slot_id == slot_id) else {
            return;
        };

        let tr = state.tr();
        let (p_title, p_desc) = tr.port_tooltip(state.language, module.kind, dir, port_idx);
        let (ar, ag, ab) = module.kind.accent_color();

        let toast_w = 276.0;
        let toast_h = 68.0;
        let tx = (state.hover_screen_pos.x + 16.0).clamp(12.0, (win_w - toast_w - 12.0).max(12.0));
        let ty = (state.hover_screen_pos.y - toast_h - 12.0)
            .clamp(46.0, (win_h - toast_h - 12.0).max(46.0));

        let mut box_path = Path::new();
        box_path.rounded_rect(tx, ty, toast_w, toast_h, 6.0);
        canvas.fill_path(&box_path, &Paint::color(palette.drawer_bg));

        let mut border = Paint::color(Color::rgb(ar, ag, ab));
        border.set_line_width(1.4);
        canvas.stroke_path(&box_path, &border);

        let badge = match dir {
            PortDirection::Input => tr.port_input_badge,
            PortDirection::Output => tr.port_output_badge,
        };
        let b_paint = self.text_paint(
            Color::rgb(ar, ag, ab),
            8.5,
            true,
            Align::Right,
            Baseline::Top,
        );
        let _ = canvas.fill_text(tx + toast_w - 10.0, ty + 9.0, badge, &b_paint);

        let t_paint = self.text_paint(
            palette.ui_text,
            11.5,
            true,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(tx + 12.0, ty + 8.0, p_title, &t_paint);

        // Découpage propre sur 2 lignes max pour que le texte tienne toujours dans le toast
        let words: Vec<&str> = p_desc.split_whitespace().collect();
        let mid = words.len() / 2;
        let (line1, line2) = if p_desc.len() > 42 && mid > 0 {
            (words[..mid].join(" "), words[mid..].join(" "))
        } else {
            (p_desc.to_string(), String::new())
        };

        let d_paint = self.text_paint(
            palette.ui_subtext,
            9.8,
            false,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(tx + 12.0, ty + 28.0, &line1, &d_paint);
        if !line2.is_empty() {
            let _ = canvas.fill_text(tx + 12.0, ty + 43.0, &line2, &d_paint);
        }
    }

    fn draw_overwrite_modal(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        preset_idx: usize,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let tr = state.tr();
        let (p_name, _, _) = tr.preset_info(state.language, preset_idx);

        let mut backdrop = Path::new();
        backdrop.rect(0.0, 0.0, win_w, win_h);
        canvas.fill_path(&backdrop, &Paint::color(Color::rgba(0, 0, 0, 165)));

        let ((mx, my, mw, mh), (cx, cy, cw, ch), (ox, oy, ow, oh)) =
            Self::confirm_modal_rects(win_w, win_h);

        let mut modal = Path::new();
        modal.rounded_rect(mx, my, mw, mh, 8.0);
        canvas.fill_path(&modal, &Paint::color(palette.drawer_bg));

        let mut border = Paint::color(Color::rgb(235, 77, 75));
        border.set_line_width(1.6);
        canvas.stroke_path(&modal, &border);

        let title = self.text_paint(
            palette.ui_text,
            14.2,
            true,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(mx + 22.0, my + 18.0, tr.modal_overwrite_title, &title);

        let msg1 = format!("{} \"{}\"", tr.modal_overwrite_line1_prefix, p_name);
        let body_paint = self.text_paint(
            palette.ui_subtext,
            11.2,
            false,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(mx + 22.0, my + 50.0, &msg1, &body_paint);
        let _ = canvas.fill_text(mx + 22.0, my + 69.0, tr.modal_overwrite_line2, &body_paint);

        let mut c_btn = Path::new();
        c_btn.rounded_rect(cx, cy, cw, ch, 5.0);
        canvas.fill_path(&c_btn, &Paint::color(palette.card_bg));
        let mut cb_border = Paint::color(palette.card_border);
        cb_border.set_line_width(1.2);
        canvas.stroke_path(&c_btn, &cb_border);

        let c_txt = self.text_paint(
            palette.ui_text,
            11.2,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(cx + cw * 0.5, cy + ch * 0.5, tr.modal_cancel, &c_txt);

        let mut o_btn = Path::new();
        o_btn.rounded_rect(ox, oy, ow, oh, 5.0);
        canvas.fill_path(&o_btn, &Paint::color(Color::rgb(235, 77, 75)));

        let o_txt = self.text_paint(
            Color::rgb(255, 255, 255),
            11.2,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(ox + ow * 0.5, oy + oh * 0.5, tr.modal_confirm, &o_txt);
    }
}

impl View for RackCanvas {
    fn element(&self) -> Option<&'static str> {
        Some("rack-canvas")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _| match window_event {
            WindowEvent::KeyDown(code, _) => {
                if *code == Code::Escape && win32_window::is_fullscreen() {
                    win32_window::toggle_fullscreen();
                    cx.needs_redraw();
                }
            }

            WindowEvent::MouseScroll(dx, dy) => {
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, win_h) = self.last_size.get();
                let modifiers = cx.modifiers();
                let mut state_guard = self.state.lock().unwrap();
                let state = &mut *state_guard;

                // Défilement vertical dans le drawer Modules si le curseur est dessus
                let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                if state.active_drawer == Some(RightDrawerTab::Modules) && mx >= panel_x {
                    let (_, _, _, total_h) = Self::layout_module_drawer_entries(0.0);
                    let max_scroll = (total_h - win_h).max(0.0);
                    state.drawer_scroll = (state.drawer_scroll - *dy * 36.0).clamp(0.0, max_scroll);
                    cx.needs_redraw();
                    return;
                }

                if modifiers.contains(Modifiers::CTRL) {
                    let old_zoom = state.zoom;
                    let factor = if *dy > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    let new_zoom = (old_zoom * factor).clamp(0.45, 2.2);

                    let world_x = (mx - state.pan_x) / old_zoom;
                    let world_y = (my - state.pan_y) / old_zoom;

                    state.zoom = new_zoom;
                    state.pan_x = mx - world_x * new_zoom;
                    state.pan_y = my - world_y * new_zoom;
                } else if modifiers.contains(Modifiers::SHIFT) {
                    state.pan_y += *dy * 45.0;
                } else {
                    state.pan_x += *dx * 45.0 + *dy * 45.0;
                }

                cx.needs_redraw();
            }

            WindowEvent::MouseDown(button) => {
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, win_h) = self.last_size.get();
                let modifiers = cx.modifiers();
                let mut state_guard = self.state.lock().unwrap();
                let state = &mut *state_guard;

                // 0. Modale de confirmation d'écrasement de preset
                if let Some(preset_idx) = state.pending_preset {
                    if *button == MouseButton::Left {
                        let (_, (cx_b, cy_b, cw_b, ch_b), (ox_b, oy_b, ow_b, oh_b)) =
                            Self::confirm_modal_rects(win_w, win_h);

                        if mx >= cx_b && mx <= cx_b + cw_b && my >= cy_b && my <= cy_b + ch_b {
                            state.pending_preset = None;
                        } else if mx >= ox_b && mx <= ox_b + ow_b && my >= oy_b && my <= oy_b + oh_b {
                            state.pending_preset = None;
                            state.apply_preset(preset_idx);
                        }
                        cx.needs_redraw();
                    }
                    return;
                }

                // Poignée de redimensionnement natif Windows (coin inférieur droit)
                if *button == MouseButton::Left
                    && !win32_window::is_fullscreen()
                    && mx >= win_w - RESIZE_GRIP_SIZE
                    && my >= win_h - RESIZE_GRIP_SIZE
                {
                    self.window_resize_drag.set(Some((mx, my, win_w, win_h)));
                    return;
                }

                // Clic molette : déplacement 2D libre de la vue
                if *button == MouseButton::Middle {
                    state.pan_drag = Some(ActivePanDrag {
                        last_screen: Vec2::new(mx, my),
                    });
                    return;
                }

                // 1. Menu contextuel clic droit
                if let Some(menu) = state.context_menu.take() {
                    if *button == MouseButton::Left
                        && mx >= menu.screen_x
                        && mx <= menu.screen_x + MENU_WIDTH
                        && my >= menu.screen_y
                    {
                        let item_idx = ((my - menu.screen_y) / MENU_ITEM_H).floor() as usize;
                        if item_idx < ModuleKind::ALL.len() {
                            state.spawn_module_at(ModuleKind::ALL[item_idx], 0, None);
                        } else if item_idx == ModuleKind::ALL.len() {
                            if let Some(slot) = menu.target_slot {
                                state.remove_module(slot);
                            }
                        }
                        cx.needs_redraw();
                        return;
                    }
                }

                // 2. Bouton FULLSCREEN (à côté de Settings)
                let (fx, fy, fw, fh) = Self::fullscreen_button_rect(win_w, state.drawer_anim);
                if *button == MouseButton::Left && mx >= fx && mx <= fx + fw && my >= fy && my <= fy + fh {
                    win32_window::toggle_fullscreen();
                    cx.needs_redraw();
                    return;
                }

                // 3. Bouton et Popover SETTINGS
                let (sx, sy, sw, sh) = Self::settings_button_rect(win_w, state.drawer_anim);
                if *button == MouseButton::Left && mx >= sx && mx <= sx + sw && my >= sy && my <= sy + sh {
                    state.settings_open = !state.settings_open;
                    cx.needs_redraw();
                    return;
                }

                if state.settings_open && *button == MouseButton::Left {
                    let (px, py, pw, ph) = Self::settings_popover_rect(win_w, state.drawer_anim);
                    if mx >= px && mx <= px + pw && my >= py && my <= py + ph {
                        // Choix Langue (EN / FR)
                        let lang_w = (pw - 32.0) * 0.5;
                        for (i, lang_opt) in [Language::En, Language::Fr].iter().enumerate() {
                            let bx = px + 12.0 + i as f32 * (lang_w + 8.0);
                            let by = py + 27.0;
                            if mx >= bx && mx <= bx + lang_w && my >= by && my <= by + 26.0 {
                                state.language = *lang_opt;
                                cx.needs_redraw();
                                return;
                            }
                        }

                        // Choix Thème
                        for (idx, &theme_opt) in RackTheme::ALL.iter().enumerate() {
                            let item_y = py + 81.0 + idx as f32 * 32.0;
                            if my >= item_y && my <= item_y + 27.0 {
                                state.theme = theme_opt;
                                cx.needs_redraw();
                                return;
                            }
                        }

                        // Choix Délai Info-bulle port
                        let delays: [u64; 4] = [200, 500, 1000, 1500];
                        let pill_w = (pw - 36.0) / 4.0;
                        for (i, &ms) in delays.iter().enumerate() {
                            let bx = px + 12.0 + i as f32 * (pill_w + 4.0);
                            let by = py + 236.0;
                            if mx >= bx && mx <= bx + pill_w && my >= by && my <= by + 26.0 {
                                state.tooltip_delay_ms = ms;
                                cx.needs_redraw();
                                return;
                            }
                        }
                        return;
                    } else {
                        state.settings_open = false;
                    }
                }

                // 4. Onglets des drawers droits (MODULES / PRESETS)
                for (tab_kind, (tx, ty, tw, th)) in
                    Self::drawer_tabs_rects(win_w, win_h, state.drawer_anim)
                {
                    if *button == MouseButton::Left
                        && mx >= tx
                        && mx <= tx + tw
                        && my >= ty
                        && my <= ty + th
                    {
                        if state.active_drawer == Some(tab_kind) {
                            state.active_drawer = None;
                        } else {
                            state.active_drawer = Some(tab_kind);
                        }
                        cx.needs_redraw();
                        return;
                    }
                }

                // 5. Contenu du Drawer actif
                let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                if *button == MouseButton::Left && state.active_drawer.is_some() && mx >= panel_x {
                    match state.active_drawer.unwrap() {
                        RightDrawerTab::Modules => {
                            let (_, entries, _, _) =
                                Self::layout_module_drawer_entries(state.drawer_scroll);
                            let card_x = panel_x + 12.0;
                            let card_w = DRAWER_WIDTH - 24.0;
                            for entry in entries {
                                if my >= 50.0
                                    && mx >= card_x
                                    && mx <= card_x + card_w
                                    && my >= entry.y
                                    && my <= entry.y + entry.h
                                {
                                    let world_pos = state.screen_to_world(mx, my);
                                    let (target_row, insert_x) =
                                        Self::compute_target_row_and_x(state, world_pos, None);
                                    state.drawer_drag = Some(ActiveDrawerDrag {
                                        kind: entry.kind,
                                        screen_cursor: Vec2::new(mx, my),
                                        target_row,
                                        insert_x,
                                    });
                                    cx.needs_redraw();
                                    return;
                                }
                            }
                        }
                        RightDrawerTab::Presets => {
                            for idx in 0..SYNTH_PRESETS.len() {
                                let (card_x, card_y, card_w, card_h) =
                                    Self::preset_card_rect(panel_x, idx);
                                if mx >= card_x
                                    && mx <= card_x + card_w
                                    && my >= card_y
                                    && my <= card_y + card_h
                                {
                                    state.pending_preset = Some(idx);
                                    cx.needs_redraw();
                                    return;
                                }
                            }
                        }
                    }
                    return;
                }

                let world_pos = state.screen_to_world(mx, my);

                // 6. Clic droit -> menu contextuel rapide
                if *button == MouseButton::Right {
                    let target_slot = state
                        .modules
                        .iter()
                        .find(|m| {
                            let my_top = m.world_top_y();
                            world_pos.x >= m.x
                                && world_pos.x <= m.x + m.kind.width()
                                && world_pos.y >= my_top
                                && world_pos.y <= my_top + MODULE_HEIGHT
                        })
                        .map(|m| m.slot_id);

                    state.context_menu = Some(ContextMenuState {
                        screen_x: mx,
                        screen_y: my,
                        target_slot,
                    });
                    cx.needs_redraw();
                    return;
                }

                if *button == MouseButton::Left {
                    // 7. Clic sur une prise Jack (en conservant la couleur si on déplace un câble!)
                    if let Some((slot_id, dir, port_idx, port_pos)) =
                        Self::hit_test_port(state, mx, my)
                    {
                        if let Some((removed, old_color)) =
                            state.disconnect_cable_at_port(slot_id, dir, port_idx)
                        {
                            let (anchor_slot, anchor_port, anchor_dir, anchor_pos) = match dir {
                                PortDirection::Input => {
                                    let p = state
                                        .modules
                                        .iter()
                                        .find(|m| m.slot_id == removed.src_slot)
                                        .and_then(|m| {
                                            m.port_world_pos(PortDirection::Output, removed.src_port)
                                        })
                                        .unwrap_or(port_pos);
                                    (removed.src_slot, removed.src_port, PortDirection::Output, p)
                                }
                                PortDirection::Output => {
                                    let p = state
                                        .modules
                                        .iter()
                                        .find(|m| m.slot_id == removed.dst_slot)
                                        .and_then(|m| {
                                            m.port_world_pos(PortDirection::Input, removed.dst_port)
                                        })
                                        .unwrap_or(port_pos);
                                    (removed.dst_slot, removed.dst_port, PortDirection::Input, p)
                                }
                            };

                            let mut phys = CablePhysics::new(anchor_pos, world_pos, 0);
                            phys.color = old_color;

                            state.cable_drag = Some(ActiveCableDrag {
                                from_slot: anchor_slot,
                                from_port: anchor_port,
                                from_dir: anchor_dir,
                                physics: phys,
                                world_mouse: world_pos,
                                preserved_color: Some(old_color),
                            });
                        } else {
                            state.cable_drag = Some(ActiveCableDrag {
                                from_slot: slot_id,
                                from_port: port_idx,
                                from_dir: dir,
                                physics: CablePhysics::new(
                                    port_pos,
                                    world_pos,
                                    state.cables.len(),
                                ),
                                world_mouse: world_pos,
                                preserved_color: None,
                            });
                        }
                        cx.needs_redraw();
                        return;
                    }

                    // 8. Clic sur un potentiomètre (Ctrl+Clic = reset à la valeur par défaut!)
                    let mut reset_slot = None;
                    for m in &state.modules {
                        let my_top = m.world_top_y();
                        for (k_idx, knob) in m.knobs.iter().enumerate() {
                            let kx = m.x + knob.local_x;
                            let ky = my_top + knob.local_y;
                            if Vec2::new(kx, ky).distance(world_pos) <= knob.radius + 4.0 {
                                if modifiers.contains(Modifiers::CTRL) {
                                    knob.param.store(knob.default);
                                    reset_slot = Some(m.slot_id);
                                } else {
                                    state.knob_drag = Some(ActiveKnobDrag {
                                        slot_id: m.slot_id,
                                        knob_idx: k_idx,
                                        start_y: my,
                                        start_val: knob.param.load(),
                                    });
                                }
                                break;
                            }
                        }
                        if reset_slot.is_some() || state.knob_drag.is_some() {
                            break;
                        }
                    }

                    if let Some(slot) = reset_slot {
                        state.sync_module_knobs_to_dsp(slot);
                        cx.needs_redraw();
                        return;
                    }
                    if state.knob_drag.is_some() {
                        cx.needs_redraw();
                        return;
                    }

                    // 9. Clic sur la façade d'un module pour le déplacer horizontalement/verticalement
                    for m in &state.modules {
                        let my_top = m.world_top_y();
                        if world_pos.x >= m.x
                            && world_pos.x <= m.x + m.kind.width()
                            && world_pos.y >= my_top
                            && world_pos.y <= my_top + MODULE_HEIGHT
                        {
                            state.module_drag = Some(ActiveModuleDrag {
                                slot_id: m.slot_id,
                                grab_offset_x: world_pos.x - m.x,
                                screen_cursor: Vec2::new(mx, my),
                                target_row: m.row,
                                insert_x: m.x,
                                over_trash: false,
                            });
                            cx.needs_redraw();
                            return;
                        }
                    }
                }
            }

            WindowEvent::MouseMove(mx, my) => {
                // Redimensionnement natif Windows en cours
                if let Some((start_mx, start_my, start_w, start_h)) = self.window_resize_drag.get() {
                    let new_w = (start_w + (*mx - start_mx)).max(820.0) as i32;
                    let new_h = (start_h + (*my - start_my)).max(520.0) as i32;
                    win32_window::resize_host_window(new_w, new_h);
                    return;
                }

                let (win_w, win_h) = self.last_size.get();
                let mut state_guard = self.state.lock().unwrap();
                let state = &mut *state_guard;

                // Détection de survol sur les prises Jack pour l'info-bulle temporisée
                let hit_port = Self::hit_test_port(state, *mx, *my)
                    .map(|(slot_id, dir, port_idx, _)| (slot_id, dir, port_idx));
                if hit_port != state.hovered_port {
                    state.hovered_port = hit_port;
                    state.hover_since = hit_port.map(|_| Instant::now());
                }
                if hit_port.is_some() {
                    state.hover_screen_pos = Vec2::new(*mx, *my);
                }

                if let Some(p_drag) = &mut state.pan_drag {
                    state.pan_x += *mx - p_drag.last_screen.x;
                    state.pan_y += *my - p_drag.last_screen.y;
                    p_drag.last_screen = Vec2::new(*mx, *my);
                    return;
                }

                let world_pos = state.screen_to_world(*mx, *my);

                if let Some(drag) = &mut state.cable_drag {
                    drag.world_mouse = world_pos;
                    drag.physics.wake();
                }

                if let Some(k_drag) = state
                    .knob_drag
                    .as_ref()
                    .map(|d| (d.slot_id, d.knob_idx, d.start_y, d.start_val))
                {
                    let (slot_id, knob_idx, start_y, start_val) = k_drag;
                    if let Some(m) = state.modules.iter().find(|m| m.slot_id == slot_id) {
                        let knob = &m.knobs[knob_idx];
                        let delta_norm = (start_y - *my) / 140.0;
                        let new_val = (start_val + delta_norm * (knob.max - knob.min))
                            .clamp(knob.min, knob.max);
                        knob.param.store(new_val);
                    }
                    state.sync_module_knobs_to_dsp(slot_id);
                }

                if state.drawer_drag.is_some() {
                    let (target_row, insert_x) =
                        Self::compute_target_row_and_x(state, world_pos, None);
                    if let Some(d_drag) = &mut state.drawer_drag {
                        d_drag.screen_cursor = Vec2::new(*mx, *my);
                        d_drag.target_row = target_row;
                        d_drag.insert_x = insert_x;
                    }
                }

                if let Some((slot_id, offset)) = state
                    .module_drag
                    .as_ref()
                    .map(|d| (d.slot_id, d.grab_offset_x))
                {
                    let (target_row, insert_x) =
                        Self::compute_target_row_and_x(state, world_pos, Some(slot_id));
                    if let Some(m) = state.modules.iter_mut().find(|m| m.slot_id == slot_id) {
                        m.x = (world_pos.x - offset).max(RACK_LEFT_MARGIN);
                        m.row = target_row;
                    }
                    let (tx, ty, tw, th) = Self::trash_zone_rect(win_w, win_h);
                    let over_trash = *mx >= tx && *mx <= tx + tw && *my >= ty && *my <= ty + th;
                    if let Some(m_drag) = &mut state.module_drag {
                        m_drag.screen_cursor = Vec2::new(*mx, *my);
                        m_drag.target_row = target_row;
                        m_drag.insert_x = insert_x;
                        m_drag.over_trash = over_trash;
                    }
                    for c in &mut state.cables {
                        c.physics.wake();
                    }
                }
            }

            WindowEvent::MouseUp(button) => {
                if *button == MouseButton::Left {
                    self.window_resize_drag.set(None);
                }

                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, _) = self.last_size.get();
                let mut state_guard = self.state.lock().unwrap();
                let state = &mut *state_guard;

                if *button == MouseButton::Middle {
                    state.pan_drag = None;
                }

                if *button == MouseButton::Left {
                    // Fin du Drag & Drop depuis le Drawer Modules
                    if let Some(d_drag) = state.drawer_drag.take() {
                        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                        if mx < panel_x {
                            state.spawn_module_at(
                                d_drag.kind,
                                d_drag.target_row,
                                Some(d_drag.insert_x),
                            );
                        } else {
                            state.spawn_module_at(d_drag.kind, 0, None);
                        }
                    }

                    // Fin du Drag & Drop de câble (avec conservation de couleur!)
                    if let Some(drag) = state.cable_drag.take() {
                        if let Some((target_slot, target_dir, target_port, _)) =
                            Self::hit_test_port(state, mx, my)
                        {
                            if drag.from_dir != target_dir {
                                let logical = match drag.from_dir {
                                    PortDirection::Output => LogicalCable {
                                        src_slot: drag.from_slot,
                                        src_port: drag.from_port,
                                        dst_slot: target_slot,
                                        dst_port: target_port,
                                    },
                                    PortDirection::Input => LogicalCable {
                                        src_slot: target_slot,
                                        src_port: target_port,
                                        dst_slot: drag.from_slot,
                                        dst_port: drag.from_port,
                                    },
                                };
                                state.connect_cable_with_color(logical, drag.preserved_color);
                            }
                        }
                    }

                    // Fin du Drag & Drop d'un module existant
                    if let Some(m_drag) = state.module_drag.take() {
                        if m_drag.over_trash {
                            state.remove_module(m_drag.slot_id);
                        } else {
                            if let Some(m) = state.modules.iter_mut().find(|m| m.slot_id == m_drag.slot_id) {
                                m.row = m_drag.target_row;
                                m.x = m_drag.insert_x;
                            }
                            state.pack_modules();
                        }
                        for c in &mut state.cables {
                            c.physics.wake();
                        }
                    }

                    state.knob_drag = None;
                    cx.needs_redraw();
                }
            }

            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        self.ensure_fonts(canvas);

        let bounds = cx.bounds();
        self.last_size.set((bounds.w, bounds.h));

        let mut state_guard = self.state.lock().unwrap();
        let state = &mut *state_guard;
        state.collect_garbage();
        state.poll_telemetry();

        let target_anim = if state.active_drawer.is_some() { 1.0 } else { 0.0 };
        state.drawer_anim += (target_anim - state.drawer_anim) * 0.24;

        let palette = ThemePalette::from_theme(state.theme);
        let tr = state.tr();

        // 1. Fond principal
        let mut bg = Path::new();
        bg.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&bg, &Paint::color(palette.canvas_bg));

        // =====================================================================
        // ESPACE MONDE ZOOMABLE & DÉPLAÇABLE (Châssis, Rails, Modules, Câbles)
        // =====================================================================
        canvas.save();
        canvas.translate(state.pan_x, state.pan_y);
        canvas.scale(state.zoom, state.zoom);

        let mut display_rows = state.num_rows();
        if let Some(d_drag) = &state.drawer_drag {
            display_rows = display_rows.max(d_drag.target_row + 1);
        }
        if let Some(m_drag) = &state.module_drag {
            display_rows = display_rows.max(m_drag.target_row + 1);
        }

        let cabinet_w = state.max_row_width().max((bounds.w - 60.0) / state.zoom);
        let cabinet_h = (display_rows as f32) * ROW_STRIDE + 16.0;

        let mut cab = Path::new();
        cab.rounded_rect(8.0, RACK_TOP_Y - 18.0, cabinet_w, cabinet_h, 8.0);
        canvas.fill_path(&cab, &Paint::color(palette.cabinet_bg));

        if let Some(grain_color) = palette.wood_grain {
            let mut g_paint = Paint::color(grain_color);
            g_paint.set_line_width(1.6);
            let mut wy = RACK_TOP_Y - 10.0;
            while wy < RACK_TOP_Y - 18.0 + cabinet_h - 6.0 {
                let mut w_line = Path::new();
                w_line.move_to(12.0, wy);
                w_line.bezier_to(
                    12.0 + cabinet_w * 0.33,
                    wy + (wy * 0.07).sin() * 4.5,
                    12.0 + cabinet_w * 0.66,
                    wy - (wy * 0.05).cos() * 4.5,
                    8.0 + cabinet_w - 4.0,
                    wy,
                );
                canvas.stroke_path(&w_line, &g_paint);
                wy += 7.0;
            }
        }

        for r in 0..display_rows {
            let ry_top = ModuleVisual::row_top_y(r) - 10.0;
            let ry_bot = ModuleVisual::row_top_y(r) + MODULE_HEIGHT + 2.0;
            for rail_y in [ry_top, ry_bot] {
                let mut rail = Path::new();
                rail.rect(12.0, rail_y, cabinet_w - 8.0, 8.0);
                canvas.fill_path(&rail, &Paint::color(palette.rail_bg));

                let mut hl = Path::new();
                hl.move_to(12.0, rail_y + 1.0);
                hl.line_to(12.0 + cabinet_w - 8.0, rail_y + 1.0);
                let mut hl_paint = Paint::color(palette.rail_highlight);
                hl_paint.set_line_width(1.0);
                canvas.stroke_path(&hl, &hl_paint);
            }
        }

        for m in &state.modules {
            self.draw_module(canvas, m, state, &palette);
        }

        let insertion_guide = if let Some(d_drag) = &state.drawer_drag {
            let panel_x = Self::drawer_panel_x(bounds.w, state.drawer_anim);
            if d_drag.screen_cursor.x < panel_x {
                Some((d_drag.target_row, d_drag.insert_x, d_drag.kind.accent_color()))
            } else {
                None
            }
        } else {
            state.module_drag.as_ref().and_then(|m_drag| {
                if !m_drag.over_trash {
                    Some((m_drag.target_row, m_drag.insert_x, (64, 196, 255)))
                } else {
                    None
                }
            })
        };

        if let Some((row, ix, (ar, ag, ab))) = insertion_guide {
            let gy = ModuleVisual::row_top_y(row) - 6.0;
            let mut guide = Path::new();
            guide.rounded_rect(ix - 3.0, gy, 6.0, MODULE_HEIGHT + 12.0, 3.0);
            canvas.fill_path(&guide, &Paint::color(Color::rgb(ar, ag, ab)));
        }

        let alpha = (self.params.cable_alpha.value() * 255.0).round() as u8;
        let dt = 1.0 / 60.0;

        let mut endpoints = Vec::with_capacity(state.cables.len());
        for c in &state.cables {
            let p0 = state
                .modules
                .iter()
                .find(|m| m.slot_id == c.logical.src_slot)
                .and_then(|m| m.port_world_pos(PortDirection::Output, c.logical.src_port))
                .unwrap_or_default();
            let p1 = state
                .modules
                .iter()
                .find(|m| m.slot_id == c.logical.dst_slot)
                .and_then(|m| m.port_world_pos(PortDirection::Input, c.logical.dst_port))
                .unwrap_or_default();
            endpoints.push((p0, p1));
        }

        for (c, (p0, p1)) in state.cables.iter_mut().zip(endpoints.into_iter()) {
            c.physics.step(p0, p1, dt);
            Self::draw_cable(canvas, &c.physics, alpha);
        }

        let anchor_pos = state.cable_drag.as_ref().and_then(|drag| {
            state
                .modules
                .iter()
                .find(|m| m.slot_id == drag.from_slot)
                .and_then(|m| m.port_world_pos(drag.from_dir, drag.from_port))
        });

        if let (Some(drag), Some(p0)) = (state.cable_drag.as_mut(), anchor_pos) {
            drag.physics.step(p0, drag.world_mouse, dt);
            Self::draw_cable(canvas, &drag.physics, 235);
        }

        canvas.restore();
        // =====================================================================

        // 2. Barre supérieure (Zoom + Fullscreen + Settings)
        self.draw_top_bar_and_settings(canvas, state, &palette, bounds.w);

        // 3. Drawers latéraux droits (MODULES DSP/FX & PRESETS)
        self.draw_right_drawers(canvas, state, &palette, bounds.w, bounds.h);

        // 4. Fantôme translucide lors du Drag & Drop depuis le Drawer Modules
        if let Some(d_drag) = &state.drawer_drag {
            let gw = d_drag.kind.width() * 0.75;
            let gh = 110.0;
            let gx = d_drag.screen_cursor.x - gw * 0.5;
            let gy = d_drag.screen_cursor.y - 24.0;
            let (ar, ag, ab) = d_drag.kind.accent_color();
            let (g_name, _, _) = tr.module_info(state.language, d_drag.kind);

            let mut ghost = Path::new();
            ghost.rounded_rect(gx, gy, gw, gh, 6.0);
            canvas.fill_path(&ghost, &Paint::color(Color::rgba(32, 36, 44, 210)));

            let mut g_border = Paint::color(Color::rgb(ar, ag, ab));
            g_border.set_line_width(2.0);
            canvas.stroke_path(&ghost, &g_border);

            let g_title = self.text_paint(
                Color::rgb(255, 255, 255),
                13.0,
                true,
                Align::Center,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(gx + gw * 0.5, gy + 26.0, g_name, &g_title);
        }

        // 5. Zone Corbeille rouge lors du déplacement d'un module
        if let Some(m_drag) = &state.module_drag {
            let (tx, ty, tw, th) = Self::trash_zone_rect(bounds.w, bounds.h);
            let mut trash = Path::new();
            trash.rounded_rect(tx, ty, tw, th, 8.0);
            canvas.fill_path(
                &trash,
                &Paint::color(if m_drag.over_trash {
                    Color::rgba(220, 53, 69, 235)
                } else {
                    Color::rgba(140, 35, 45, 195)
                }),
            );

            let mut t_border = Paint::color(Color::rgb(255, 110, 120));
            t_border.set_line_width(1.6);
            canvas.stroke_path(&trash, &t_border);

            let t_label = self.text_paint(
                Color::rgb(255, 255, 255),
                11.0,
                true,
                Align::Center,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(tx + tw * 0.5, ty + th * 0.5, tr.trash_drop_label, &t_label);
        }

        // 6. Menu contextuel clic droit
        if let Some(menu) = &state.context_menu {
            let total_items = ModuleKind::ALL.len() + 1;
            let total_h = total_items as f32 * MENU_ITEM_H;
            let mut box_path = Path::new();
            box_path.rounded_rect(menu.screen_x, menu.screen_y, MENU_WIDTH, total_h, 5.0);
            canvas.fill_path(&box_path, &Paint::color(palette.drawer_bg));

            let mut border = Paint::color(palette.card_border);
            border.set_line_width(1.2);
            canvas.stroke_path(&box_path, &border);

            for (idx, kind) in ModuleKind::ALL.iter().enumerate() {
                let iy = menu.screen_y + idx as f32 * MENU_ITEM_H;
                let (m_title, _, _) = tr.module_info(state.language, *kind);
                let label = format!("+ {}", m_title);
                let txt = self.text_paint(
                    palette.ui_text,
                    11.2,
                    false,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(menu.screen_x + 12.0, iy + MENU_ITEM_H * 0.5, &label, &txt);
            }

            let del_y = menu.screen_y + ModuleKind::ALL.len() as f32 * MENU_ITEM_H;
            let del_txt = self.text_paint(
                if menu.target_slot.is_some() {
                    Color::rgb(255, 105, 115)
                } else {
                    palette.ui_subtext
                },
                11.2,
                true,
                Align::Left,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(
                menu.screen_x + 12.0,
                del_y + MENU_ITEM_H * 0.5,
                tr.ctx_delete_module,
                &del_txt,
            );
        }

        // 7. Poignée visible de redimensionnement natif Windows en bas à droite
        if !win32_window::is_fullscreen() {
            let mut grip_paint = Paint::color(palette.ui_subtext);
            grip_paint.set_line_width(1.5);
            for offset in [6.0, 11.0, 16.0] {
                let mut line = Path::new();
                line.move_to(bounds.w - offset, bounds.h - 3.0);
                line.line_to(bounds.w - 3.0, bounds.h - offset);
                canvas.stroke_path(&line, &grip_paint);
            }
        }

        // 8. Toast d'explication au survol d'un port
        self.draw_port_hover_toast(canvas, state, &palette, bounds.w, bounds.h);

        // 9. Fenêtre modale de confirmation d'écrasement de preset
        if let Some(preset_idx) = state.pending_preset {
            self.draw_overwrite_modal(canvas, state, preset_idx, &palette, bounds.w, bounds.h);
        }
    }
}