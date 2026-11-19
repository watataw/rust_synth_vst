use crate::engine::LogicalCable;
use crate::physics::{CablePhysics, Vec2, CABLE_NODES};
use crate::ui::state::{
    ActiveCableDrag, ActiveDrawerDrag, ActiveKnobDrag, ActiveModuleDrag, ContextMenuState,
    ModuleKind, ModuleVisual, PortDirection, RackUiState, MODULE_HEIGHT, RACK_LEFT_MARGIN,
    RACK_TOP_Y,
};
use crate::ModularParams;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::{
    Align, Baseline, Color, FontId, LineCap, LineJoin, Paint, Path, Solidity,
};
use std::cell::Cell;
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const JACK_HIT_RADIUS: f32 = 14.0;
const MENU_WIDTH: f32 = 185.0;
const MENU_ITEM_H: f32 = 28.0;
const DRAWER_WIDTH: f32 = 248.0;
const DRAWER_TAB_W: f32 = 30.0;
const DRAWER_TAB_H: f32 = 128.0;

pub struct RackCanvas {
    state: Arc<Mutex<RackUiState>>,
    params: Arc<ModularParams>,
    font_regular: Cell<Option<FontId>>,
    font_bold: Cell<Option<FontId>>,
    last_size: Cell<(f32, f32)>,
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
            last_size: Cell::new((1320.0, 560.0)),
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

