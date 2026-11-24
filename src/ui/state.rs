use crate::dsp::{
    AdsrParams, AudioOutParams, ChorusParams, DistortionParams, LfoParams, OverdriveParams,
    PhaserParams, VcaParams, VcfParams, VcoParams,
};
use crate::engine::{
    compile_execution_plan, AtomicF32, AudioCommand, AudioGarbage, LogicalCable, TelemetrySnapshot,
    MAX_MODULES, MAX_PORTS_PER_MODULE,
};
use crate::i18n::{Language, Translations};
use crate::physics::{CablePhysics, Vec2};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use super::presets::SYNTH_PRESETS;
pub use super::theme::RackTheme;

pub const RACK_TOP_Y: f32 = 54.0;
pub const MODULE_HEIGHT: f32 = 492.0;
pub const ROW_GAP: f32 = 26.0;
pub const ROW_STRIDE: f32 = MODULE_HEIGHT + ROW_GAP;
pub const RACK_LEFT_MARGIN: f32 = 20.0;
pub const MODULE_GAP: f32 = 4.0;
pub const LFO_HISTORY_LEN: usize = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightDrawerTab {
    Modules,
    Presets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    MidiToCv,
    Vco,
    Vcf,
    Adsr,
    Lfo,
    Vca,
    AudioOutput,
    Overdrive,
    Distortion,
    Chorus,
    Phaser,
}

impl ModuleKind {
    pub const DSP_MODULES: [ModuleKind; 7] = [
        ModuleKind::MidiToCv,
        ModuleKind::Vco,
        ModuleKind::Vcf,
        ModuleKind::Adsr,
        ModuleKind::Lfo,
        ModuleKind::Vca,
        ModuleKind::AudioOutput,
    ];

    pub const FX_MODULES: [ModuleKind; 4] = [
        ModuleKind::Overdrive,
        ModuleKind::Distortion,
        ModuleKind::Chorus,
        ModuleKind::Phaser,
    ];

    pub const ALL: [ModuleKind; 11] = [
        ModuleKind::MidiToCv,
        ModuleKind::Vco,
        ModuleKind::Vcf,
        ModuleKind::Adsr,
        ModuleKind::Lfo,
        ModuleKind::Vca,
        ModuleKind::AudioOutput,
        ModuleKind::Overdrive,
        ModuleKind::Distortion,
        ModuleKind::Chorus,
        ModuleKind::Phaser,
    ];

    pub fn accent_color(self) -> (u8, u8, u8) {
        match self {
            ModuleKind::MidiToCv => (52, 172, 224),
            ModuleKind::Vco => (255, 121, 63),
            ModuleKind::Vcf => (255, 177, 66),
            ModuleKind::Adsr => (51, 217, 178),
            ModuleKind::Lfo => (165, 94, 234),
            ModuleKind::Vca => (235, 77, 75),
            ModuleKind::AudioOutput => (46, 204, 113),
            ModuleKind::Overdrive => (230, 126, 34),
            ModuleKind::Distortion => (231, 76, 60),
            ModuleKind::Chorus => (26, 188, 156),
            ModuleKind::Phaser => (155, 89, 182),
        }
    }

