use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

// ============================================================================
// 1. OVERDRIVE (Saturation chaude asymétrique + Tone + Dry/Wet)
// ============================================================================
pub struct OverdriveParams {
    pub drive: AtomicF32,
    pub tone: AtomicF32,
    pub mix: AtomicF32,
}

impl OverdriveParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            drive: AtomicF32::new(3.5),
            tone: AtomicF32::new(4500.0),
            mix: AtomicF32::new(1.0),
        })
    }
}

pub struct OverdriveModule {
    params: Arc<OverdriveParams>,
    lp_state: f32,
}

impl OverdriveModule {
    pub fn new(params: Arc<OverdriveParams>) -> Self {
        Self {
            params,
            lp_state: 0.0,
        }
    }
}

impl DspModule for OverdriveModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let dry = inputs[0] * 0.2; // ±5V -> ±1.0 interne
        let drive_cv = (inputs[1] * 0.2).max(-0.5);
        let drive = (self.params.drive.load() * (1.0 + drive_cv * 2.0)).clamp(1.0, 20.0);
        let tone_hz = self.params.tone.load().clamp(300.0, 14000.0);
        let mix = self.params.mix.load().clamp(0.0, 1.0);

        // Saturation asymétrique chaude (harmoniques paires + impaires)
        let x = dry * drive;
        let sat = if x >= 0.0 {
            (x * 1.15).tanh()
        } else {
            (x * 0.85).tanh() * 0.92
        };

        // Filtre Tone 1-pôle
        let alpha = (TAU * tone_hz / sample_rate).clamp(0.001, 0.99);
        self.lp_state += alpha * (sat - self.lp_state);

        let wet = dry * (1.0 - mix) + self.lp_state * mix;
        outputs[0] = (wet * 5.0).clamp(-10.0, 10.0);
    }
}

// ============================================================================
// 2. DISTORTION (Hard-clipping + Wavefolder + Filtre Post-Gain)
// ============================================================================
pub struct DistortionParams {
    pub gain: AtomicF32,
    pub fold: AtomicF32,
    pub tone: AtomicF32,
    pub mix: AtomicF32,
}

impl DistortionParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            gain: AtomicF32::new(6.0),
            fold: AtomicF32::new(0.35),
            tone: AtomicF32::new(3200.0),
            mix: AtomicF32::new(1.0),
        })
    }
}

pub struct DistortionModule {
    params: Arc<DistortionParams>,
    lp_state: f32,
}

impl DistortionModule {
    pub fn new(params: Arc<DistortionParams>) -> Self {
        Self {
            params,
            lp_state: 0.0,
        }
    }
}

impl DspModule for DistortionModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let dry = inputs[0] * 0.2;
        let cv = (inputs[1] * 0.2).max(-0.5);
        let gain = (self.params.gain.load() * (1.0 + cv * 2.0)).clamp(1.0, 30.0);
        let fold = (self.params.fold.load() + cv * 0.5).clamp(0.0, 1.0);
        let tone_hz = self.params.tone.load().clamp(250.0, 12000.0);
        let mix = self.params.mix.load().clamp(0.0, 1.0);

        let driven = dry * gain;
        let hard = driven.clamp(-1.0, 1.0);
        let folded = (driven * PI * 0.75).sin();
        let combined = hard * (1.0 - fold) + folded * fold;

        let alpha = (TAU * tone_hz / sample_rate).clamp(0.001, 0.99);
        self.lp_state += alpha * (combined - self.lp_state);

        let wet = dry * (1.0 - mix) + self.lp_state * mix;
        outputs[0] = (wet * 5.0).clamp(-10.0, 10.0);
    }
}

// ============================================================================
// 3. CHORUS STÉRÉO (Double ligne à retard BBD modulée en quadrature)
// ============================================================================
const CHORUS_BUF_LEN: usize = 4096;

pub struct ChorusParams {
    pub rate: AtomicF32,
    pub depth: AtomicF32,
    pub feedback: AtomicF32,
    pub mix: AtomicF32,
}

impl ChorusParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rate: AtomicF32::new(1.2),
            depth: AtomicF32::new(0.5),
            feedback: AtomicF32::new(0.2),
            mix: AtomicF32::new(0.5),
        })
    }
}

pub struct ChorusModule {
    params: Arc<ChorusParams>,
    buffer: [f32; CHORUS_BUF_LEN],
    write_idx: usize,
    lfo_phase: f32,
}

impl ChorusModule {
    pub fn new(params: Arc<ChorusParams>) -> Self {
        Self {
            params,
            buffer: [0.0; CHORUS_BUF_LEN],
            write_idx: 0,
            lfo_phase: 0.0,
        }
    }

    #[inline(always)]
    fn read_linear(&self, delay_samples: f32) -> f32 {
        let len = CHORUS_BUF_LEN as f32;
        let mut read_pos = (self.write_idx as f32) - delay_samples;
        while read_pos < 0.0 {
            read_pos += len;
        }
        let i0 = (read_pos as usize) % CHORUS_BUF_LEN;
        let i1 = (i0 + 1) % CHORUS_BUF_LEN;
        let frac = read_pos - read_pos.floor();
        self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac
    }
}

