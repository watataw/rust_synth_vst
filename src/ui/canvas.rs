use super::layout::{MENU_WIDTH, RESIZE_GRIP_SIZE};
use super::presets::SYNTH_PRESETS;
use super::state::{
    ActiveCableDrag, ActiveDrawerDrag, ActiveKnobDrag, ActiveModuleDrag, ActivePanDrag,
    ContextMenuState, ModuleKind, ModuleVisual, PortDirection, RackTheme, RackUiState,
    RightDrawerTab, MODULE_HEIGHT, RACK_LEFT_MARGIN, RACK_TOP_Y, ROW_STRIDE,
};
use super::theme::ThemePalette;
use super::win32_window;
use crate::engine::LogicalCable;
use crate::i18n::Language;
use crate::physics::{CablePhysics, Vec2};
use crate::ModularParams;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::{Align, Baseline, Color, FontId, Paint, Path};
use nih_plug_vizia::ViziaState;
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIN_LOGICAL_W: u32 = 560;
const MIN_LOGICAL_H: u32 = 380;
const MAX_LOGICAL_W: u32 = 3840;
const MAX_LOGICAL_H: u32 = 2160;

#[derive(Lens)]
struct CanvasWindowSizeModel {
    width: Units,
    height: Units,
}

enum CanvasSizeEvent {
    SetLogicalSize(u32, u32),
}

impl Model for CanvasWindowSizeModel {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|msg, _| match msg {
            CanvasSizeEvent::SetLogicalSize(w, h) => {
                self.width = Units::Pixels(*w as f32);
                self.height = Units::Pixels(*h as f32);
            }
        });
    }
}

#[derive(Clone, Copy)]
struct PreFullscreenState {
    logical_w: u32,
    logical_h: u32,
    root_left: i32,
    root_top: i32,
}

pub struct RackCanvas {
    state: Arc<Mutex<RackUiState>>,
    params: Arc<ModularParams>,
    editor_state: Arc<ViziaState>,
    parent_hwnd: Arc<AtomicIsize>,
    pre_fullscreen: Cell<Option<PreFullscreenState>>,
    font_regular: Cell<Option<FontId>>,
    font_bold: Cell<Option<FontId>>,
    last_size: Cell<(f32, f32)>,
    window_resize_drag: Cell<Option<(f32, f32, f32, f32)>>,
    last_sent_logical_size: Cell<(u32, u32)>,
    last_resize_request: Cell<Instant>,
}