    pub fn width(self) -> f32 {
        match self {
            ModuleKind::MidiToCv => 144.0,
            ModuleKind::Vco => 186.0,
            ModuleKind::Vcf => 186.0,
            ModuleKind::Adsr => 176.0,
            ModuleKind::Lfo => 164.0,
            ModuleKind::Vca => 148.0,
            ModuleKind::AudioOutput => 156.0,
            ModuleKind::Overdrive => 156.0,
            ModuleKind::Distortion => 156.0,
            ModuleKind::Chorus => 164.0,
            ModuleKind::Phaser => 164.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

pub struct UiPort {
    pub name: &'static str,
    pub index: usize,
    pub direction: PortDirection,
    pub local_x: f32,
    pub local_y: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum KnobDisplayStyle {
    Semitones,
    Percent,
    Hertz,
    Milliseconds,
    LfoWave,
    Polarity,
    LinExp,
}

pub struct UiKnob {
    pub label: &'static str,
    pub local_x: f32,
    pub local_y: f32,
    pub radius: f32,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub style: KnobDisplayStyle,
    pub param: Arc<AtomicF32>,
}

impl UiKnob {
    pub fn formatted_value(&self) -> String {
        let v = self.param.load();
        match self.style {
            KnobDisplayStyle::Semitones => format!("{:+.1} st", v),
            KnobDisplayStyle::Percent => format!("{:.0}%", v * 100.0),
            KnobDisplayStyle::Hertz => {
                if v >= 1000.0 {
                    format!("{:.2} kHz", v / 1000.0)
                } else if v >= 10.0 {
                    format!("{:.0} Hz", v)
                } else {
                    format!("{:.2} Hz", v)
                }
            }
            KnobDisplayStyle::Milliseconds => {
                if v >= 1.0 {
                    format!("{:.2} s", v)
                } else {
                    format!("{:.0} ms", v * 1000.0)
                }
            }
            KnobDisplayStyle::LfoWave => match v.round() as i32 {
                0 => "SIN".to_string(),
                1 => "TRI".to_string(),
                2 => "SAW".to_string(),
                3 => "SQR".to_string(),
                _ => "S&H".to_string(),
            },
            KnobDisplayStyle::Polarity => {
                if v > 0.5 {
                    "UNI +5V".to_string()
                } else {
                    "BI ±5V".to_string()
                }
            }
            KnobDisplayStyle::LinExp => {
                if v < 0.25 {
                    "LIN".to_string()
                } else if v > 0.75 {
                    "EXP".to_string()
                } else {
                    format!("MIX {:.0}%", v * 100.0)
                }
            }
        }
    }
}

pub enum ModuleParamHandle {
    None,
    Vco(Arc<VcoParams>),
    Vcf(Arc<VcfParams>),
    Adsr(Arc<AdsrParams>),
    Lfo(Arc<LfoParams>),
    Vca(Arc<VcaParams>),
    AudioOut(Arc<AudioOutParams>),
    Overdrive(Arc<OverdriveParams>),
    Distortion(Arc<DistortionParams>),
    Chorus(Arc<ChorusParams>),
    Phaser(Arc<PhaserParams>),
}

pub struct ModuleVisual {
    pub slot_id: usize,
    pub kind: ModuleKind,
    pub row: usize,
    pub x: f32,
    pub ports: Vec<UiPort>,
    pub knobs: Vec<UiKnob>,
    pub dsp_params: ModuleParamHandle,
}

impl ModuleVisual {
    #[inline]
    pub fn row_top_y(row: usize) -> f32 {
        RACK_TOP_Y + (row as f32) * ROW_STRIDE
    }

    pub fn world_top_y(&self) -> f32 {
        Self::row_top_y(self.row)
    }

    pub fn port_world_pos(&self, dir: PortDirection, port_idx: usize) -> Option<Vec2> {
        self.ports
            .iter()
            .find(|p| p.direction == dir && p.index == port_idx)
            .map(|p| Vec2::new(self.x + p.local_x, self.world_top_y() + p.local_y))
    }
}

pub struct PatchCableVisual {
    pub logical: LogicalCable,
    pub physics: CablePhysics,
}

pub struct ActiveCableDrag {
    pub from_slot: usize,
    pub from_port: usize,
    pub from_dir: PortDirection,
    pub physics: CablePhysics,
    pub world_mouse: Vec2,
    pub preserved_color: Option<(u8, u8, u8)>,
}

pub struct ActiveModuleDrag {
    pub slot_id: usize,
    pub grab_offset_x: f32,
    pub screen_cursor: Vec2,
    pub target_row: usize,
    pub insert_x: f32,
    pub over_trash: bool,
}

pub struct ActiveDrawerDrag {
    pub kind: ModuleKind,
    pub screen_cursor: Vec2,
    pub target_row: usize,
    pub insert_x: f32,
}

pub struct ActiveKnobDrag {
    pub slot_id: usize,
    pub knob_idx: usize,
    pub start_y: f32,
    pub start_val: f32,
}

pub struct ActivePanDrag {
    pub last_screen: Vec2,
}

pub struct ContextMenuState {
    pub screen_x: f32,
    pub screen_y: f32,
    pub target_slot: Option<usize>,
}

pub struct RackUiState {
    pub modules: Vec<ModuleVisual>,
    pub cables: Vec<PatchCableVisual>,
    pub cmd_tx: rtrb::Producer<AudioCommand>,
    pub garbage_rx: rtrb::Consumer<AudioGarbage>,
    pub telemetry_rx: triple_buffer::Output<TelemetrySnapshot>,
    pub latest_telemetry: TelemetrySnapshot,
    pub cable_drag: Option<ActiveCableDrag>,
    pub module_drag: Option<ActiveModuleDrag>,
    pub drawer_drag: Option<ActiveDrawerDrag>,
    pub knob_drag: Option<ActiveKnobDrag>,
    pub pan_drag: Option<ActivePanDrag>,
    pub context_menu: Option<ContextMenuState>,
    pub active_drawer: Option<RightDrawerTab>,
    pub drawer_anim: f32,
    pub drawer_scroll: f32,
    pub settings_open: bool,
    pub theme: RackTheme,
    pub language: Language,
    pub tooltip_delay_ms: u64,
    pub hovered_port: Option<(usize, PortDirection, usize)>,
    pub hover_since: Option<Instant>,
    pub hover_screen_pos: Vec2,
    pub pending_preset: Option<usize>,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub auto_fit: bool,
    pub lfo_histories: HashMap<usize, VecDeque<f32>>,
    next_color_idx: usize,
}

impl RackUiState {
    pub fn new(
        cmd_tx: rtrb::Producer<AudioCommand>,
        garbage_rx: rtrb::Consumer<AudioGarbage>,
        telemetry_rx: triple_buffer::Output<TelemetrySnapshot>,
    ) -> Self {
        let mut state = Self {
            modules: Vec::new(),
            cables: Vec::new(),
            cmd_tx,
            garbage_rx,
            telemetry_rx,
            latest_telemetry: TelemetrySnapshot::default(),
            cable_drag: None,
            module_drag: None,
            drawer_drag: None,
            knob_drag: None,
            pan_drag: None,
            context_menu: None,
            active_drawer: Some(RightDrawerTab::Modules),
            drawer_anim: 1.0,
            drawer_scroll: 0.0,
            settings_open: false,
            theme: RackTheme::Current,
            language: Language::En,
            tooltip_delay_ms: 500,
            hovered_port: None,
            hover_since: None,
            hover_screen_pos: Vec2::new(0.0, 0.0),
            pending_preset: None,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            auto_fit: true,
            lfo_histories: HashMap::new(),
            next_color_idx: 0,
        };

        state.apply_preset(0);
        state
    }

    pub fn tr(&self) -> &'static Translations {
        self.language.translations()
    }

    pub fn screen_to_world(&self, sx: f32, sy: f32) -> Vec2 {
        let z = self.zoom.max(0.05);
        Vec2::new((sx - self.pan_x) / z, (sy - self.pan_y) / z)
    }

    pub fn collect_garbage(&mut self) {
        while let Ok(item) = self.garbage_rx.pop() {
            drop(item);
        }
    }

    pub fn poll_telemetry(&mut self) {
        self.latest_telemetry = *self.telemetry_rx.read();

        for m in &self.modules {
            if m.kind == ModuleKind::Lfo {
                let bus_idx = m.slot_id * MAX_PORTS_PER_MODULE;
                let live_v = self.latest_telemetry.port_values[bus_idx];
                let hist = self
                    .lfo_histories
                    .entry(m.slot_id)
                    .or_insert_with(|| VecDeque::with_capacity(LFO_HISTORY_LEN));
                if hist.len() >= LFO_HISTORY_LEN {
                    hist.pop_front();
                }
                hist.push_back(live_v);
            }
        }
    }

    pub(super) fn allocate_slot(&self) -> Option<usize> {
        (0..MAX_MODULES).find(|&s| !self.modules.iter().any(|m| m.slot_id == s))
    }

    pub fn remove_module(&mut self, slot_id: usize) {
        self.modules.retain(|m| m.slot_id != slot_id);
        self.lfo_histories.remove(&slot_id);
        self.cables
            .retain(|c| c.logical.src_slot != slot_id && c.logical.dst_slot != slot_id);

        let _ = self.cmd_tx.push(AudioCommand::RemoveModule { slot_id });
        self.pack_modules();
        self.recompile_and_push_plan();
    }

    pub fn apply_preset(&mut self, preset_idx: usize) {
        let Some(preset) = SYNTH_PRESETS.get(preset_idx) else {
            return;
        };

        let existing_slots: Vec<usize> = self.modules.iter().map(|m| m.slot_id).collect();
        for slot in existing_slots {
            let _ = self.cmd_tx.push(AudioCommand::RemoveModule { slot_id: slot });
        }
        self.modules.clear();
        self.cables.clear();
        self.lfo_histories.clear();
        self.next_color_idx = 0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.auto_fit = true;

        let mut created_slots = Vec::with_capacity(preset.modules.len());
        for spec in preset.modules {
            let slot_opt = self.spawn_module_at(spec.kind, spec.row, None);
            if let Some(slot_id) = slot_opt {
                if let Some(m) = self.modules.iter().find(|m| m.slot_id == slot_id) {
                    for (k_idx, &val) in spec.knobs.iter().enumerate() {
                        if let Some(knob) = m.knobs.get(k_idx) {
                            knob.param.store(val.clamp(knob.min, knob.max));
                        }
                    }
                }
                self.sync_module_knobs_to_dsp(slot_id);
            }
            created_slots.push(slot_opt);
        }

        for c_spec in preset.cables {
            if let (Some(Some(src_slot)), Some(Some(dst_slot))) = (
                created_slots.get(c_spec.src_mod_idx),
                created_slots.get(c_spec.dst_mod_idx),
            ) {
                self.connect_cable(LogicalCable {
                    src_slot: *src_slot,
                    src_port: c_spec.src_port,
                    dst_slot: *dst_slot,
                    dst_port: c_spec.dst_port,
                });
            }
        }
    }

    pub fn pack_modules(&mut self) {
        if self.modules.is_empty() {
            return;
        }

        let mut distinct_rows: Vec<usize> = self.modules.iter().map(|m| m.row).collect();
        distinct_rows.sort_unstable();
        distinct_rows.dedup();

        for m in &mut self.modules {
            if let Some(compact_row) = distinct_rows.iter().position(|&r| r == m.row) {
                m.row = compact_row;
            }
        }

        self.modules
            .sort_by(|a, b| a.row.cmp(&b.row).then_with(|| a.x.total_cmp(&b.x)));

        let mut current_row = usize::MAX;
        let mut cursor_x = RACK_LEFT_MARGIN;
        for m in &mut self.modules {
            if m.row != current_row {
                current_row = m.row;
                cursor_x = RACK_LEFT_MARGIN;
            }
            m.x = cursor_x;
            cursor_x += m.kind.width() + MODULE_GAP;
        }
    }

    pub fn num_rows(&self) -> usize {
        self.modules.iter().map(|m| m.row + 1).max().unwrap_or(1)
    }

    pub fn max_row_width(&self) -> f32 {
        self.rack_content_size(self.num_rows()).0
    }

    pub fn rack_content_size(&self, display_rows: usize) -> (f32, f32) {
        let rows = display_rows.max(1);
        let mut max_w = if self.modules.is_empty() { 480.0f32 } else { 240.0f32 };
        for r in 0..rows {
            let row_end = self
                .modules
                .iter()
                .filter(|m| m.row == r)
                .map(|m| m.x + m.kind.width() + RACK_LEFT_MARGIN)
                .fold(0.0f32, f32::max);
            max_w = max_w.max(row_end);
        }
        let total_h = RACK_TOP_Y + (rows as f32) * ROW_STRIDE + 16.0;
        (max_w, total_h)
    }

    pub fn fit_to_viewport(&mut self, avail_w: f32, avail_h: f32, display_rows: usize) {
        let (content_w, content_h) = self.rack_content_size(display_rows);
        let usable_w = (avail_w - 16.0).max(80.0);
        let usable_h = (avail_h - RACK_TOP_Y - 12.0).max(80.0);
        let world_h = (content_h - RACK_TOP_Y).max(80.0);

        let scale_x = usable_w / content_w.max(1.0);
        let scale_y = usable_h / world_h;
        let new_zoom = scale_x.min(scale_y).clamp(0.15, 1.75);

        self.zoom = new_zoom;
        let scaled_w = content_w * new_zoom;
        self.pan_x = ((avail_w - scaled_w) * 0.5).max(6.0);
        self.pan_y = RACK_TOP_Y * (1.0 - new_zoom);
    }

    pub fn clamp_pan(&mut self, avail_w: f32, avail_h: f32, display_rows: usize) {
        let (content_w, content_h) = self.rack_content_size(display_rows);
        let scaled_w = content_w * self.zoom;
        let scaled_h = content_h * self.zoom;
        let margin_x = (avail_w * 0.35).max(120.0);
        let margin_y = (avail_h * 0.35).max(120.0);

        let min_x = (avail_w - scaled_w - margin_x).min(margin_x);
        let max_x = margin_x.max(avail_w - scaled_w);
        let min_y = (avail_h - scaled_h - margin_y).min(margin_y);
        let max_y = margin_y.max(avail_h - scaled_h);

        self.pan_x = self.pan_x.clamp(min_x, max_x);
        self.pan_y = self.pan_y.clamp(min_y, max_y);
    }

    pub fn sync_module_knobs_to_dsp(&self, slot_id: usize) {
        if let Some(m) = self.modules.iter().find(|m| m.slot_id == slot_id) {
            match &m.dsp_params {
                ModuleParamHandle::None => {}
                ModuleParamHandle::Vco(vp) => {
                    vp.coarse.store(m.knobs[0].param.load());
                    vp.fine.store(m.knobs[1].param.load());
                    vp.fm_depth.store(m.knobs[2].param.load());
                    vp.pulse_width.store(m.knobs[3].param.load());
                }
                ModuleParamHandle::Vcf(fp) => {
                    fp.cutoff.store(m.knobs[0].param.load());
                    fp.resonance.store(m.knobs[1].param.load());
                    fp.drive.store(m.knobs[2].param.load());
                    fp.cv_amount.store(m.knobs[3].param.load());
                }
                ModuleParamHandle::Adsr(ap) => {
                    ap.attack.store(m.knobs[0].param.load());
                    ap.decay.store(m.knobs[1].param.load());
                    ap.sustain.store(m.knobs[2].param.load());
                    ap.release.store(m.knobs[3].param.load());
                }
                ModuleParamHandle::Lfo(lp) => {
                    lp.rate.store(m.knobs[0].param.load());
                    lp.waveform.store(m.knobs[1].param.load());
                    lp.unipolar.store(m.knobs[2].param.load());
                }
                ModuleParamHandle::Vca(cp) => {
                    cp.level.store(m.knobs[0].param.load());
                    cp.cv_amount.store(m.knobs[1].param.load());
                    cp.response.store(m.knobs[2].param.load());
                }
                ModuleParamHandle::AudioOut(op) => {
                    op.master_gain.store(m.knobs[0].param.load());
                }
                ModuleParamHandle::Overdrive(odp) => {
                    odp.drive.store(m.knobs[0].param.load());
                    odp.tone.store(m.knobs[1].param.load());
                    odp.mix.store(m.knobs[2].param.load());
                }
                ModuleParamHandle::Distortion(dp) => {
                    dp.gain.store(m.knobs[0].param.load());
                    dp.fold.store(m.knobs[1].param.load());
                    dp.mix.store(m.knobs[2].param.load());
                }
                ModuleParamHandle::Chorus(chp) => {
                    chp.rate.store(m.knobs[0].param.load());
                    chp.depth.store(m.knobs[1].param.load());
                    chp.mix.store(m.knobs[2].param.load());
                }
                ModuleParamHandle::Phaser(php) => {
                    php.rate.store(m.knobs[0].param.load());
                    php.depth.store(m.knobs[1].param.load());
                    php.feedback.store(m.knobs[2].param.load());
                }
            }
        }
    }

    pub fn connect_cable_with_color(
        &mut self,
        logical: LogicalCable,
        preserved_color: Option<(u8, u8, u8)>,
    ) -> bool {
        if self.cables.iter().any(|c| c.logical == logical) {
            return false;
        }

        let start = self
            .modules
            .iter()
            .find(|m| m.slot_id == logical.src_slot)
            .and_then(|m| m.port_world_pos(PortDirection::Output, logical.src_port));

        let end = self
            .modules
            .iter()
            .find(|m| m.slot_id == logical.dst_slot)
            .and_then(|m| m.port_world_pos(PortDirection::Input, logical.dst_port));

        if let (Some(p0), Some(p1)) = (start, end) {
            let mut physics = CablePhysics::new(p0, p1, self.next_color_idx);
            if let Some(col) = preserved_color {
                physics.color = col;
            } else {
                self.next_color_idx += 1;
            }
            self.cables.push(PatchCableVisual { logical, physics });
            self.recompile_and_push_plan();
            true
        } else {
            false
        }
    }

    pub fn connect_cable(&mut self, logical: LogicalCable) -> bool {
        self.connect_cable_with_color(logical, None)
    }

    pub fn disconnect_cable_at_port(
        &mut self,
        slot_id: usize,
        dir: PortDirection,
        port_idx: usize,
    ) -> Option<(LogicalCable, (u8, u8, u8))> {
        let found_idx = self.cables.iter().rposition(|c| match dir {
            PortDirection::Output => c.logical.src_slot == slot_id && c.logical.src_port == port_idx,
            PortDirection::Input => c.logical.dst_slot == slot_id && c.logical.dst_port == port_idx,
        });

        if let Some(idx) = found_idx {
            let removed = self.cables.remove(idx);
            self.recompile_and_push_plan();
            Some((removed.logical, removed.physics.color))
        } else {
            None
        }
    }

    pub fn recompile_and_push_plan(&mut self) {
        let active_slots: Vec<usize> = self.modules.iter().map(|m| m.slot_id).collect();
        let logical_cables: Vec<LogicalCable> = self.cables.iter().map(|c| c.logical).collect();
        let plan = compile_execution_plan(&active_slots, &logical_cables);
        let _ = self.cmd_tx.push(AudioCommand::UpdatePlan(plan));
    }

    pub fn port_signal_level(&self, slot_id: usize, port_idx: usize) -> f32 {
        let bus_idx = slot_id * MAX_PORTS_PER_MODULE + port_idx;
        self.latest_telemetry.port_levels[bus_idx]
    }

    pub fn port_signal_value(&self, slot_id: usize, port_idx: usize) -> f32 {
        let bus_idx = slot_id * MAX_PORTS_PER_MODULE + port_idx;
        self.latest_telemetry.port_values[bus_idx]
    }
}