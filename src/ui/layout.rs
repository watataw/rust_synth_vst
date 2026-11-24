use super::canvas::RackCanvas;
use super::presets::SYNTH_PRESETS;
use super::state::{
    ModuleKind, PortDirection, RackUiState, RightDrawerTab, RACK_LEFT_MARGIN, RACK_TOP_Y,
    ROW_STRIDE,
};
use crate::physics::Vec2;

pub(super) const JACK_HIT_RADIUS: f32 = 14.0;
pub(super) const MENU_WIDTH: f32 = 195.0;
pub(super) const MENU_ITEM_H: f32 = 27.0;
pub(super) const DRAWER_WIDTH: f32 = 264.0;
pub(super) const DRAWER_TAB_W: f32 = 30.0;
pub(super) const DRAWER_TAB_H: f32 = 112.0;
pub(super) const RESIZE_GRIP_SIZE: f32 = 22.0;

pub(super) struct DrawerModuleEntry {
    pub(super) kind: ModuleKind,
    pub(super) y: f32,
    pub(super) h: f32,
}

impl RackCanvas {
    pub(super) fn effective_drawer_width(win_w: f32) -> f32 {
        (win_w * 0.30).clamp(145.0, DRAWER_WIDTH)
    }

    pub(super) fn effective_drawer_tab_w(win_w: f32) -> f32 {
        if win_w < 640.0 {
            24.0
        } else {
            DRAWER_TAB_W
        }
    }

    pub(super) fn drawer_panel_x(win_w: f32, drawer_anim: f32) -> f32 {
        win_w - Self::effective_drawer_width(win_w) * drawer_anim
    }

    pub(super) fn drawer_tabs_rects(
        win_w: f32,
        win_h: f32,
        drawer_anim: f32,
    ) -> [(RightDrawerTab, (f32, f32, f32, f32)); 2] {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        let tab_w = Self::effective_drawer_tab_w(win_w);
        let tab_x = panel_x - tab_w;
        let tab_h = DRAWER_TAB_H.min(((win_h - 70.0) * 0.45).max(68.0));
        let total_h = tab_h * 2.0 + 8.0;
        let start_y = ((win_h - total_h) * 0.5).max(48.0);
        [
            (RightDrawerTab::Modules, (tab_x, start_y, tab_w, tab_h)),
            (
                RightDrawerTab::Presets,
                (tab_x, start_y + tab_h + 8.0, tab_w, tab_h),
            ),
        ]
    }