    fn drawer_tab_rect(win_w: f32, win_h: f32, drawer_anim: f32) -> (f32, f32, f32, f32) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        let tab_x = panel_x - DRAWER_TAB_W;
        let tab_y = (win_h - DRAWER_TAB_H) * 0.5;
        (tab_x, tab_y, DRAWER_TAB_W, DRAWER_TAB_H)
    }

    fn drawer_card_rect(panel_x: f32, index: usize) -> (f32, f32, f32, f32) {
        let card_x = panel_x + 12.0;
        let card_y = 54.0 + index as f32 * 64.0;
        let card_w = DRAWER_WIDTH - 24.0;
        let card_h = 56.0;
        (card_x, card_y, card_w, card_h)
    }

    fn trash_zone_rect(win_w: f32, win_h: f32) -> (f32, f32, f32, f32) {
        (win_w - 220.0, win_h - 68.0, 200.0, 52.0)
    }

    fn hit_test_port(state: &RackUiState, mx: f32, my: f32) -> Option<(usize, PortDirection, usize, Vec2)> {
        let cursor = Vec2::new(mx, my);
        for m in &state.modules {
            let draw_x = m.x - state.scroll_x;
            for p in &m.ports {
                let pos = Vec2::new(draw_x + p.local_x, RACK_TOP_Y + p.local_y);
                if pos.distance(cursor) <= JACK_HIT_RADIUS {
                    return Some((m.slot_id, p.direction, p.index, pos));
                }
            }
        }
        None
    }

    fn compute_insertion_x(state: &RackUiState, cursor_world_x: f32) -> f32 {
        for m in &state.modules {
            let mid = m.x + m.kind.width() * 0.5;
            if cursor_world_x < mid {
                return m.x - 2.0;
            }
        }
        state
            .modules
            .last()
            .map(|m| m.x + m.kind.width() + 2.0)
            .unwrap_or(RACK_LEFT_MARGIN)
    }

    fn midi_note_name(v_oct: f32) -> String {
        let midi = (v_oct * 12.0 + 60.0).round() as i32;
        let clamped = midi.clamp(0, 127);
        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let note = names[(clamped % 12) as usize];
        let octave = (clamped / 12) - 1;
        format!("{}{}", note, octave)
    }

    /// Dessine l'écran OLED interactif propre à chaque type de module.
    fn draw_module_visualizer(
        &self,
        canvas: &mut Canvas,
        m: &ModuleVisual,
        state: &RackUiState,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let sx = x + 14.0;
        let sy = y + 48.0;
        let sw = w - 28.0;
        let sh = 74.0;

        // Cadre d'écran OLED encastré
        let mut screen = Path::new();
        screen.rounded_rect(sx, sy, sw, sh, 4.0);
        canvas.fill_path(&screen, &Paint::color(Color::rgb(12, 15, 20)));

        let mut bezel = Paint::color(Color::rgb(68, 75, 86));
        bezel.set_line_width(1.0);
        canvas.stroke_path(&screen, &bezel);

        // Grille subtile de fond
        let mut grid_paint = Paint::color(Color::rgba(255, 255, 255, 14));
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

                // Nom de la note en gros
                let note_paint = self.text_paint(
                    if gate_on { accent } else { Color::rgb(95, 105, 120) },
                    24.0,
                    true,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(sx + 12.0, sy + 28.0, &note_str, &note_paint);

                // Badge GATE
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

                // Barre de vélocité
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

                // Affiche la fréquence de base en Hz
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

                // Nombre de cycles affichés varie légèrement avec le pitch
                let cycles = (1.5 + ((coarse + 24.0) / 48.0) * 2.5).clamp(1.5, 4.0);

                // Courbe d'onde carrée PWM (en arrière-plan) + dent de scie (avant-plan)
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

                // Position logarithmique normalisée 20Hz .. 16kHz
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

                // Repère vertical Cutoff
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

                // Proportions visuelles dynamiques des segments ADSR
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

                // Indicateur lumineux temps réel du niveau d'enveloppe actuel (0..5V)
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

                // Densité d'affichage proportionnelle à la fréquence du LFO
                let cycles = (1.0 + (rate / 40.0).sqrt() * 3.5).clamp(1.0, 4.5);

                let mut wave_path = Path::new();
                let steps = 72;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let t = (u * cycles).fract();
                    let px = sx + 6.0 + u * (sw - 12.0);

                    let val = match wave_idx {
                        0 => (t * TAU).sin(),
                        1 => {
                            if t < 0.5 {
                                4.0 * t - 1.0
                            } else {
                                3.0 - 4.0 * t
                            }
                        }
                        2 => 2.0 * t - 1.0,
                        3 => {
                            if t < 0.5 {
                                1.0
                            } else {
                                -1.0
                            }
                        }
                        _ => {
                            // Aperçu S&H en escalier déterministe
                            let step_id = (u * cycles * 2.0).floor() as i32;
                            (((step_id * 73 + 19) % 11) as f32 / 5.0) - 1.0
                        }
                    };

                    let py = if unipolar {
                        sy + sh - 10.0 - ((val + 1.0) * 0.5) * (sh - 20.0)
                    } else {
                        sy + sh * 0.5 - val * (sh * 0.36)
                    };

                    if i == 0 {
                        wave_path.move_to(px, py);
                    } else {
                        wave_path.line_to(px, py);
                    }
                }

                let mut stroke = Paint::color(accent);
                stroke.set_line_width(2.2);
                canvas.stroke_path(&wave_path, &stroke);

                // Pastille LED live qui suit la tension de sortie instantanée du LFO
                let live_v = state.port_signal_value(m.slot_id, 0);
                let norm_y = if unipolar {
                    (live_v / 5.0).clamp(0.0, 1.0)
                } else {
                    ((live_v + 5.0) / 10.0).clamp(0.0, 1.0)
                };
                let led_y = sy + sh - 10.0 - norm_y * (sh - 20.0);
                let mut live_dot = Path::new();
                live_dot.circle(sx + sw - 10.0, led_y, 4.0);
                canvas.fill_path(&live_dot, &Paint::color(Color::rgb(255, 255, 255)));
            }

            ModuleKind::Vca => {
                let level = m.knobs[0].param.load();
                let exp_blend = m.knobs[2].param.load();

                // Courbe de transfert Linéaire -> Exponentielle
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

                // Barre d'activité de sortie à droite de l'écran
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

                // Onde de sortie stéréo stylisée + barres L/R
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

                // Double barre L / R dans l'écran
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
        }
    }

    fn draw_module(&self, canvas: &mut Canvas, m: &ModuleVisual, state: &RackUiState, draw_x: f32) {
        let y = RACK_TOP_Y;
        let w = m.kind.width();
        let h = MODULE_HEIGHT;
        let (ar, ag, ab) = m.kind.accent_color();

        // 1. Façade aluminium brossé
        let mut panel = Path::new();
        panel.rounded_rect(draw_x, y, w, h, 5.0);
        let bg = Paint::linear_gradient(
            draw_x,
            y,
            draw_x + w,
            y + h,
            Color::rgb(52, 56, 63),
            Color::rgb(35, 38, 44),
        );
        canvas.fill_path(&panel, &bg);

        let mut border = Paint::color(Color::rgb(82, 88, 98));
        border.set_line_width(1.2);
        canvas.stroke_path(&panel, &border);

        // Micro-stries horizontales
        let mut grain_paint = Paint::color(Color::rgba(255, 255, 255, 5));
        grain_paint.set_line_width(1.0);
        let mut gy = y + 8.0;
        while gy < y + h - 8.0 {
            let mut line = Path::new();
            line.move_to(draw_x + 2.0, gy);
            line.line_to(draw_x + w - 2.0, gy);
            canvas.stroke_path(&line, &grain_paint);
            gy += 4.0;
        }

        // 2. Vis Eurorack
        for (sx, sy) in [(10.0, 10.0), (w - 10.0, 10.0), (10.0, h - 10.0), (w - 10.0, h - 10.0)] {
            let mut screw = Path::new();
            screw.circle(draw_x + sx, y + sy, 4.0);
            canvas.fill_path(&screw, &Paint::color(Color::rgb(132, 138, 148)));

            let mut slot = Path::new();
            slot.move_to(draw_x + sx - 2.3, y + sy - 2.3);
            slot.line_to(draw_x + sx + 2.3, y + sy + 2.3);
            let mut slot_paint = Paint::color(Color::rgb(28, 30, 34));
            slot_paint.set_line_width(1.2);
            canvas.stroke_path(&slot, &slot_paint);
        }

        // 3. Bandeau titre clair + barre de couleur par catégorie + sous-titre
        let mut header = Path::new();
        header.rounded_rect(draw_x + 16.0, y + 7.0, w - 32.0, 34.0, 4.0);
        canvas.fill_path(&header, &Paint::color(Color::rgb(20, 22, 27)));

        let mut accent_strip = Path::new();
        accent_strip.rounded_rect(draw_x + 16.0, y + 7.0, 4.5, 34.0, 2.0);
        canvas.fill_path(&accent_strip, &Paint::color(Color::rgb(ar, ag, ab)));

        let title_paint = self.text_paint(
            Color::rgb(245, 248, 252),
            13.0,
            true,
            Align::Center,
            Baseline::Top,
        );
        let _ = canvas.fill_text(draw_x + w * 0.5 + 2.0, y + 10.0, m.kind.title(), &title_paint);

        let sub_paint = self.text_paint(
            Color::rgb(ar, ag, ab),
            8.5,
            true,
            Align::Center,
            Baseline::Top,
        );
        let _ = canvas.fill_text(draw_x + w * 0.5 + 2.0, y + 26.5, m.kind.subtitle(), &sub_paint);

        // 4. Écran visuel temps réel propre au module
        self.draw_module_visualizer(canvas, m, state, draw_x, y, w);

        // 5. Potentiomètres rotatifs avec valeur en temps réel
        let active_knob = state.knob_drag.as_ref().and_then(|kd| {
            if kd.slot_id == m.slot_id {
                Some(kd.knob_idx)
            } else {
                None
            }
        });

        for (k_idx, knob) in m.knobs.iter().enumerate() {
            let kx = draw_x + knob.local_x;
            let ky = y + knob.local_y;
            let val = knob.param.load();
            let norm = ((val - knob.min) / (knob.max - knob.min)).clamp(0.0, 1.0);
            let is_active = active_knob == Some(k_idx);

            let start_angle = 0.75 * PI;
            let end_angle = 2.25 * PI;
            let active_angle = start_angle + norm * (end_angle - start_angle);

            let mut track = Path::new();
            track.arc(kx, ky, knob.radius + 4.5, start_angle, end_angle, Solidity::Hole);
            let mut track_paint = Paint::color(Color::rgb(20, 22, 26));
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
                Color::rgb(48, 52, 60),
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

            // Affiche la valeur dynamique pendant l'ajustement ou le nom au repos
            let label_text = if is_active {
                knob.formatted_value()
            } else {
                knob.label.to_string()
            };
            let lbl_paint = self.text_paint(
                if is_active {
                    Color::rgb(ar, ag, ab)
                } else {
                    Color::rgb(195, 202, 214)
                },
                9.5,
                true,
                Align::Center,
                Baseline::Top,
            );
            let _ = canvas.fill_text(kx, ky + knob.radius + 6.5, &label_text, &lbl_paint);
        }

        // 6. Prises Jack 3.5mm + LEDs d'activité
        for port in &m.ports {
            let px = draw_x + port.local_x;
            let py = y + port.local_y;

            if port.direction == PortDirection::Output {
                let mut out_plate = Path::new();
                out_plate.rounded_rect(px - 19.0, py - 15.0, 38.0, 44.0, 4.0);
                canvas.fill_path(&out_plate, &Paint::color(Color::rgb(18, 20, 25)));
            }

            let mut nut = Path::new();
            nut.circle(px, py, 10.0);
            canvas.fill_path(&nut, &Paint::color(Color::rgb(178, 185, 195)));

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
                    Color::rgb(240, 244, 250)
                } else {
                    Color::rgb(180, 188, 200)
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

    fn draw_drawer(&self, canvas: &mut Canvas, state: &RackUiState, win_w: f32, win_h: f32) {
        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);

        // 1. Onglet latéral pour ouvrir / fermer le drawer
        let (tx, ty, tw, th) = Self::drawer_tab_rect(win_w, win_h, state.drawer_anim);
        let mut tab = Path::new();
        tab.rounded_rect(tx, ty, tw + 4.0, th, 6.0);
        canvas.fill_path(&tab, &Paint::color(Color::rgb(32, 36, 44)));

        let mut tab_border = Paint::color(Color::rgb(78, 86, 98));
        tab_border.set_line_width(1.2);
        canvas.stroke_path(&tab, &tab_border);

        canvas.save();
        canvas.translate(tx + tw * 0.5, ty + th * 0.5);
        canvas.rotate(-PI * 0.5);
        let tab_label = if state.drawer_open { "FERMER ▼" } else { "MODULES ▲" };
        let tab_paint = self.text_paint(
            Color::rgb(230, 236, 245),
            10.5,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(0.0, 0.0, tab_label, &tab_paint);
        canvas.restore();

        if state.drawer_anim <= 0.01 {
            return;
        }

        // 2. Panneau du Drawer
        let mut bg = Path::new();
        bg.rect(panel_x, 0.0, DRAWER_WIDTH, win_h);
        canvas.fill_path(&bg, &Paint::color(Color::rgba(24, 27, 33, 246)));

        let mut sep = Path::new();
        sep.move_to(panel_x, 0.0);
        sep.line_to(panel_x, win_h);
        let mut sep_paint = Paint::color(Color::rgb(75, 83, 96));
        sep_paint.set_line_width(1.5);
        canvas.stroke_path(&sep, &sep_paint);

        let hdr_paint = self.text_paint(
            Color::rgb(245, 248, 252),
            13.0,
            true,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(panel_x + 14.0, 14.0, "BIBLIOTHÈQUE MODULES", &hdr_paint);

        let hint_paint = self.text_paint(
            Color::rgb(145, 155, 170),
            10.0,
            false,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(panel_x + 14.0, 32.0, "Glisser-déposer ou cliquer (+)", &hint_paint);

        // 3. Cartes des 7 modules disponibles
        for (idx, &kind) in ModuleKind::ALL.iter().enumerate() {
            let (cx, cy, cw, ch) = Self::drawer_card_rect(panel_x, idx);
            let (ar, ag, ab) = kind.accent_color();

            let mut card = Path::new();
            card.rounded_rect(cx, cy, cw, ch, 5.0);
            canvas.fill_path(&card, &Paint::color(Color::rgb(35, 39, 47)));

            let mut c_border = Paint::color(Color::rgb(64, 71, 84));
            c_border.set_line_width(1.0);
            canvas.stroke_path(&card, &c_border);

            let mut strip = Path::new();
            strip.rounded_rect(cx, cy, 5.0, ch, 2.0);
            canvas.fill_path(&strip, &Paint::color(Color::rgb(ar, ag, ab)));

            let t_paint = self.text_paint(
                Color::rgb(242, 246, 252),
                12.0,
                true,
                Align::Left,
                Baseline::Top,
            );
            let _ = canvas.fill_text(cx + 13.0, cy + 8.0, kind.title(), &t_paint);

            let badge_paint = self.text_paint(
                Color::rgb(ar, ag, ab),
                9.0,
                true,
                Align::Right,
                Baseline::Top,
            );
            let _ = canvas.fill_text(cx + cw - 10.0, cy + 9.0, kind.subtitle(), &badge_paint);

            let desc_paint = self.text_paint(
                Color::rgb(168, 178, 192),
                10.0,
                false,
                Align::Left,
                Baseline::Top,
            );
            let _ = canvas.fill_text(cx + 13.0, cy + 29.0, kind.description(), &desc_paint);
        }
    }
}

impl View for RackCanvas {
    fn element(&self) -> Option<&'static str> {
        Some("rack-canvas")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _| match window_event {
            WindowEvent::MouseScroll(_, dy) => {
                let mut state = self.state.lock().unwrap();
                let (win_w, _) = self.last_size.get();
                let max_scroll = (state.total_rack_width() - win_w + 120.0).max(0.0);
                state.scroll_x = (state.scroll_x - *dy * 45.0).clamp(0.0, max_scroll);
                for c in &mut state.cables {
                    c.physics.wake();
                }
                cx.needs_redraw();
            }

            WindowEvent::MouseDown(button) => {
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let (win_w, win_h) = self.last_size.get();
                let mut state = self.state.lock().unwrap();

                // 1. Menu contextuel clic droit
                if let Some(menu) = state.context_menu.take() {
                    if *button == MouseButton::Left
                        && mx >= menu.x
                        && mx <= menu.x + MENU_WIDTH
                        && my >= menu.y
                    {
                        let item_idx = ((my - menu.y) / MENU_ITEM_H).floor() as usize;
                        if item_idx < ModuleKind::ALL.len() {
                            state.spawn_module_at(ModuleKind::ALL[item_idx], None);
                        } else if item_idx == ModuleKind::ALL.len() {
                            if let Some(slot) = menu.target_slot {
                                state.remove_module(slot);
                            }
                        }
                        cx.needs_redraw();
                        return;
                    }
                }

                // 2. Clic sur l'onglet du Drawer latéral droit
                let (tx, ty, tw, th) = Self::drawer_tab_rect(win_w, win_h, state.drawer_anim);
                if *button == MouseButton::Left
                    && mx >= tx
                    && mx <= tx + tw
                    && my >= ty
                    && my <= ty + th
                {
                    state.drawer_open = !state.drawer_open;
                    cx.needs_redraw();
                    return;
                }

                // 3. Clic dans le Drawer latéral droit (démarrer Drag & Drop d'un nouveau module)
                let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                if *button == MouseButton::Left && state.drawer_open && mx >= panel_x {
                    for (idx, &kind) in ModuleKind::ALL.iter().enumerate() {
                        let (card_x, card_y, card_w, card_h) = Self::drawer_card_rect(panel_x, idx);
                        if mx >= card_x
                            && mx <= card_x + card_w
                            && my >= card_y
                            && my <= card_y + card_h
                        {
                            let insert_x = Self::compute_insertion_x(&state, mx + state.scroll_x);
                            state.drawer_drag = Some(ActiveDrawerDrag {
                                kind,
                                cursor_pos: Vec2::new(mx, my),
                                insert_x,
                            });
                            cx.needs_redraw();
                            return;
                        }
                    }
                    return;
                }

                // 4. Clic droit sur le rack -> menu rapide
                if *button == MouseButton::Right {
                    let target_slot = state
                        .modules
                        .iter()
                        .find(|m| {
                            let dx = m.x - state.scroll_x;
                            mx >= dx
                                && mx <= dx + m.kind.width()
                                && my >= RACK_TOP_Y
                                && my <= RACK_TOP_Y + MODULE_HEIGHT
                        })
                        .map(|m| m.slot_id);

                    state.context_menu = Some(ContextMenuState {
                        x: mx,
                        y: my,
                        target_slot,
                    });
                    cx.needs_redraw();
                    return;
                }

                if *button == MouseButton::Left {
                    // 5. Clic sur une prise Jack
                    if let Some((slot_id, dir, port_idx, port_pos)) = Self::hit_test_port(&state, mx, my) {
                        if let Some(removed) = state.disconnect_cable_at_port(slot_id, dir, port_idx) {
                            let (anchor_slot, anchor_port, anchor_dir, anchor_pos) = match dir {
                                PortDirection::Input => {
                                    let p = state
                                        .modules
                                        .iter()
                                        .find(|m| m.slot_id == removed.src_slot)
                                        .and_then(|m| {
                                            m.port_world_pos(
                                                PortDirection::Output,
                                                removed.src_port,
                                                state.scroll_x,
                                            )
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
                                            m.port_world_pos(
                                                PortDirection::Input,
                                                removed.dst_port,
                                                state.scroll_x,
                                            )
                                        })
                                        .unwrap_or(port_pos);
                                    (removed.dst_slot, removed.dst_port, PortDirection::Input, p)
                                }
                            };

                            state.cable_drag = Some(ActiveCableDrag {
                                from_slot: anchor_slot,
                                from_port: anchor_port,
                                from_dir: anchor_dir,
                                physics: CablePhysics::new(anchor_pos, Vec2::new(mx, my), 0),
                                mouse_pos: Vec2::new(mx, my),
                            });
                        } else {
                            state.cable_drag = Some(ActiveCableDrag {
                                from_slot: slot_id,
                                from_port: port_idx,
                                from_dir: dir,
                                physics: CablePhysics::new(
                                    port_pos,
                                    Vec2::new(mx, my),
                                    state.cables.len(),
                                ),
                                mouse_pos: Vec2::new(mx, my),
                            });
                        }
                        cx.needs_redraw();
                        return;
                    }

                    // 6. Clic sur un potentiomètre
                    for m in &state.modules {
                        let dx = m.x - state.scroll_x;
                        for (k_idx, knob) in m.knobs.iter().enumerate() {
                            let kx = dx + knob.local_x;
                            let ky = RACK_TOP_Y + knob.local_y;
                            if Vec2::new(kx, ky).distance(Vec2::new(mx, my)) <= knob.radius + 4.0 {
                                state.knob_drag = Some(ActiveKnobDrag {
                                    slot_id: m.slot_id,
                                    knob_idx: k_idx,
                                    start_y: my,
                                    start_val: knob.param.load(),
                                });
                                cx.needs_redraw();
                                return;
                            }
                        }
                    }

                    // 7. Clic sur un module du rack pour le déplacer ou le glisser à la corbeille
                    for m in &state.modules {
                        let dx = m.x - state.scroll_x;
                        if mx >= dx
                            && mx <= dx + m.kind.width()
                            && my >= RACK_TOP_Y
                            && my <= RACK_TOP_Y + MODULE_HEIGHT
                        {
                            state.module_drag = Some(ActiveModuleDrag {
                                slot_id: m.slot_id,
                                grab_offset_x: mx - dx,
                                cursor_pos: Vec2::new(mx, my),
                                over_trash: false,
                            });
                            cx.needs_redraw();
                            return;
                        }
                    }
                }
            }

            WindowEvent::MouseMove(mx, my) => {
                let (win_w, win_h) = self.last_size.get();
                let mut state = self.state.lock().unwrap();

                if let Some(drag) = &mut state.cable_drag {
                    drag.mouse_pos = Vec2::new(*mx, *my);
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
                    let insert_x = Self::compute_insertion_x(&state, *mx + state.scroll_x);
                    if let Some(d_drag) = &mut state.drawer_drag {
                        d_drag.cursor_pos = Vec2::new(*mx, *my);
                        d_drag.insert_x = insert_x;
                    }
                }

                if let Some((slot_id, offset)) = state
                    .module_drag
                    .as_ref()
                    .map(|d| (d.slot_id, d.grab_offset_x))
                {
                    let scroll_x = state.scroll_x;
                    if let Some(m) = state.modules.iter_mut().find(|m| m.slot_id == slot_id) {
                        m.x = (*mx - offset + scroll_x).max(RACK_LEFT_MARGIN);
                    }
                    let (tx, ty, tw, th) = Self::trash_zone_rect(win_w, win_h);
                    let over_trash = *mx >= tx && *mx <= tx + tw && *my >= ty && *my <= ty + th;
                    if let Some(m_drag) = &mut state.module_drag {
                        m_drag.cursor_pos = Vec2::new(*mx, *my);
                        m_drag.over_trash = over_trash;
                    }
                    for c in &mut state.cables {
                        c.physics.wake();
                    }
                }
            }

            WindowEvent::MouseUp(button) => {
                if *button == MouseButton::Left {
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;
                    let (win_w, _) = self.last_size.get();
                    let mut state = self.state.lock().unwrap();

                    // Fin du Drag & Drop depuis le Drawer latéral
                    if let Some(d_drag) = state.drawer_drag.take() {
                        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);
                        // Si relâché sur le rack (ou simple clic sur la carte), on ajoute le module
                        let target_x = if mx < panel_x {
                            Some(d_drag.insert_x)
                        } else {
                            None
                        };
                        state.spawn_module_at(d_drag.kind, target_x);
                    }

                    // Fin du Drag & Drop de câble
                    if let Some(drag) = state.cable_drag.take() {
                        if let Some((target_slot, target_dir, target_port, _)) =
                            Self::hit_test_port(&state, mx, my)
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
                                state.connect_cable(logical);
                            }
                        }
                    }

                    // Fin du Drag & Drop d'un module existant (réorganisation ou suppression)
                    if let Some(m_drag) = state.module_drag.take() {
                        if m_drag.over_trash {
                            state.remove_module(m_drag.slot_id);
                        } else {
                            state.pack_modules_horizontally();
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

        let mut state = self.state.lock().unwrap();
        state.collect_garbage();
        state.poll_telemetry();

        // Animation fluide d'ouverture/fermeture du Drawer droit
        let target_anim = if state.drawer_open { 1.0 } else { 0.0 };
        state.drawer_anim += (target_anim - state.drawer_anim) * 0.24;

        // 1. Fond du châssis Eurorack + Rails supérieurs/inférieurs
        let mut bg = Path::new();
        bg.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&bg, &Paint::color(Color::rgb(18, 20, 24)));

        for rail_y in [RACK_TOP_Y - 10.0, RACK_TOP_Y + MODULE_HEIGHT + 2.0] {
            let mut rail = Path::new();
            rail.rect(bounds.x, rail_y, bounds.w, 8.0);
            canvas.fill_path(&rail, &Paint::color(Color::rgb(88, 95, 105)));
        }

        // 2. Dessin des modules du rack
        for m in &state.modules {
            let draw_x = m.x - state.scroll_x;
            if draw_x + m.kind.width() >= -20.0 && draw_x <= bounds.w + 20.0 {
                self.draw_module(canvas, m, &state, draw_x);
            }
        }

        // 3. Repère d'insertion lumineux lors du glisser-déposer depuis le Drawer
        if let Some(d_drag) = &state.drawer_drag {
            let panel_x = Self::drawer_panel_x(bounds.w, state.drawer_anim);
            if d_drag.cursor_pos.x < panel_x {
                let ix = d_drag.insert_x - state.scroll_x;
                let (ar, ag, ab) = d_drag.kind.accent_color();
                let mut guide = Path::new();
                guide.rounded_rect(ix - 3.0, RACK_TOP_Y - 6.0, 6.0, MODULE_HEIGHT + 12.0, 3.0);
                canvas.fill_path(&guide, &Paint::color(Color::rgb(ar, ag, ab)));
            }
        }

        // 4. Simulation physique Verlet et rendu des câbles
        let alpha = (self.params.cable_alpha.value() * 255.0).round() as u8;
        let dt = 1.0 / 60.0;
        let scroll_x = state.scroll_x;

        let mut endpoints = Vec::with_capacity(state.cables.len());
        for c in &state.cables {
            let p0 = state
                .modules
                .iter()
                .find(|m| m.slot_id == c.logical.src_slot)
                .and_then(|m| m.port_world_pos(PortDirection::Output, c.logical.src_port, scroll_x))
                .unwrap_or_default();
            let p1 = state
                .modules
                .iter()
                .find(|m| m.slot_id == c.logical.dst_slot)
                .and_then(|m| m.port_world_pos(PortDirection::Input, c.logical.dst_port, scroll_x))
                .unwrap_or_default();
            endpoints.push((p0, p1));
        }

        for (c, (p0, p1)) in state.cables.iter_mut().zip(endpoints.into_iter()) {
            c.physics.step(p0, p1, dt);
            Self::draw_cable(canvas, &c.physics, alpha);
        }

        // 5. Câble volant tenu à la souris
        let anchor_pos = state.cable_drag.as_ref().and_then(|drag| {
            state
                .modules
                .iter()
                .find(|m| m.slot_id == drag.from_slot)
                .and_then(|m| m.port_world_pos(drag.from_dir, drag.from_port, scroll_x))
        });

        if let (Some(drag), Some(p0)) = (state.cable_drag.as_mut(), anchor_pos) {
            drag.physics.step(p0, drag.mouse_pos, dt);
            Self::draw_cable(canvas, &drag.physics, 235);
        }

        // 6. Drawer latéral droit (Bibliothèque des modules)
        self.draw_drawer(canvas, &state, bounds.w, bounds.h);

        // 7. Fantôme translucide lors du Drag & Drop d'un module depuis le Drawer
        if let Some(d_drag) = &state.drawer_drag {
            let gw = d_drag.kind.width();
            let gh = MODULE_HEIGHT;
            let gx = d_drag.cursor_pos.x - gw * 0.5;
            let gy = (d_drag.cursor_pos.y - gh * 0.5).clamp(12.0, bounds.h - gh - 12.0);
            let (ar, ag, ab) = d_drag.kind.accent_color();

            let mut ghost = Path::new();
            ghost.rounded_rect(gx, gy, gw, gh, 6.0);
            canvas.fill_path(&ghost, &Paint::color(Color::rgba(35, 39, 47, 195)));

            let mut g_border = Paint::color(Color::rgb(ar, ag, ab));
            g_border.set_line_width(2.0);
            canvas.stroke_path(&ghost, &g_border);

            let g_title = self.text_paint(
                Color::rgb(255, 255, 255),
                14.0,
                true,
                Align::Center,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(gx + gw * 0.5, gy + 28.0, d_drag.kind.title(), &g_title);
        }

        // 8. Zone Corbeille rouge en bas à droite quand on déplace un module du rack
        if let Some(m_drag) = &state.module_drag {
            let (tx, ty, tw, th) = Self::trash_zone_rect(bounds.w, bounds.h);
            let mut trash = Path::new();
            trash.rounded_rect(tx, ty, tw, th, 8.0);
            canvas.fill_path(
                &trash,
                &Paint::color(if m_drag.over_trash {
                    Color::rgba(220, 53, 69, 235)
                } else {
                    Color::rgba(140, 35, 45, 190)
                }),
            );

            let mut t_border = Paint::color(Color::rgb(255, 110, 120));
            t_border.set_line_width(1.6);
            canvas.stroke_path(&trash, &t_border);

            let t_label = self.text_paint(
                Color::rgb(255, 255, 255),
                12.0,
                true,
                Align::Center,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(
                tx + tw * 0.5,
                ty + th * 0.5,
                "DÉPOSER ICI POUR RETIRER",
                &t_label,
            );
        }

        // 9. Menu contextuel rapide au clic droit
        if let Some(menu) = &state.context_menu {
            let total_items = ModuleKind::ALL.len() + 1;
            let total_h = total_items as f32 * MENU_ITEM_H;
            let mut box_path = Path::new();
            box_path.rounded_rect(menu.x, menu.y, MENU_WIDTH, total_h, 5.0);
            canvas.fill_path(&box_path, &Paint::color(Color::rgb(28, 31, 37)));

            let mut border = Paint::color(Color::rgb(88, 96, 110));
            border.set_line_width(1.2);
            canvas.stroke_path(&box_path, &border);

            for (idx, kind) in ModuleKind::ALL.iter().enumerate() {
                let iy = menu.y + idx as f32 * MENU_ITEM_H;
                let label = format!("+ {}", kind.title());
                let txt = self.text_paint(
                    Color::rgb(235, 240, 246),
                    11.5,
                    false,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(menu.x + 12.0, iy + MENU_ITEM_H * 0.5, &label, &txt);
            }

            let del_y = menu.y + ModuleKind::ALL.len() as f32 * MENU_ITEM_H;
            let del_txt = self.text_paint(
                if menu.target_slot.is_some() {
                    Color::rgb(255, 105, 115)
                } else {
                    Color::rgb(95, 102, 112)
                },
                11.5,
                true,
                Align::Left,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(
                menu.x + 12.0,
                del_y + MENU_ITEM_H * 0.5,
                "Supprimer ce module",
                &del_txt,
            );
        }
    }
}