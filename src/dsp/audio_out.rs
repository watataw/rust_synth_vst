use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::sync::Arc;

pub struct AudioOutParams {
    /// Volume master (0.0 à 1.5)
    pub master_gain: AtomicF32,
}

impl AudioOutParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            master_gain: AtomicF32::new(0.8),
        })
    }
}

/// Module 7 : Sortie Audio Stéréo DAW avec normalisation mono et soft-clipper tanh.
/// Entrées : 0: Left In, 1: Right In (si non branchée, copie Left In)
/// Sorties : 0: Vumètre L (télémétrie), 1: Vumètre R (télémétrie)
pub struct AudioOutputModule {
    params: Arc<AudioOutParams>,
    last_stereo: (f32, f32),
}

impl AudioOutputModule {
    pub fn new(params: Arc<AudioOutParams>) -> Self {
        Self {
            params,
            last_stereo: (0.0, 0.0),
        }
    }

    /// Limiteur doux analogique autour de ±5.0V Eurorack pour protéger les oreilles.
    #[inline(always)]
    fn soft_clip_eurorack(volts: f32) -> f32 {
        5.0 * (volts / 5.0).tanh()
    }
}

impl DspModule for AudioOutputModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        connected_inputs: u8,
        _sample_rate: f32,
    ) {
        let gain = self.params.master_gain.load();
        let left_in = inputs[0];
        // Normalisation mono : si le bit 1 (Right In) n'est pas connecté, on duplique Left
        let right_connected = (connected_inputs & (1 << 1)) != 0;
        let right_in = if right_connected { inputs[1] } else { left_in };

        let out_l = Self::soft_clip_eurorack(left_in * gain);
        let out_r = Self::soft_clip_eurorack(right_in * gain);

        self.last_stereo = (out_l, out_r);

        // Exposé en sortie uniquement pour alimenter le vumètre LED de la télémétrie UI
        outputs[0] = out_l;
        outputs[1] = out_r;
    }

    fn take_audio_out(&mut self) -> Option<(f32, f32)> {
        Some(self.last_stereo)
    }
}