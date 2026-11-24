use super::canvas::RackCanvas;
use super::layout::MENU_WIDTH;
use super::presets::SYNTH_PRESETS;
use super::state::{
    ModuleKind, ModuleVisual, PortDirection, RackTheme, RackUiState, RightDrawerTab, MODULE_HEIGHT,
};
use super::theme::ThemePalette;
use crate::i18n::Language;
use crate::physics::{CablePhysics, CABLE_NODES};
use nih_plug_vizia::vizia::prelude::Canvas;
use nih_plug_vizia::vizia::vg::{
    Align, Baseline, Color, LineCap, LineJoin, Paint, Path, Solidity,
};
use std::f32::consts::PI;

impl RackCanvas {
    pub(super) fn draw_module(
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

        self.draw_module_visualizer(canvas, m, state, palette, x, y, w);

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

    pub(super) fn draw_cable(canvas: &mut Canvas, physics: &CablePhysics, alpha: u8) {
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

    pub(super) fn draw_top_bar_and_settings(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let tr = state.tr();
        let ((ax, ay, aw, ah), (fx, fy, fw, fh), (sx, sy, sw, sh), compact) =
            Self::top_bar_rects(win_w, state.drawer_anim);

        if ax >= 68.0 {
            let zoom_str = if compact || ax < 220.0 {
                format!("{:.0}%", state.zoom * 100.0)
            } else {
                format!("ZOOM {:.0}%  ({})", state.zoom * 100.0, tr.zoom_hint)
            };
            let z_paint = self.text_paint(
                palette.ui_subtext,
                10.5,
                true,
                Align::Left,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(14.0, 24.0, &zoom_str, &z_paint);
        }

        let mut fit_btn = Path::new();
        fit_btn.rounded_rect(ax, ay, aw, ah, 6.0);
        canvas.fill_path(
            &fit_btn,
            &Paint::color(if state.auto_fit {
                Color::rgb(52, 172, 224)
            } else {
                palette.card_bg
            }),
        );
        let mut fit_border = Paint::color(palette.card_border);
        fit_border.set_line_width(1.2);
        canvas.stroke_path(&fit_btn, &fit_border);

        let fit_label = match state.language {
            Language::En => "FIT",
            Language::Fr => {
                if compact {
                    "FIT"
                } else {
                    "AJUSTER"
                }
            }
        };
        let fit_txt = self.text_paint(
            if state.auto_fit {
                Color::rgb(12, 16, 20)
            } else {
                palette.ui_text
            },
            10.0,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(ax + aw * 0.5, ay + ah * 0.5, fit_label, &fit_txt);

        let is_full = self.is_fullscreen();
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
            10.0,
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
            10.0,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(sx + sw * 0.5, sy + sh * 0.5, tr.settings_btn, &btn_txt);

        if state.settings_open {
            let (px, py, pw, ph) = Self::settings_popover_rect(win_w, win_h, state.drawer_anim);
            let mut pop = Path::new();
            pop.rounded_rect(px, py, pw, ph, 7.0);
            canvas.fill_path(&pop, &Paint::color(palette.drawer_bg));

            let mut p_border = Paint::color(palette.card_border);
            p_border.set_line_width(1.4);
            canvas.stroke_path(&pop, &p_border);

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

    pub(super) fn draw_right_drawers(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let tr = state.tr();
        let drawer_w = Self::effective_drawer_width(win_w);
        let panel_x = Self::drawer_panel_x(win_w, state.drawer_anim);

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

        let mut bg = Path::new();
        bg.rect(panel_x, 0.0, drawer_w, win_h);
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
                canvas.scissor(panel_x, 50.0, drawer_w, (win_h - 54.0).max(10.0));

                let sec_paint = self.text_paint(
                    Color::rgb(52, 172, 224),
                    9.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, dsp_hdr_y + 4.0, tr.section_dsp, &sec_paint);

                let fx_sec_paint = self.text_paint(
                    Color::rgb(255, 121, 63),
                    9.8,
                    true,
                    Align::Left,
                    Baseline::Top,
                );
                let _ = canvas.fill_text(panel_x + 14.0, fx_hdr_y + 4.0, tr.section_fx, &fx_sec_paint);

                let card_x = panel_x + 12.0;
                let card_w = (drawer_w - 24.0).max(120.0);
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

                    if card_w > 180.0 {
                        let badge_paint = self.text_paint(
                            Color::rgb(ar, ag, ab),
                            8.4,
                            true,
                            Align::Right,
                            Baseline::Top,
                        );
                        let _ = canvas.fill_text(
                            card_x + card_w - 9.0,
                            entry.y + 8.5,
                            m_sub,
                            &badge_paint,
                        );
                    }

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

                let mut top_mask = Path::new();
                top_mask.rect(panel_x + 1.0, 0.0, (drawer_w - 1.0).max(1.0), 50.0);
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
                    let (cx, cy, cw, ch) = Self::preset_card_rect(panel_x, drawer_w, win_h, idx);
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
                    let _ = canvas.fill_text(cx + 13.0, cy + 7.0, p_name, &t_paint);

                    let cat_paint = self.text_paint(
                        Color::rgb(52, 172, 224),
                        8.5,
                        true,
                        Align::Left,
                        Baseline::Top,
                    );
                    let _ = canvas.fill_text(cx + 13.0, cy + 24.0, p_cat, &cat_paint);

                    if ch >= 58.0 {
                        let desc_paint = self.text_paint(
                            palette.ui_subtext,
                            9.4,
                            false,
                            Align::Left,
                            Baseline::Top,
                        );
                        let _ = canvas.fill_text(cx + 13.0, cy + 41.0, p_desc, &desc_paint);
                    }
                }
            }
        }
    }

    pub(super) fn draw_port_hover_toast(
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

        let toast_w = 276.0f32.min((win_w - 24.0).max(180.0));
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

    pub(super) fn draw_overwrite_modal(
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
        let _ = canvas.fill_text(mx + 20.0, my + 18.0, tr.modal_overwrite_title, &title);

        let msg1 = format!("{} \"{}\"", tr.modal_overwrite_line1_prefix, p_name);
        let body_paint = self.text_paint(
            palette.ui_subtext,
            11.0,
            false,
            Align::Left,
            Baseline::Top,
        );
        let _ = canvas.fill_text(mx + 20.0, my + 48.0, &msg1, &body_paint);
        let _ = canvas.fill_text(mx + 20.0, my + 67.0, tr.modal_overwrite_line2, &body_paint);

        let mut c_btn = Path::new();
        c_btn.rounded_rect(cx, cy, cw, ch, 5.0);
        canvas.fill_path(&c_btn, &Paint::color(palette.card_bg));
        let mut cb_border = Paint::color(palette.card_border);
        cb_border.set_line_width(1.2);
        canvas.stroke_path(&c_btn, &cb_border);

        let c_txt = self.text_paint(
            palette.ui_text,
            11.0,
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
            11.0,
            true,
            Align::Center,
            Baseline::Middle,
        );
        let _ = canvas.fill_text(ox + ow * 0.5, oy + oh * 0.5, tr.modal_confirm, &o_txt);
    }

    pub(super) fn draw_overlays(
        &self,
        canvas: &mut Canvas,
        state: &RackUiState,
        palette: &ThemePalette,
        win_w: f32,
        win_h: f32,
    ) {
        let tr = state.tr();

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

        if let Some(m_drag) = &state.module_drag {
            let (tx, ty, tw, th) = Self::trash_zone_rect(win_w, win_h);
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

        if let Some(menu) = &state.context_menu {
            let item_h = Self::context_menu_item_height(win_h);
            let font_sz = (item_h * 0.42).clamp(9.0, 11.2);
            let total_items = ModuleKind::ALL.len() + 1;
            let total_h = total_items as f32 * item_h;
            let mut box_path = Path::new();
            box_path.rounded_rect(menu.screen_x, menu.screen_y, MENU_WIDTH, total_h, 5.0);
            canvas.fill_path(&box_path, &Paint::color(palette.drawer_bg));

            let mut border = Paint::color(palette.card_border);
            border.set_line_width(1.2);
            canvas.stroke_path(&box_path, &border);

            for (idx, kind) in ModuleKind::ALL.iter().enumerate() {
                let iy = menu.screen_y + idx as f32 * item_h;
                let (m_title, _, _) = tr.module_info(state.language, *kind);
                let label = format!("+ {}", m_title);
                let txt = self.text_paint(
                    palette.ui_text,
                    font_sz,
                    false,
                    Align::Left,
                    Baseline::Middle,
                );
                let _ = canvas.fill_text(menu.screen_x + 12.0, iy + item_h * 0.5, &label, &txt);
            }

            let del_y = menu.screen_y + ModuleKind::ALL.len() as f32 * item_h;
            let del_txt = self.text_paint(
                if menu.target_slot.is_some() {
                    Color::rgb(255, 105, 115)
                } else {
                    palette.ui_subtext
                },
                font_sz,
                true,
                Align::Left,
                Baseline::Middle,
            );
            let _ = canvas.fill_text(
                menu.screen_x + 12.0,
                del_y + item_h * 0.5,
                tr.ctx_delete_module,
                &del_txt,
            );
        }

        if !self.is_fullscreen() {
            let mut grip_paint = Paint::color(palette.ui_subtext);
            grip_paint.set_line_width(1.5);
            for offset in [6.0, 11.0, 16.0] {
                let mut line = Path::new();
                line.move_to(win_w - offset, win_h - 3.0);
                line.line_to(win_w - 3.0, win_h - offset);
                canvas.stroke_path(&line, &grip_paint);
            }
        }

        self.draw_port_hover_toast(canvas, state, palette, win_w, win_h);

        if let Some(preset_idx) = state.pending_preset {
            self.draw_overwrite_modal(canvas, state, preset_idx, palette, win_w, win_h);
        }
    }
}