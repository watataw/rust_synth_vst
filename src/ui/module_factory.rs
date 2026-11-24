use super::state::{
    KnobDisplayStyle, ModuleKind, ModuleParamHandle, ModuleVisual, PortDirection, RackUiState,
    UiKnob, UiPort, MODULE_GAP, RACK_LEFT_MARGIN,
};
use crate::dsp::{
    AdsrModule, AdsrParams, AudioOutParams, AudioOutputModule, ChorusModule, ChorusParams,
    DistortionModule, DistortionParams, LfoModule, LfoParams, MidiToCvModule, OverdriveModule,
    OverdriveParams, PhaserModule, PhaserParams, VcaModule, VcaParams, VcfModule, VcfParams,
    VcoModule, VcoParams,
};
use crate::engine::{AtomicF32, AudioCommand, DspModule};
use std::sync::Arc;

impl RackUiState {
    pub fn spawn_module_at(
        &mut self,
        kind: ModuleKind,
        target_row: usize,
        target_x: Option<f32>,
    ) -> Option<usize> {
        let slot_id = self.allocate_slot()?;
        let x = target_x.unwrap_or_else(|| {
            self.modules
                .iter()
                .filter(|m| m.row == target_row)
                .map(|m| m.x + m.kind.width() + MODULE_GAP)
                .fold(RACK_LEFT_MARGIN, f32::max)
        });

        let (dsp_box, visual): (Box<dyn DspModule>, ModuleVisual) = match kind {
            ModuleKind::MidiToCv => {
                let dsp = Box::new(MidiToCvModule::new());
                let ports = vec![
                    UiPort { name: "V/OCT", index: 0, direction: PortDirection::Output, local_x: 42.0,  local_y: 255.0 },
                    UiPort { name: "GATE",  index: 1, direction: PortDirection::Output, local_x: 102.0, local_y: 255.0 },
                    UiPort { name: "TRIG",  index: 2, direction: PortDirection::Output, local_x: 42.0,  local_y: 330.0 },
                    UiPort { name: "VEL",   index: 3, direction: PortDirection::Output, local_x: 102.0, local_y: 330.0 },
                    UiPort { name: "BEND",  index: 4, direction: PortDirection::Output, local_x: 72.0,  local_y: 405.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        row: target_row,
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
                        row: target_row,
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
                        row: target_row,
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
                        row: target_row,
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
                        row: target_row,
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
                        row: target_row,
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
                        row: target_row,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::AudioOut(params),
                    },
                )
            }
            ModuleKind::Overdrive => {
                let params = OverdriveParams::new();
                let dsp = Box::new(OverdriveModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "DRIVE", local_x: 78.0,  local_y: 162.0, radius: 18.0, min: 0.0, max: 1.0, default: 0.35, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.35)) },
                    UiKnob { label: "TONE",  local_x: 46.0,  local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 0.6,  style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.6)) },
                    UiKnob { label: "MIX",   local_x: 110.0, local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 1.0,  style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(1.0)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",     index: 0, direction: PortDirection::Input,  local_x: 48.0,  local_y: 315.0 },
                    UiPort { name: "DRV CV", index: 1, direction: PortDirection::Input,  local_x: 108.0, local_y: 315.0 },
                    UiPort { name: "OUT",    index: 0, direction: PortDirection::Output, local_x: 78.0,  local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        row: target_row,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Overdrive(params),
                    },
                )
            }
            ModuleKind::Distortion => {
                let params = DistortionParams::new();
                let dsp = Box::new(DistortionModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "DRIVE", local_x: 78.0,  local_y: 162.0, radius: 18.0, min: 0.0, max: 1.0, default: 0.5, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.5)) },
                    UiKnob { label: "FOLD",  local_x: 46.0,  local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 0.3, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.3)) },
                    UiKnob { label: "MIX",   local_x: 110.0, local_y: 232.0, radius: 15.0, min: 0.0, max: 1.0, default: 1.0, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(1.0)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",     index: 0, direction: PortDirection::Input,  local_x: 48.0,  local_y: 315.0 },
                    UiPort { name: "FLD CV", index: 1, direction: PortDirection::Input,  local_x: 108.0, local_y: 315.0 },
                    UiPort { name: "OUT",    index: 0, direction: PortDirection::Output, local_x: 78.0,  local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        row: target_row,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Distortion(params),
                    },
                )
            }
            ModuleKind::Chorus => {
                let params = ChorusParams::new();
                let dsp = Box::new(ChorusModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "RATE",  local_x: 82.0,  local_y: 162.0, radius: 18.0, min: 0.05, max: 5.0, default: 0.6, style: KnobDisplayStyle::Hertz,   param: Arc::new(AtomicF32::new(0.6)) },
                    UiKnob { label: "DEPTH", local_x: 48.0,  local_y: 232.0, radius: 15.0, min: 0.0,  max: 1.0, default: 0.6, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.6)) },
                    UiKnob { label: "MIX",   local_x: 116.0, local_y: 232.0, radius: 15.0, min: 0.0,  max: 1.0, default: 0.5, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.5)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",      index: 0, direction: PortDirection::Input,  local_x: 48.0,  local_y: 315.0 },
                    UiPort { name: "RATE CV", index: 1, direction: PortDirection::Input,  local_x: 116.0, local_y: 315.0 },
                    UiPort { name: "OUT L",   index: 0, direction: PortDirection::Output, local_x: 48.0,  local_y: 398.0 },
                    UiPort { name: "OUT R",   index: 1, direction: PortDirection::Output, local_x: 116.0, local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        row: target_row,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Chorus(params),
                    },
                )
            }
            ModuleKind::Phaser => {
                let params = PhaserParams::new();
                let dsp = Box::new(PhaserModule::new(params.clone()));
                let knobs = vec![
                    UiKnob { label: "RATE",  local_x: 82.0,  local_y: 162.0, radius: 18.0, min: 0.05, max: 6.0,  default: 0.45, style: KnobDisplayStyle::Hertz,   param: Arc::new(AtomicF32::new(0.45)) },
                    UiKnob { label: "DEPTH", local_x: 48.0,  local_y: 232.0, radius: 15.0, min: 0.0,  max: 1.0,  default: 0.7,  style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.7)) },
                    UiKnob { label: "FDBK",  local_x: 116.0, local_y: 232.0, radius: 15.0, min: 0.0,  max: 0.92, default: 0.45, style: KnobDisplayStyle::Percent, param: Arc::new(AtomicF32::new(0.45)) },
                ];
                let ports = vec![
                    UiPort { name: "IN",    index: 0, direction: PortDirection::Input,  local_x: 48.0,  local_y: 315.0 },
                    UiPort { name: "SWEEP", index: 1, direction: PortDirection::Input,  local_x: 116.0, local_y: 315.0 },
                    UiPort { name: "OUT",   index: 0, direction: PortDirection::Output, local_x: 82.0,  local_y: 398.0 },
                ];
                (
                    dsp,
                    ModuleVisual {
                        slot_id,
                        kind,
                        row: target_row,
                        x,
                        ports,
                        knobs,
                        dsp_params: ModuleParamHandle::Phaser(params),
                    },
                )
            }
        };

        let _ = self.cmd_tx.push(AudioCommand::InsertModule {
            slot_id,
            module: dsp_box,
        });

        self.modules.push(visual);
        self.pack_modules();
        self.recompile_and_push_plan();
        Some(slot_id)
    }
}