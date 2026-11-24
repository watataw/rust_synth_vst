use super::canvas::RackCanvas;
use super::state::{ModuleKind, ModuleVisual, RackUiState};
use super::theme::ThemePalette;
use nih_plug_vizia::vizia::prelude::Canvas;
use nih_plug_vizia::vizia::vg::{Align, Baseline, Color, LineJoin, Paint, Path};
use std::f32::consts::{PI, TAU};

impl RackCanvas {
    pub(super) fn midi_note_name(v_oct: f32) -> String {
        let midi = (v_oct * 12.0 + 60.0).round() as i32;
        let clamped = midi.clamp(0, 127);
        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let note = names[(clamped % 12) as usize];
        let octave = (clamped / 12) - 1;
        format!("{}{}", note, octave)
    }

    pub(super) fn draw_module_visualizer(
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
                if bar_h > 0.5 {
                    let mut meter = Path::new();
                    meter.rounded_rect(sx + sw - 16.0, sy + sh - 8.0 - bar_h, 8.0, bar_h, 2.0);
                    canvas.fill_path(&meter, &Paint::color(accent));
                }
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
                    if bh > 0.5 {
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
}