    pub(super) fn top_bar_rects(
        win_w: f32,
        drawer_anim: f32,
    ) -> ((f32, f32, f32, f32), (f32, f32, f32, f32), (f32, f32, f32, f32), bool) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        let compact = panel_x < 560.0;
        let (fit_w, full_w, set_w, gap) = if compact {
            (44.0, 74.0, 74.0, 5.0)
        } else {
            (62.0, 110.0, 106.0, 8.0)
        };
        let set_x = (panel_x - set_w - 8.0).max(fit_w + full_w + gap * 2.0 + 8.0);
        let full_x = set_x - full_w - gap;
        let fit_x = full_x - fit_w - gap;
        (
            (fit_x, 10.0, fit_w, 30.0),
            (full_x, 10.0, full_w, 30.0),
            (set_x, 10.0, set_w, 30.0),
            compact,
        )
    }

    pub(super) fn settings_popover_rect(
        win_w: f32,
        win_h: f32,
        drawer_anim: f32,
    ) -> (f32, f32, f32, f32) {
        let panel_x = Self::drawer_panel_x(win_w, drawer_anim);
        let pw = 254.0f32.min((win_w - 16.0).max(190.0));
        let ph = 294.0f32.min((win_h - 54.0).max(210.0));
        let px = (panel_x - pw - 10.0).clamp(8.0, (win_w - pw - 8.0).max(8.0));
        (px, 46.0, pw, ph)
    }

    pub(super) fn layout_module_drawer_entries(
        scroll: f32,
    ) -> (f32, Vec<DrawerModuleEntry>, f32, f32) {
        let mut y = 56.0 - scroll;
        let dsp_hdr_y = y;
        y += 24.0;
        let mut entries = Vec::with_capacity(ModuleKind::ALL.len());
        let card_h = 48.0;
        let card_gap = 6.0;

        for &kind in &ModuleKind::DSP_MODULES {
            entries.push(DrawerModuleEntry { kind, y, h: card_h });
            y += card_h + card_gap;
        }

        y += 8.0;
        let fx_hdr_y = y;
        y += 24.0;

        for &kind in &ModuleKind::FX_MODULES {
            entries.push(DrawerModuleEntry { kind, y, h: card_h });
            y += card_h + card_gap;
        }

        let total_h = y + scroll + 16.0;
        (dsp_hdr_y, entries, fx_hdr_y, total_h)
    }

    pub(super) fn preset_card_rect(
        panel_x: f32,
        drawer_w: f32,
        win_h: f32,
        index: usize,
    ) -> (f32, f32, f32, f32) {
        let card_x = panel_x + 10.0;
        let avail_h = (win_h - 64.0).max(210.0);
        let stride = 76.0f32.min(avail_h / SYNTH_PRESETS.len() as f32);
        let card_h = (stride - 6.0).max(36.0);
        let card_y = 54.0 + index as f32 * stride;
        let card_w = (drawer_w - 20.0).max(100.0);
        (card_x, card_y, card_w, card_h)
    }

    pub(super) fn trash_zone_rect(win_w: f32, win_h: f32) -> (f32, f32, f32, f32) {
        let tw = 238.0f32.min((win_w - 24.0).max(140.0));
        (win_w - tw - 16.0, win_h - 64.0, tw, 48.0)
    }

    pub(super) fn confirm_modal_rects(
        win_w: f32,
        win_h: f32,
    ) -> ((f32, f32, f32, f32), (f32, f32, f32, f32), (f32, f32, f32, f32)) {
        let mw = 416.0f32.min((win_w - 20.0).max(240.0));
        let mh = 176.0f32.min((win_h - 20.0).max(140.0));
        let mx = (win_w - mw) * 0.5;
        let my = (win_h - mh) * 0.5;
        let btn_w = ((mw - 52.0) * 0.48).max(84.0);
        let cancel_btn = (mx + 16.0, my + mh - 46.0, btn_w, 32.0);
        let confirm_btn = (mx + mw - btn_w - 16.0, my + mh - 46.0, btn_w, 32.0);
        ((mx, my, mw, mh), cancel_btn, confirm_btn)
    }

    pub(super) fn context_menu_item_height(win_h: f32) -> f32 {
        let count = (ModuleKind::ALL.len() + 1) as f32;
        MENU_ITEM_H.min(((win_h - 16.0) / count).max(16.0))
    }

    pub(super) fn hit_test_port(
        state: &RackUiState,
        screen_x: f32,
        screen_y: f32,
    ) -> Option<(usize, PortDirection, usize, Vec2)> {
        let world_cursor = state.screen_to_world(screen_x, screen_y);
        let hit_r = JACK_HIT_RADIUS / state.zoom.max(0.20).sqrt();
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

    pub(super) fn is_over_knob(state: &RackUiState, screen_x: f32, screen_y: f32) -> bool {
        let world_pos = state.screen_to_world(screen_x, screen_y);
        for m in &state.modules {
            let my_top = m.world_top_y();
            for knob in &m.knobs {
                let kx = m.x + knob.local_x;
                let ky = my_top + knob.local_y;
                if Vec2::new(kx, ky).distance(world_pos) <= knob.radius + 4.0 {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn compute_target_row_and_x(
        state: &RackUiState,
        world_pos: Vec2,
        ignore_slot: Option<usize>,
    ) -> (usize, f32) {
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
}