impl DspModule for ChorusModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let dry = inputs[0] * 0.2;
        let rate_mod = 2.0f32.powf(inputs[1].clamp(-2.0, 3.0));
        let rate = (self.params.rate.load() * rate_mod).clamp(0.05, 12.0);
        let depth = self.params.depth.load().clamp(0.0, 1.0);
        let feedback = self.params.feedback.load().clamp(0.0, 0.85);
        let mix = self.params.mix.load().clamp(0.0, 1.0);

        let mod_l = (self.lfo_phase * TAU).sin();
        let mod_r = (self.lfo_phase * TAU).cos(); // Quadrature 90° pour élargissement stéréo

        let base_delay_ms = 11.0;
        let swing_ms = 6.5 * depth;
        let delay_l = ((base_delay_ms + mod_l * swing_ms) * 0.001 * sample_rate)
            .clamp(2.0, (CHORUS_BUF_LEN - 4) as f32);
        let delay_r = ((base_delay_ms + mod_r * swing_ms) * 0.001 * sample_rate)
            .clamp(2.0, (CHORUS_BUF_LEN - 4) as f32);

        let wet_l = self.read_linear(delay_l);
        let wet_r = self.read_linear(delay_r);

        let fb_sample = (dry + (wet_l + wet_r) * 0.5 * feedback).tanh();
        self.buffer[self.write_idx] = fb_sample;
        self.write_idx = (self.write_idx + 1) % CHORUS_BUF_LEN;

        self.lfo_phase = (self.lfo_phase + rate / sample_rate).fract();

        let out_l = dry * (1.0 - mix) + wet_l * mix;
        let out_r = dry * (1.0 - mix) + wet_r * mix;

        outputs[0] = (out_l * 5.0).clamp(-10.0, 10.0);
        outputs[1] = (out_r * 5.0).clamp(-10.0, 10.0);
    }
}

// ============================================================================
// 4. PHASER STÉRÉO 6 ÉTAGES (Cascade de filtres passe-tout + Feedback)
// ============================================================================
pub struct PhaserParams {
    pub rate: AtomicF32,
    pub depth: AtomicF32,
    pub feedback: AtomicF32,
    pub mix: AtomicF32,
}

impl PhaserParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rate: AtomicF32::new(0.6),
            depth: AtomicF32::new(0.65),
            feedback: AtomicF32::new(0.45),
            mix: AtomicF32::new(0.5),
        })
    }
}

pub struct PhaserModule {
    params: Arc<PhaserParams>,
    ap_l: [f32; 6],
    ap_r: [f32; 6],
    fb_l: f32,
    fb_r: f32,
    lfo_phase: f32,
}

impl PhaserModule {
    pub fn new(params: Arc<PhaserParams>) -> Self {
        Self {
            params,
            ap_l: [0.0; 6],
            ap_r: [0.0; 6],
            fb_l: 0.0,
            fb_r: 0.0,
            lfo_phase: 0.0,
        }
    }

    #[inline(always)]
    fn process_allpass_chain(state: &mut [f32; 6], mut x: f32, coeff: f32) -> f32 {
        for s in state.iter_mut() {
            let y = -x * coeff + *s;
            *s = x + y * coeff;
            x = y;
        }
        x
    }
}

impl DspModule for PhaserModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let dry = inputs[0] * 0.2;
        let rate_mod = 2.0f32.powf(inputs[1].clamp(-2.0, 3.0));
        let rate = (self.params.rate.load() * rate_mod).clamp(0.05, 12.0);
        let depth = self.params.depth.load().clamp(0.0, 1.0);
        let feedback = self.params.feedback.load().clamp(-0.85, 0.85);
        let mix = self.params.mix.load().clamp(0.0, 1.0);

        let lfo_l = 0.5 + 0.5 * (self.lfo_phase * TAU).sin();
        let lfo_r = 0.5 + 0.5 * ((self.lfo_phase + 0.25) * TAU).sin();

        let min_f = 220.0f32;
        let max_f = 2800.0f32;
        let freq_l = min_f * (max_f / min_f).powf(lfo_l * depth);
        let freq_r = min_f * (max_f / min_f).powf(lfo_r * depth);

        let w_l = (PI * freq_l / sample_rate).min(1.2);
        let coeff_l = (1.0 - w_l) / (1.0 + w_l);

        let w_r = (PI * freq_r / sample_rate).min(1.2);
        let coeff_r = (1.0 - w_r) / (1.0 + w_r);

        let in_l = (dry + self.fb_l * feedback).tanh();
        let in_r = (dry + self.fb_r * feedback).tanh();

        let wet_l = Self::process_allpass_chain(&mut self.ap_l, in_l, coeff_l);
        let wet_r = Self::process_allpass_chain(&mut self.ap_r, in_r, coeff_r);

        self.fb_l = wet_l;
        self.fb_r = wet_r;
        self.lfo_phase = (self.lfo_phase + rate / sample_rate).fract();

        let out_l = dry * (1.0 - mix) + wet_l * mix;
        let out_r = dry * (1.0 - mix) + wet_r * mix;

        outputs[0] = (out_l * 5.0).clamp(-10.0, 10.0);
        outputs[1] = (out_r * 5.0).clamp(-10.0, 10.0);
    }
}