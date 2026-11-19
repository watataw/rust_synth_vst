use crate::engine::{AtomicF32, DspModule, MAX_PORTS_PER_MODULE};
use std::f64::consts::TAU;
use std::sync::Arc;

/// Paramètres atomiques partagés entre les potentiomètres UI et le VCO DSP.
pub struct VcoParams {
    /// Accordage grossier en demi-tons (-24.0 à +24.0)
    pub coarse: AtomicF32,
    /// Accordage fin en demi-tons (-1.0 à +1.0)
    pub fine: AtomicF32,
    /// Atténuateur FM (0.0 à 1.0)
    pub fm_depth: AtomicF32,
    /// Largeur d'impulsion de base (0.05 à 0.95)
    pub pulse_width: AtomicF32,
}

impl VcoParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            coarse: AtomicF32::new(0.0),
            fine: AtomicF32::new(0.0),
            fm_depth: AtomicF32::new(0.25),
            pulse_width: AtomicF32::new(0.5),
        })
    }
}

/// Module 2 : Oscillateur Contrôlé en Tension (VCO) avec anti-aliasing PolyBLEP.
/// Entrées : 0: V/Oct, 1: FM, 2: PWM
/// Sorties : 0: Sine, 1: Triangle, 2: Saw, 3: Square (±5.0V)
pub struct VcoModule {
    params: Arc<VcoParams>,
    phase: f64,
}

impl VcoModule {
    pub fn new(params: Arc<VcoParams>) -> Self {
        Self { params, phase: 0.0 }
    }

    #[inline(always)]
    fn poly_blep(t: f64, dt: f64) -> f64 {
        if t < dt {
            let t = t / dt;
            t + t - t * t - 1.0
        } else if t > 1.0 - dt {
            let t = (t - 1.0) / dt;
            t * t + t + t + 1.0
        } else {
            0.0
        }
    }
}

impl DspModule for VcoModule {
    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        sample_rate: f32,
    ) {
        let v_oct = inputs[0];
        let fm_in = inputs[1];
        let pwm_in = inputs[2];

        let coarse_oct = self.params.coarse.load() / 12.0;
        let fine_oct = self.params.fine.load() / 12.0;
        let fm_amount = self.params.fm_depth.load();

        // C4 (0.0V) = 261.6256 Hz. La FM linéaire/exponentielle permet la modulation audio-rate.
        let total_octaves = (v_oct + coarse_oct + fine_oct + fm_in * fm_amount).clamp(-8.0, 8.0);
        let freq = (261.625565f64 * 2.0f64.powf(total_octaves as f64))
            .clamp(0.5, (sample_rate as f64) * 0.48);

        let dt = freq / (sample_rate as f64);
        let t = self.phase;

        // 1. Sinus (±5V)
        let sine = (t * TAU).sin() as f32 * 5.0;

        // 2. Triangle (±5V)
        let tri_raw = if t < 0.5 {
            4.0 * t - 1.0
        } else {
            3.0 - 4.0 * t
        };
        let triangle = tri_raw as f32 * 5.0;

        // 3. Dent de scie avec PolyBLEP (±5V)
        let mut saw_raw = 2.0 * t - 1.0;
        saw_raw -= Self::poly_blep(t, dt);
        let saw = saw_raw as f32 * 5.0;

        // 4. Carrée / PWM avec PolyBLEP (±5V)
        let pw = (self.params.pulse_width.load() + pwm_in * 0.09).clamp(0.05, 0.95) as f64;
        let mut sq_raw = if t < pw { 1.0 } else { -1.0 };
        sq_raw += Self::poly_blep(t, dt);
        sq_raw -= Self::poly_blep((t - pw).rem_euclid(1.0), dt);
        let square = sq_raw as f32 * 5.0;

        outputs[0] = sine;
        outputs[1] = triangle;
        outputs[2] = saw;
        outputs[3] = square;

        self.phase = (self.phase + dt).rem_euclid(1.0);
    }
}