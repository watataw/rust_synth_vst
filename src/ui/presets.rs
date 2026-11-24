use super::state::ModuleKind;

#[derive(Clone, Copy)]
pub struct PresetModuleSpec {
    pub kind: ModuleKind,
    pub row: usize,
    pub knobs: &'static [f32],
}

#[derive(Clone, Copy)]
pub struct PresetCableSpec {
    pub src_mod_idx: usize,
    pub src_port: usize,
    pub dst_mod_idx: usize,
    pub dst_port: usize,
}

#[derive(Clone, Copy)]
pub struct SynthPreset {
    pub modules: &'static [PresetModuleSpec],
    pub cables: &'static [PresetCableSpec],
}

pub const SYNTH_PRESETS: [SynthPreset; 5] = [
    SynthPreset {
        modules: &[
            PresetModuleSpec { kind: ModuleKind::MidiToCv, row: 0, knobs: &[] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[0.0, 0.0, 0.2, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Vcf, row: 0, knobs: &[1850.0, 0.28, 1.3, 0.55] },
            PresetModuleSpec { kind: ModuleKind::Adsr, row: 0, knobs: &[0.015, 0.28, 0.65, 0.32] },
            PresetModuleSpec { kind: ModuleKind::Vca, row: 0, knobs: &[0.0, 1.0, 0.35] },
            PresetModuleSpec { kind: ModuleKind::AudioOutput, row: 0, knobs: &[0.85] },
        ],
        cables: &[
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 1, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 1, dst_mod_idx: 3, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 1, src_port: 2, dst_mod_idx: 2, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 2, src_port: 0, dst_mod_idx: 4, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 4, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 2, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 4, src_port: 0, dst_mod_idx: 5, dst_port: 0 },
        ],
    },
    SynthPreset {
        modules: &[
            PresetModuleSpec { kind: ModuleKind::MidiToCv, row: 0, knobs: &[] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[-12.0, 0.0, 0.15, 0.35] },
            PresetModuleSpec { kind: ModuleKind::Vcf, row: 0, knobs: &[380.0, 0.84, 2.6, 0.9] },
            PresetModuleSpec { kind: ModuleKind::Adsr, row: 0, knobs: &[0.004, 0.21, 0.12, 0.14] },
            PresetModuleSpec { kind: ModuleKind::Lfo, row: 0, knobs: &[0.45, 0.0, 0.0] },
            PresetModuleSpec { kind: ModuleKind::Vca, row: 0, knobs: &[0.0, 1.0, 0.65] },
            PresetModuleSpec { kind: ModuleKind::AudioOutput, row: 0, knobs: &[0.85] },
        ],
        cables: &[
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 1, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 1, dst_mod_idx: 3, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 1, src_port: 3, dst_mod_idx: 2, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 4, src_port: 0, dst_mod_idx: 1, dst_port: 2 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 2, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 2, src_port: 0, dst_mod_idx: 5, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 5, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 5, src_port: 0, dst_mod_idx: 6, dst_port: 0 },
        ],
    },
    SynthPreset {
        modules: &[
            PresetModuleSpec { kind: ModuleKind::MidiToCv, row: 0, knobs: &[] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[19.0, 0.04, 0.0, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[0.0, 0.0, 0.42, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Adsr, row: 0, knobs: &[0.002, 1.15, 0.0, 1.2] },
            PresetModuleSpec { kind: ModuleKind::Vca, row: 0, knobs: &[0.0, 1.0, 0.85] },
            PresetModuleSpec { kind: ModuleKind::AudioOutput, row: 0, knobs: &[0.85] },
        ],
        cables: &[
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 1, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 2, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 1, dst_mod_idx: 3, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 1, src_port: 0, dst_mod_idx: 2, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 2, src_port: 0, dst_mod_idx: 4, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 4, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 4, src_port: 0, dst_mod_idx: 5, dst_port: 0 },
        ],
    },
    SynthPreset {
        modules: &[
            PresetModuleSpec { kind: ModuleKind::MidiToCv, row: 0, knobs: &[] },
            PresetModuleSpec { kind: ModuleKind::Lfo, row: 0, knobs: &[6.5, 4.0, 1.0] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[-12.0, 0.0, 0.28, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Vcf, row: 0, knobs: &[850.0, 0.72, 1.6, 0.65] },
            PresetModuleSpec { kind: ModuleKind::Adsr, row: 0, knobs: &[0.06, 0.45, 0.75, 0.55] },
            PresetModuleSpec { kind: ModuleKind::Vca, row: 0, knobs: &[0.0, 1.0, 0.4] },
            PresetModuleSpec { kind: ModuleKind::AudioOutput, row: 0, knobs: &[0.85] },
        ],
        cables: &[
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 2, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 1, dst_mod_idx: 4, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 1, src_port: 1, dst_mod_idx: 2, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 1, src_port: 1, dst_mod_idx: 3, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 2, src_port: 2, dst_mod_idx: 3, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 3, src_port: 1, dst_mod_idx: 5, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 4, src_port: 0, dst_mod_idx: 5, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 5, src_port: 0, dst_mod_idx: 6, dst_port: 0 },
        ],
    },
    SynthPreset {
        modules: &[
            PresetModuleSpec { kind: ModuleKind::MidiToCv, row: 0, knobs: &[] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[0.0, -0.22, 0.1, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Vco, row: 0, knobs: &[0.0, 0.22, 0.1, 0.5] },
            PresetModuleSpec { kind: ModuleKind::Lfo, row: 0, knobs: &[0.32, 1.0, 0.0] },
            PresetModuleSpec { kind: ModuleKind::Vcf, row: 1, knobs: &[980.0, 0.22, 1.2, 0.45] },
            PresetModuleSpec { kind: ModuleKind::Adsr, row: 1, knobs: &[0.55, 0.85, 0.78, 1.35] },
            PresetModuleSpec { kind: ModuleKind::Vca, row: 1, knobs: &[0.0, 1.0, 0.5] },
            PresetModuleSpec { kind: ModuleKind::AudioOutput, row: 1, knobs: &[0.85] },
        ],
        cables: &[
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 1, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 0, dst_mod_idx: 2, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 0, src_port: 1, dst_mod_idx: 5, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 1, src_port: 2, dst_mod_idx: 4, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 2, src_port: 1, dst_mod_idx: 4, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 3, src_port: 0, dst_mod_idx: 4, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 4, src_port: 0, dst_mod_idx: 6, dst_port: 0 },
            PresetCableSpec { src_mod_idx: 5, src_port: 0, dst_mod_idx: 6, dst_port: 1 },
            PresetCableSpec { src_mod_idx: 6, src_port: 0, dst_mod_idx: 7, dst_port: 0 },
        ],
    },
];