impl RackCanvas {
    pub fn new(
        cx: &mut Context,
        state: Arc<Mutex<RackUiState>>,
        params: Arc<ModularParams>,
        editor_state: Arc<ViziaState>,
        parent_hwnd: Arc<AtomicIsize>,
    ) -> Handle<'_, Self> {
        let timer = cx.add_timer(Duration::from_millis(16), None, |cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                cx.needs_redraw();
            }
        });
        cx.start_timer(timer);

        let initial_size = editor_state.inner_logical_size();

        CanvasWindowSizeModel {
            width: Units::Pixels(initial_size.0 as f32),
            height: Units::Pixels(initial_size.1 as f32),
        }
            .build(cx);

        Self {
            state,
            params,
            editor_state,
            parent_hwnd,
            pre_fullscreen: Cell::new(None),
            font_regular: Cell::new(None),
            font_bold: Cell::new(None),
            last_size: Cell::new((initial_size.0 as f32, initial_size.1 as f32)),
            window_resize_drag: Cell::new(None),
            last_sent_logical_size: Cell::new(initial_size),
            last_resize_request: Cell::new(Instant::now() - Duration::from_secs(1)),
        }
            .build(cx, |_| {})
            .width(CanvasWindowSizeModel::width)
            .height(CanvasWindowSizeModel::height)
    }

    pub(super) fn is_fullscreen(&self) -> bool {
        self.pre_fullscreen.get().is_some()
    }

    fn current_logical_size(&self, cx_scale: f32) -> (u32, u32) {
        let state_size = self.editor_state.inner_logical_size();
        if state_size.0 >= MIN_LOGICAL_W && state_size.1 >= MIN_LOGICAL_H {
            state_size
        } else {
            let scale = cx_scale.max(1.0);
            let (phys_w, phys_h) = self.last_size.get();
            (
                (phys_w / scale).round().max(MIN_LOGICAL_W as f32) as u32,
                (phys_h / scale).round().max(MIN_LOGICAL_H as f32) as u32,
            )
        }
    }

    fn set_canvas_logical_size(&self, cx: &mut EventContext, new_w: u32, new_h: u32) {
        let clamped_w = new_w.clamp(MIN_LOGICAL_W, MAX_LOGICAL_W);
        let clamped_h = new_h.clamp(MIN_LOGICAL_H, MAX_LOGICAL_H);
        cx.emit(CanvasSizeEvent::SetLogicalSize(clamped_w, clamped_h));
        cx.needs_relayout();
    }

    fn request_window_resize(&self, cx: &mut EventContext, new_w: u32, new_h: u32, force: bool) {
        let clamped = (
            new_w.clamp(MIN_LOGICAL_W, MAX_LOGICAL_W),
            new_h.clamp(MIN_LOGICAL_H, MAX_LOGICAL_H),
        );
        if !force && self.last_resize_request.get().elapsed() < Duration::from_millis(16) {
            return;
        }
        if !force && self.last_sent_logical_size.get() == clamped {
            return;
        }

        self.last_sent_logical_size.set(clamped);
        self.last_resize_request.set(Instant::now());

        self.set_canvas_logical_size(cx, clamped.0, clamped.1);
        cx.emit(WindowEvent::SetSize(WindowSize {
            width: clamped.0,
            height: clamped.1,
        }));
    }

    fn toggle_fullscreen(&self, cx: &mut EventContext) {
        if let Some(prev) = self.pre_fullscreen.take() {
            self.request_window_resize(cx, prev.logical_w, prev.logical_h, true);
            win32_window::restore_root_window_pos(
                self.parent_hwnd.load(Ordering::Acquire),
                prev.root_left,
                prev.root_top,
            );
        } else {
            let (cur_w, cur_h) = self.current_logical_size(cx.scale_factor());
            let hwnd = self.parent_hwnd.load(Ordering::Acquire);
            if let Some(target) = win32_window::enter_fullscreen_on_monitor(hwnd) {
                self.pre_fullscreen.set(Some(PreFullscreenState {
                    logical_w: cur_w,
                    logical_h: cur_h,
                    root_left: target.prev_root_left,
                    root_top: target.prev_root_top,
                }));

                let scale = cx.scale_factor().max(1.0);
                let logical_w = ((target.avail_phys_w as f32 / scale).floor() as u32)
                    .clamp(MIN_LOGICAL_W, MAX_LOGICAL_W);
                let logical_h = ((target.avail_phys_h as f32 / scale).floor() as u32)
                    .clamp(MIN_LOGICAL_H, MAX_LOGICAL_H);
                self.request_window_resize(cx, logical_w, logical_h, true);
            }
        }
        self.state.lock().unwrap().auto_fit = true;
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

    pub(super) fn text_paint(
        &self,
        color: Color,
        size: f32,
        bold: bool,
        align: Align,
        baseline: Baseline,
    ) -> Paint {
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
}

impl View for RackCanvas {
    fn element(&self) -> Option<&'static str> {
        Some("rack-canvas")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _| match window_event {
            WindowEvent::GeometryChanged(_) => {
                if self.window_resize_drag.get().is_none() {
                    let hwnd = self.parent_hwnd.load(Ordering::Acquire);
                    if let Some((phys_w, phys_h)) = win32_window::parent_client_size(hwnd) {
                        let scale = cx.scale_factor().max(1.0);
                        let new_w = ((phys_w as f32 / scale).round() as u32)
                            .clamp(MIN_LOGICAL_W, MAX_LOGICAL_W);
                        let new_h = ((phys_h as f32 / scale).round() as u32)
                            .clamp(MIN_LOGICAL_H, MAX_LOGICAL_H);
                        if self.last_sent_logical_size.get() != (new_w, new_h) {
                            self.last_sent_logical_size.set((new_w, new_h));
                            self.set_canvas_logical_size(cx, new_w, new_h);
                        }
                    }
                }
            }

            WindowEvent::FocusOut => {
                if self.window_resize_drag.get().is_none() {
                    cx.release();
                }
                let mut state = self.state.lock().unwrap();
                state.cable_drag = None;
                state.module_drag = None;
                state.drawer_drag = None;
                state.knob_drag = None;
                state.pan_drag = None;
            }

            WindowEvent::KeyDown(code, _) => {
                if *code == Code::Escape && self.is_fullscreen() {
                    self.toggle_fullscreen(cx);
                    cx.needs_redraw();
                }
            }

            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, _) = self.last_size.get();
                let mut state = self.state.lock().unwrap();
                let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                let tab_w = Self::effective_drawer_tab_w(win_w);
                if my > 44.0
                    && mx < panel_x - tab_w
                    && Self::hit_test_port(&state, mx, my).is_none()
                    && !Self::is_over_knob(&state, mx, my)
                {
                    state.auto_fit = true;
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

                let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                if state.active_drawer.is_some() && mx >= panel_x {
                    if state.active_drawer == Some(RightDrawerTab::Modules) {
                        let (_, _, _, total_h) = Self::layout_module_drawer_entries(0.0);
                        let max_scroll = (total_h - win_h).max(0.0);
                        state.drawer_scroll =
                            (state.drawer_scroll - *dy * 36.0).clamp(0.0, max_scroll);
                        cx.needs_redraw();
                    }
                    return;
                }

                state.auto_fit = false;

                if modifiers.contains(Modifiers::CTRL) {
                    let old_zoom = state.zoom;
                    let factor = if *dy > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    let new_zoom = (old_zoom * factor).clamp(0.15, 2.2);

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

                let tab_w = Self::effective_drawer_tab_w(win_w);
                let avail_w = (panel_x - tab_w).max(120.0);
                let rows = state.num_rows();
                state.clamp_pan(avail_w, win_h, rows);

                cx.needs_redraw();
            }

            WindowEvent::MouseDown(button) => {
                cx.capture();

                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, win_h) = self.last_size.get();
                let scale = cx.scale_factor().max(1.0);
                let grip_size = RESIZE_GRIP_SIZE * scale;
                let modifiers = cx.modifiers();

                let mut toggle_full_requested = false;

                {
                    let mut state_guard = self.state.lock().unwrap();
                    let state = &mut *state_guard;

                    if let Some(preset_idx) = state.pending_preset {
                        if *button == MouseButton::Left {
                            let (_, (cx_b, cy_b, cw_b, ch_b), (ox_b, oy_b, ow_b, oh_b)) =
                                Self::confirm_modal_rects(win_w, win_h);

                            if mx >= cx_b && mx <= cx_b + cw_b && my >= cy_b && my <= cy_b + ch_b {
                                state.pending_preset = None;
                            } else if mx >= ox_b
                                && mx <= ox_b + ow_b
                                && my >= oy_b
                                && my <= oy_b + oh_b
                            {
                                state.pending_preset = None;
                                state.apply_preset(preset_idx);
                            }
                            cx.needs_redraw();
                        }
                        return;
                    }

                    if *button == MouseButton::Left
                        && !self.is_fullscreen()
                        && mx >= win_w - grip_size
                        && my >= win_h - grip_size
                    {
                        self.window_resize_drag.set(Some((mx, my, win_w, win_h)));
                        return;
                    }

                    if *button == MouseButton::Middle {
                        state.auto_fit = false;
                        state.pan_drag = Some(ActivePanDrag {
                            last_screen: Vec2::new(mx, my),
                        });
                        return;
                    }

                    if let Some(menu) = state.context_menu.take() {
                        let item_h = Self::context_menu_item_height(win_h);
                        if *button == MouseButton::Left
                            && mx >= menu.screen_x
                            && mx <= menu.screen_x + MENU_WIDTH
                            && my >= menu.screen_y
                        {
                            let item_idx = ((my - menu.screen_y) / item_h).floor() as usize;
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

                    let ((ax, ay, aw, ah), (fx, fy, fw, fh), (sx, sy, sw, sh), _) =
                        Self::top_bar_rects(win_w, state.drawer_anim);

                    if *button == MouseButton::Left
                        && mx >= ax
                        && mx <= ax + aw
                        && my >= ay
                        && my <= ay + ah
                    {
                        state.auto_fit = true;
                        cx.needs_redraw();
                        return;
                    }

                    if *button == MouseButton::Left
                        && mx >= fx
                        && mx <= fx + fw
                        && my >= fy
                        && my <= fy + fh
                    {
                        toggle_full_requested = true;
                    } else {
                        if *button == MouseButton::Left
                            && mx >= sx
                            && mx <= sx + sw
                            && my >= sy
                            && my <= sy + sh
                        {
                            state.settings_open = !state.settings_open;
                            cx.needs_redraw();
                            return;
                        }

                        if state.settings_open && *button == MouseButton::Left {
                            let (px, py, pw, ph) =
                                Self::settings_popover_rect(win_w, win_h, state.drawer_anim);
                            if mx >= px && mx <= px + pw && my >= py && my <= py + ph {
                                let lang_w = (pw - 32.0) * 0.5;
                                for (i, lang_opt) in [Language::En, Language::Fr].iter().enumerate()
                                {
                                    let bx = px + 12.0 + i as f32 * (lang_w + 8.0);
                                    let by = py + 27.0;
                                    if mx >= bx
                                        && mx <= bx + lang_w
                                        && my >= by
                                        && my <= by + 26.0
                                    {
                                        state.language = *lang_opt;
                                        cx.needs_redraw();
                                        return;
                                    }
                                }

                                for (idx, &theme_opt) in RackTheme::ALL.iter().enumerate() {
                                    let item_y = py + 81.0 + idx as f32 * 32.0;
                                    if my >= item_y && my <= item_y + 27.0 {
                                        state.theme = theme_opt;
                                        cx.needs_redraw();
                                        return;
                                    }
                                }

                                let delays: [u64; 4] = [200, 500, 1000, 1500];
                                let pill_w = (pw - 36.0) / 4.0;
                                for (i, &ms) in delays.iter().enumerate() {
                                    let bx = px + 12.0 + i as f32 * (pill_w + 4.0);
                                    let by = py + 236.0;
                                    if mx >= bx
                                        && mx <= bx + pill_w
                                        && my >= by
                                        && my <= by + 26.0
                                    {
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

                        let drawer_w = Self::effective_drawer_width(win_w);
                        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                        if *button == MouseButton::Left
                            && state.active_drawer.is_some()
                            && mx >= panel_x
                        {
                            match state.active_drawer.unwrap() {
                                RightDrawerTab::Modules => {
                                    let (_, entries, _, _) =
                                        Self::layout_module_drawer_entries(state.drawer_scroll);
                                    let card_x = panel_x + 12.0;
                                    let card_w = (drawer_w - 24.0).max(120.0);
                                    for entry in entries {
                                        if my >= 50.0
                                            && mx >= card_x
                                            && mx <= card_x + card_w
                                            && my >= entry.y
                                            && my <= entry.y + entry.h
                                        {
                                            let world_pos = state.screen_to_world(mx, my);
                                            let (target_row, insert_x) =
                                                Self::compute_target_row_and_x(
                                                    state, world_pos, None,
                                                );
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
                                            Self::preset_card_rect(panel_x, drawer_w, win_h, idx);
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

                            let item_h = Self::context_menu_item_height(win_h);
                            let total_h = (ModuleKind::ALL.len() + 1) as f32 * item_h;
                            let clamped_x = mx.clamp(8.0, (win_w - MENU_WIDTH - 8.0).max(8.0));
                            let clamped_y = my.clamp(8.0, (win_h - total_h - 8.0).max(8.0));

                            state.context_menu = Some(ContextMenuState {
                                screen_x: clamped_x,
                                screen_y: clamped_y,
                                target_slot,
                            });
                            cx.needs_redraw();
                            return;
                        }

                        if *button == MouseButton::Left {
                            if let Some((slot_id, dir, port_idx, port_pos)) =
                                Self::hit_test_port(state, mx, my)
                            {
                                if let Some((removed, old_color)) =
                                    state.disconnect_cable_at_port(slot_id, dir, port_idx)
                                {
                                    let (anchor_slot, anchor_port, anchor_dir, anchor_pos) =
                                        match dir {
                                            PortDirection::Input => {
                                                let p = state
                                                    .modules
                                                    .iter()
                                                    .find(|m| m.slot_id == removed.src_slot)
                                                    .and_then(|m| {
                                                        m.port_world_pos(
                                                            PortDirection::Output,
                                                            removed.src_port,
                                                        )
                                                    })
                                                    .unwrap_or(port_pos);
                                                (
                                                    removed.src_slot,
                                                    removed.src_port,
                                                    PortDirection::Output,
                                                    p,
                                                )
                                            }
                                            PortDirection::Output => {
                                                let p = state
                                                    .modules
                                                    .iter()
                                                    .find(|m| m.slot_id == removed.dst_slot)
                                                    .and_then(|m| {
                                                        m.port_world_pos(
                                                            PortDirection::Input,
                                                            removed.dst_port,
                                                        )
                                                    })
                                                    .unwrap_or(port_pos);
                                                (
                                                    removed.dst_slot,
                                                    removed.dst_port,
                                                    PortDirection::Input,
                                                    p,
                                                )
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
                }

                if toggle_full_requested {
                    self.toggle_fullscreen(cx);
                    cx.needs_redraw();
                }
            }

            WindowEvent::MouseMove(mx, my) => {
                if let Some((start_mx, start_my, start_phys_w, start_phys_h)) =
                    self.window_resize_drag.get()
                {
                    let scale = cx.scale_factor().max(1.0);
                    let new_w = ((start_phys_w + (*mx - start_mx)) / scale)
                        .round()
                        .clamp(MIN_LOGICAL_W as f32, MAX_LOGICAL_W as f32)
                        as u32;
                    let new_h = ((start_phys_h + (*my - start_my)) / scale)
                        .round()
                        .clamp(MIN_LOGICAL_H as f32, MAX_LOGICAL_H as f32)
                        as u32;
                    self.request_window_resize(cx, new_w, new_h, false);
                    return;
                }

                let (win_w, win_h) = self.last_size.get();
                let mut state_guard = self.state.lock().unwrap();
                let state = &mut *state_guard;

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
                    let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                    let tab_w = Self::effective_drawer_tab_w(win_w);
                    let avail_w = (panel_x - tab_w).max(120.0);
                    let rows = state.num_rows();
                    state.clamp_pan(avail_w, win_h, rows);
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
                cx.release();

                if *button == MouseButton::Left {
                    if let Some((start_mx, start_my, start_phys_w, start_phys_h)) =
                        self.window_resize_drag.take()
                    {
                        let mx = cx.mouse().cursorx;
                        let my = cx.mouse().cursory;
                        let scale = cx.scale_factor().max(1.0);
                        let new_w = ((start_phys_w + (mx - start_mx)) / scale)
                            .round()
                            .clamp(MIN_LOGICAL_W as f32, MAX_LOGICAL_W as f32)
                            as u32;
                        let new_h = ((start_phys_h + (my - start_my)) / scale)
                            .round()
                            .clamp(MIN_LOGICAL_H as f32, MAX_LOGICAL_H as f32)
                            as u32;
                        self.request_window_resize(cx, new_w, new_h, true);
                        cx.needs_redraw();
                        return;
                    }
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

                    if let Some(m_drag) = state.module_drag.take() {
                        if m_drag.over_trash {
                            state.remove_module(m_drag.slot_id);
                        } else {
                            if let Some(m) =
                                state.modules.iter_mut().find(|m| m.slot_id == m_drag.slot_id)
                            {
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
        if bounds.w < 10.0 || bounds.h < 10.0 {
            return;
        }
        self.last_size.set((bounds.w, bounds.h));

        let mut state_guard = self.state.lock().unwrap();
        let state = &mut *state_guard;
        state.collect_garbage();
        state.poll_telemetry();

        let target_anim = if state.active_drawer.is_some() { 1.0 } else { 0.0 };
        state.drawer_anim += (target_anim - state.drawer_anim) * 0.24;
        if (target_anim - state.drawer_anim).abs() < 0.002 {
            state.drawer_anim = target_anim;
        }

        let mut display_rows = state.num_rows();
        if let Some(d_drag) = &state.drawer_drag {
            display_rows = display_rows.max(d_drag.target_row + 1);
        }
        if let Some(m_drag) = &state.module_drag {
            display_rows = display_rows.max(m_drag.target_row + 1);
        }

        let panel_x = Self::drawer_panel_x(bounds.w, state.drawer_anim);
        let tab_w = Self::effective_drawer_tab_w(bounds.w);
        let avail_w = (panel_x - tab_w).max(120.0);
        if state.auto_fit && state.module_drag.is_none() {
            state.fit_to_viewport(avail_w, bounds.h, display_rows);
        }

        let palette = ThemePalette::from_theme(state.theme);

        let mut bg = Path::new();
        bg.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&bg, &Paint::color(palette.canvas_bg));

        canvas.save();
        canvas.translate(state.pan_x, state.pan_y);
        canvas.scale(state.zoom, state.zoom);

        let cabinet_w = state
            .max_row_width()
            .max(((avail_w - 24.0) / state.zoom).max(320.0));
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

        self.draw_top_bar_and_settings(canvas, state, &palette, bounds.w, bounds.h);
        self.draw_right_drawers(canvas, state, &palette, bounds.w, bounds.h);
        self.draw_overlays(canvas, state, &palette, bounds.w, bounds.h);
    }
}