use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::sync::Arc;

pub struct AdsrParams {
    /// Temps d'attaque en secondes (0.001 .. 2.0)
    pub attack: AtomicF32,
    /// Temps de déclin en secondes (0.005 .. 3.0)
    pub decay: AtomicF32,
    /// Niveau de maintien (0.0 .. 1.0)
    pub sustain: AtomicF32,
    /// Temps de relâchement en secondes (0.005 .. 4.0)
    pub release: AtomicF32,
}

impl AdsrParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            attack: AtomicF32::new(0.02),
            decay: AtomicF32::new(0.28),
            sustain: AtomicF32::new(0.65),
            release: AtomicF32::new(0.35),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Module 4 : Générateur d'enveloppe ADSR (0.0V à +5.0V).
/// Entrées : 0: GATE, 1: TRIG
/// Sorties : 0: ENV (0..+5V), 1: INV (+5..0V)
pub struct AdsrModule {
    params: Arc<AdsrParams>,
    stage: EnvStage,
    level: f32, // Normalisé 0.0 .. 1.0
    prev_gate_high: bool,
    prev_trig_high: bool,
}

impl AdsrModule {
    pub fn new(params: Arc<AdsrParams>) -> Self {
        Self {
            params,
            stage: EnvStage::Idle,
            level: 0.0,
            prev_gate_high: false,
            prev_trig_high: false,
        }
    }

    #[inline(always)]
    fn time_to_coeff(time_sec: f32, sample_rate: f32) -> f32 {
        let samples = (time_sec.max(0.001) * sample_rate).max(1.0);
        1.0 - (-1.0 / samples).exp()
    }
}

impl DspModule for AdsrModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let gate_high = inputs[0] > 1.0;
        let trig_high = inputs[1] > 1.0;

        let rising_gate = gate_high && !self.prev_gate_high;
        let rising_trig = trig_high && !self.prev_trig_high;

        if rising_gate || rising_trig {
            self.stage = EnvStage::Attack;
        } else if !gate_high && self.prev_gate_high && self.stage != EnvStage::Idle {
            self.stage = EnvStage::Release;
        }

        self.prev_gate_high = gate_high;
        self.prev_trig_high = trig_high;

        match self.stage {
            EnvStage::Idle => {
                self.level = 0.0;
            }
            EnvStage::Attack => {
                let atk = self.params.attack.load().max(0.001);
                let step = 1.0 / (atk * sample_rate);
                self.level += step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = if gate_high {
                        EnvStage::Decay
                    } else {
                        EnvStage::Release
                    };
                }
            }
            EnvStage::Decay => {
                let sus = self.params.sustain.load().clamp(0.0, 1.0);
                let coeff = Self::time_to_coeff(self.params.decay.load() * 0.35, sample_rate);
                self.level += (sus - self.level) * coeff;
                if (self.level - sus).abs() < 0.001 {
                    self.level = sus;
                    self.stage = EnvStage::Sustain;
                }
            }
            EnvStage::Sustain => {
                self.level = self.params.sustain.load().clamp(0.0, 1.0);
                if !gate_high {
                    self.stage = EnvStage::Release;
                }
            }
            EnvStage::Release => {
                let coeff = Self::time_to_coeff(self.params.release.load() * 0.35, sample_rate);
                self.level += (0.0 - self.level) * coeff;
                if self.level < 0.0005 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
            }
        }

        let env_volts = self.level * 5.0;
        outputs[0] = env_volts;
        outputs[1] = 5.0 - env_volts;
    }
}