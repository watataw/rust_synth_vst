use crate::dsp::{
    AdsrModule, AdsrParams, AudioOutParams, AudioOutputModule, LfoModule, LfoParams,
    MidiToCvModule, VcaModule, VcaParams, VcfModule, VcfParams, VcoModule, VcoParams,
};
use crate::engine::{
    compile_execution_plan, AtomicF32, AudioCommand, AudioGarbage, DspModule, LogicalCable,
    TelemetrySnapshot, MAX_MODULES, MAX_PORTS_PER_MODULE,
};
use crate::physics::{CablePhysics, Vec2};
use std::sync::Arc;

pub const RACK_TOP_Y: f32 = 34.0;
pub const MODULE_HEIGHT: f32 = 492.0;
pub const RACK_LEFT_MARGIN: f32 = 20.0;
pub const MODULE_GAP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    MidiToCv,
    Vco,
    Vcf,
    Adsr,
    Lfo,
    Vca,
    AudioOutput,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 7] = [
        ModuleKind::MidiToCv,
        ModuleKind::Vco,
        ModuleKind::Vcf,
        ModuleKind::Adsr,
        ModuleKind::Lfo,
        ModuleKind::Vca,
        ModuleKind::AudioOutput,
    ];

    pub fn title(self) -> &'static str {
        match self {
            ModuleKind::MidiToCv => "MIDI -> CV",
            ModuleKind::Vco => "VCO-1",
            ModuleKind::Vcf => "VCF-24",
            ModuleKind::Adsr => "ADSR",
            ModuleKind::Lfo => "LFO",
            ModuleKind::Vca => "VCA",
            ModuleKind::AudioOutput => "AUDIO OUT",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            ModuleKind::MidiToCv => "INTERFACE DAW",
            ModuleKind::Vco => "OSCILLATEUR",
            ModuleKind::Vcf => "FILTRE LADDER",
            ModuleKind::Adsr => "ENVELOPPE",
            ModuleKind::Lfo => "MODULATEUR",
            ModuleKind::Vca => "AMPLIFICATEUR",
            ModuleKind::AudioOutput => "SORTIE STÉRÉO",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ModuleKind::MidiToCv => "V/Oct, Gate, Trig, Vel, Bend",
            ModuleKind::Vco => "Sin, Tri, Saw, Square + FM/PWM",
            ModuleKind::Vcf => "Passe-bas/bande/haut 24dB/oct",
            ModuleKind::Adsr => "Attack, Decay, Sustain, Release",
            ModuleKind::Lfo => "0.05-50 Hz + Sample & Hold",
            ModuleKind::Vca => "Contrôle d'amplitude Lin/Exp",
            ModuleKind::AudioOutput => "Master Stéréo + Soft Clipper",
        }
    }

    pub fn accent_color(self) -> (u8, u8, u8) {
        match self {
            ModuleKind::MidiToCv => (52, 172, 224),   // Bleu cyan
            ModuleKind::Vco => (255, 121, 63),        // Orange vif
            ModuleKind::Vcf => (255, 177, 66),        // Ambre doré
            ModuleKind::Adsr => (51, 217, 178),       // Turquoise menthe
            ModuleKind::Lfo => (165, 94, 234),        // Violet électrique
            ModuleKind::Vca => (235, 77, 75),         // Rouge corail
            ModuleKind::AudioOutput => (46, 204, 113), // Vert émeraude
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
}

pub struct ModuleVisual {
    pub slot_id: usize,
    pub kind: ModuleKind,
    pub x: f32,
    pub ports: Vec<UiPort>,
    pub knobs: Vec<UiKnob>,
    pub dsp_params: ModuleParamHandle,
}

impl ModuleVisual {
    pub fn port_world_pos(&self, dir: PortDirection, port_idx: usize, scroll_x: f32) -> Option<Vec2> {
        self.ports
            .iter()
            .find(|p| p.direction == dir && p.index == port_idx)
            .map(|p| Vec2::new(self.x - scroll_x + p.local_x, RACK_TOP_Y + p.local_y))
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
    pub mouse_pos: Vec2,
}

pub struct ActiveModuleDrag {
    pub slot_id: usize,
    pub grab_offset_x: f32,
    pub cursor_pos: Vec2,
    pub over_trash: bool,
}

pub struct ActiveDrawerDrag {
    pub kind: ModuleKind,
    pub cursor_pos: Vec2,
    pub insert_x: f32,
}

pub struct ActiveKnobDrag {
    pub slot_id: usize,
    pub knob_idx: usize,
    pub start_y: f32,
    pub start_val: f32,
}

pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
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
    pub context_menu: Option<ContextMenuState>,
    pub drawer_open: bool,
    pub drawer_anim: f32,
    pub scroll_x: f32,
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
            context_menu: None,
            drawer_open: true, // Ouvert par défaut pour découvrir la bibliothèque de modules
            drawer_anim: 1.0,
            scroll_x: 0.0,
            next_color_idx: 0,
        };

        // Patch de démarrage complet et musical :
        // MIDI -> CV -> VCO-1 -> VCF-24 -> VCA -> AUDIO OUT, modulé par ADSR
        let m_midi = state.spawn_module_at(ModuleKind::MidiToCv, None);
        let m_vco = state.spawn_module_at(ModuleKind::Vco, None);
        let m_vcf = state.spawn_module_at(ModuleKind::Vcf, None);
        let m_adsr = state.spawn_module_at(ModuleKind::Adsr, None);
        let m_vca = state.spawn_module_at(ModuleKind::Vca, None);
        let m_out = state.spawn_module_at(ModuleKind::AudioOutput, None);

        if let (Some(s_midi), Some(s_vco), Some(s_vcf), Some(s_adsr), Some(s_vca), Some(s_out)) =
            (m_midi, m_vco, m_vcf, m_adsr, m_vca, m_out)
        {
            state.connect_cable(LogicalCable {
                src_slot: s_midi,
                src_port: 0, // V/OCT
                dst_slot: s_vco,
                dst_port: 0, // V/OCT
            });
            state.connect_cable(LogicalCable {
                src_slot: s_midi,
                src_port: 1, // GATE
                dst_slot: s_adsr,
                dst_port: 0, // GATE
            });
            state.connect_cable(LogicalCable {
                src_slot: s_vco,
                src_port: 2, // SAW
                dst_slot: s_vcf,
                dst_port: 0, // IN
            });
            state.connect_cable(LogicalCable {
                src_slot: s_vcf,
                src_port: 0, // LP24
                dst_slot: s_vca,
                dst_port: 0, // IN
            });
            state.connect_cable(LogicalCable {
                src_slot: s_adsr,
                src_port: 0, // ENV
                dst_slot: s_vca,
                dst_port: 1, // CV
            });
            state.connect_cable(LogicalCable {
                src_slot: s_vca,
                src_port: 0, // OUT
                dst_slot: s_out,
                dst_port: 0, // IN L (normalisé mono vers R)
            });
        }

        state
    }

    pub fn collect_garbage(&mut self) {
        while let Ok(item) = self.garbage_rx.pop() {
            drop(item);
        }
    }

    pub fn poll_telemetry(&mut self) {
        self.latest_telemetry = *self.telemetry_rx.read();
    }

    fn allocate_slot(&self) -> Option<usize> {
        (0..MAX_MODULES).find(|&s| !self.modules.iter().any(|m| m.slot_id == s))
    }

    /// Crée et insère un module à la position X souhaitée (ou à la fin du rack si `None`).
    pub fn spawn_module_at(&mut self, kind: ModuleKind, target_x: Option<f32>) -> Option<usize> {
        let slot_id = self.allocate_slot()?;
        let x = target_x.unwrap_or_else(|| {
            self.modules
                .last()
                .map(|m| m.x + m.kind.width() + MODULE_GAP)
                .unwrap_or(RACK_LEFT_MARGIN)
        });

        let (dsp_box, visual): (Box<dyn DspModule>, ModuleVisual) = match kind {
            ModuleKind::MidiToCv => {
                let dsp = Box::new(MidiToCvModule::new());
                let ports = vec![
                    UiPort { name: "V/OCT", index: 0, direction: PortDirection::Output, local_x: 42.0, local_y: 255.0 },
                    UiPort { name: "GATE",  index: 1, direction: PortDirection::Output, local_x: 102.0, local_y: 255.0 },
                    UiPort { name: "TRIG",  index: 2, direction: PortDirection::Output, local_x: 42.0, local_y: 330.0 },
                    UiPort { name: "VEL",   index: 3, direction: PortDirection::Output, local_x: 102.0, local_y: 330.0 },
                    UiPort { name: "BEND",  index: 4, direction: PortDirection::Output, local_x: 72.0, local_y: 405.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs: Vec::new(),
                        dsp_params: ModuleParamHandle::None,
                    },
                )
            }
            ModuleKind::Vco => {
                let params = VcoParams::new();
                let dsp = Box::new(VcoModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "COARSE", local_x: 54.0,  local_y: 162.0, radius: 18.0, min: -24.0, max: 24.0, default: 0.0,  style: KnobDisplayStyle::Semitones, param: Arc::new(AtomicF32::new(0.0)) },
                    UiKnob { label: "FINE",   local_x: 132.0, local_y: 162.0, radius: 16.0, min: -1.0,  max: 1.0,  default: 0.0,  style: KnobDisplayStyle::Semitones, param: Arc::new(AtomicF32::new(0.0)) },
                    UiKnob { label: "FM",     local_x: 54.0,  local_y: 228.0, radius: 16.0, min: 0.0,   max: 1.0,  default: 0.25, style: KnobDisplayStyle::Percent,   param: Arc::new(AtomicF32::new(0.25)) },
                    UiKnob { label: "PWM",    local_x: 132.0, local_y: 228.0, radius: 16.0, min: 0.05,  max: 0.95, default: 0.5,  style: KnobDisplayStyle::Percent,   param: Arc::new(AtomicF32::new(0.5)) },
                ];
                let ports = vec![
                    UiPort { name: "V/OCT", index: 0, direction: PortDirection::Input,  local_x: 42.0,  local_y: 300.0 },
                    UiPort { name: "FM",    index: 1, direction: PortDirection::Input,  local_x: 93.0,  local_y: 300.0 },
                    UiPort { name: "PWM",   index: 2, direction: PortDirection::Input,  local_x: 144.0, local_y: 300.0 },
                    UiPort { name: "SIN",   index: 0, direction: PortDirection::Output, local_x: 46.0,  local_y: 368.0 },
                    UiPort { name: "TRI",   index: 1, direction: PortDirection::Output, local_x: 140.0, local_y: 368.0 },
                    UiPort { name: "SAW",   index: 2, direction: PortDirection::Output, local_x: 46.0,  local_y: 432.0 },
                    UiPort { name: "SQR",   index: 3, direction: PortDirection::Output, local_x: 140.0, local_y: 432.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Vco(params),
                    },
                )
            }
            ModuleKind::Vcf => {
                let params = VcfParams::new();
                let dsp = Box::new(VcfModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "CUTOFF", local_x: 54.0,  local_y: 162.0, radius: 19.0, min: 20.0, max: 16000.0, default: 1400.0, style: KnobDisplayStyle::Hertz,   param: Arc::new(AtomicF32::new(1400.0)) },
                    UiKnob { label: "RES",    local_x: 132.0, local_y: 162.0, radius: 17.0, min: 0.0,  max: 0.98,    default: 0.25,   style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.25)) },
                    UiKnob { label: "DRIVE",  local_x: 54.0,  local_y: 228.0, radius: 15.0, min: 0.5,  max: 4.0,     default: 1.2,    style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(1.2)) },
                    UiKnob { label: "CV AMT", local_x: 132.0, local_y: 228.0, radius: 15.0, min: -1.0, max: 1.0,     default: 0.5,    style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.5)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",     index: 0, direction: PortDirection::Input,  local_x: 42.0,  local_y: 305.0 },
                    UiPort { name: "CUT CV", index: 1, direction: PortDirection::Input,  local_x: 93.0,  local_y: 305.0 },
                    UiPort { name: "RES CV", index: 2, direction: PortDirection::Input,  local_x: 144.0, local_y: 305.0 },
                    UiPort { name: "LP 24",  index: 0, direction: PortDirection::Output, local_x: 42.0,  local_y: 395.0 },
                    UiPort { name: "BP 12",  index: 1, direction: PortDirection::Output, local_x: 93.0,  local_y: 395.0 },
                    UiPort { name: "HP 24",  index: 2, direction: PortDirection::Output, local_x: 144.0, local_y: 395.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Vcf(params),
                    },
                )
            }
            ModuleKind::Adsr => {
                let params = AdsrParams::new();
                let dsp = Box::new(AdsrModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "ATK", local_x: 52.0,  local_y: 162.0, radius: 17.0, min: 0.001, max: 2.0, default: 0.02, style: KnobDisplayStyle::Milliseconds, param: Arc::new(AtomicF32::new(0.02)) },
                    UiKnob { label: "DEC", local_x: 124.0, local_y: 162.0, radius: 17.0, min: 0.005, max: 3.0, default: 0.28, style: KnobDisplayStyle::Milliseconds, param: Arc::new(AtomicF32::new(0.28)) },
                    UiKnob { label: "SUS", local_x: 52.0,  local_y: 232.0, radius: 17.0, min: 0.0,   max: 1.0, default: 0.65, style: KnobDisplayStyle::Percent,      param: Arc::new(AtomicF32::new(0.65)) },
                    UiKnob { label: "REL", local_x: 124.0, local_y: 232.0, radius: 17.0, min: 0.005, max: 4.0, default: 0.35, style: KnobDisplayStyle::Milliseconds, param: Arc::new(AtomicF32::new(0.35)) },
                ];
                let ports = vec![
                    UiPort { name: "GATE", index: 0, direction: PortDirection::Input,  local_x: 52.0,  local_y: 315.0 },
                    UiPort { name: "TRIG", index: 1, direction: PortDirection::Input,  local_x: 124.0, local_y: 315.0 },
                    UiPort { name: "ENV",  index: 0, direction: PortDirection::Output, local_x: 52.0,  local_y: 398.0 },
                    UiPort { name: "INV",  index: 1, direction: PortDirection::Output, local_x: 124.0, local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Adsr(params),
                    },
                )
            }
            ModuleKind::Lfo => {
                let params = LfoParams::new();
                let dsp = Box::new(LfoModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "RATE", local_x: 82.0,  local_y: 162.0, radius: 19.0, min: 0.05, max: 40.0, default: 1.8, style: KnobDisplayStyle::Hertz,    param: Arc::new(AtomicF32::new(1.8)) },
                    UiKnob { label: "WAVE", local_x: 48.0,  local_y: 232.0, radius: 16.0, min: 0.0,  max: 4.0,  default: 0.0, style: KnobDisplayStyle::LfoWave,  param: Arc::new(AtomicF32::new(0.0)) },
                    UiKnob { label: "MODE", local_x: 116.0, local_y: 232.0, radius: 16.0, min: 0.0,  max: 1.0,  default: 0.0, style: KnobDisplayStyle::Polarity, param: Arc::new(AtomicF32::new(0.0)) },
                ];
                let ports = vec![
                    UiPort { name: "RST",     index: 0, direction: PortDirection::Input,  local_x: 48.0,  local_y: 315.0 },
                    UiPort { name: "RATE CV", index: 1, direction: PortDirection::Input,  local_x: 116.0, local_y: 315.0 },
                    UiPort { name: "OUT",     index: 0, direction: PortDirection::Output, local_x: 38.0,  local_y: 398.0 },
                    UiPort { name: "S&H",     index: 1, direction: PortDirection::Output, local_x: 82.0,  local_y: 398.0 },
                    UiPort { name: "INV",     index: 2, direction: PortDirection::Output, local_x: 126.0, local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Lfo(params),
                    },
                )
            }
            ModuleKind::Vca => {
                let params = VcaParams::new();
                let dsp = Box::new(VcaModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "LEVEL",  local_x: 74.0,  local_y: 162.0, radius: 18.0, min: 0.0, max: 1.0, default: 0.0,  style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.0)) },
                    UiKnob { label: "CV AMT", local_x: 44.0,  local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 1.0,  style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(1.0)) },
                    UiKnob { label: "CURVE",  local_x: 104.0, local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 0.35, style: KnobDisplayStyle::LinExp,  param: Arc::new(AtomicF32::new(0.35)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",  index: 0, direction: PortDirection::Input,  local_x: 46.0,  local_y: 315.0 },
                    UiPort { name: "CV",  index: 1, direction: PortDirection::Input,  local_x: 102.0, local_y: 315.0 },
                    UiPort { name: "OUT", index: 0, direction: PortDirection::Output, local_x: 74.0,  local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Vca(params),
                    },
                )
            }
            ModuleKind::AudioOutput => {
                let params = AudioOutParams::new();
                let dsp = Box::new(AudioOutputModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "MASTER", local_x: 78.0, local_y: 170.0, radius: 22.0, min: 0.0, max: 1.5, default: 0.85, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.85)) },
                ];
                let ports = vec![
                    UiPort { name: "IN L", index: 0, direction: PortDirection::Input, local_x: 48.0,  local_y: 398.0 },
                    UiPort { name: "IN R", index: 1, direction: PortDirection::Input, local_x: 108.0, local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::AudioOut(params),
                    },
                )
            }
        };

        let _ = self.cmd_tx.push(AudioCommand::InsertModule {
            slot_id,
            module: dsp_box,
        });

        self.modules.push(visual);
        self.pack_modules_horizontally();
        self.recompile_and_push_plan();
        Some(slot_id)
    }

    pub fn remove_module(&mut self, slot_id: usize) {
        self.modules.retain(|m| m.slot_id != slot_id);
        self.cables
            .retain(|c| c.logical.src_slot != slot_id && c.logical.dst_slot != slot_id);

        let _ = self.cmd_tx.push(AudioCommand::RemoveModule { slot_id });
        self.pack_modules_horizontally();
        self.recompile_and_push_plan();
    }

    pub fn pack_modules_horizontally(&mut self) {
        self.modules.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut cursor_x = RACK_LEFT_MARGIN;
        for m in &mut self.modules {
            m.x = cursor_x;
            cursor_x += m.kind.width() + MODULE_GAP;
        }
    }

    pub fn total_rack_width(&self) -> f32 {
        self.modules
            .last()
            .map(|m| m.x + m.kind.width() + 80.0)
            .unwrap_or(600.0)
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
            }
        }
    }

    pub fn connect_cable(&mut self, logical: LogicalCable) -> bool {
        if self.cables.iter().any(|c| c.logical == logical) {
            return false;
        }

        let start = self
            .modules
            .iter()
            .find(|m| m.slot_id == logical.src_slot)
            .and_then(|m| m.port_world_pos(PortDirection::Output, logical.src_port, self.scroll_x));

        let end = self
            .modules
            .iter()
            .find(|m| m.slot_id == logical.dst_slot)
            .and_then(|m| m.port_world_pos(PortDirection::Input, logical.dst_port, self.scroll_x));

        if let (Some(p0), Some(p1)) = (start, end) {
            let physics = CablePhysics::new(p0, p1, self.next_color_idx);
            self.next_color_idx += 1;
            self.cables.push(PatchCableVisual { logical, physics });
            self.recompile_and_push_plan();
            true
        } else {
            false
        }
    }

    pub fn disconnect_cable_at_port(
        &mut self,
        slot_id: usize,
        dir: PortDirection,
        port_idx: usize,
    ) -> Option<LogicalCable> {
        let found_idx = self.cables.iter().rposition(|c| match dir {
            PortDirection::Output => c.logical.src_slot == slot_id && c.logical.src_port == port_idx,
            PortDirection::Input => c.logical.dst_slot == slot_id && c.logical.dst_port == port_idx,
        });

        if let Some(idx) = found_idx {
            let removed = self.cables.remove(idx);
            self.recompile_and_push_plan();
            Some(removed.logical)
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