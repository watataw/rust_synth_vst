use super::Translations;
use crate::ui::state::{ModuleKind, PortDirection};

pub static EN: Translations = Translations {
    settings_btn: "SETTINGS",
    settings_title: "RACK SETTINGS",
    settings_language: "LANGUAGE",
    settings_theme: "APPEARANCE / THEME",
    settings_tooltip_delay: "PORT TOOLTIP DELAY",
    fullscreen_btn: "FULLSCREEN",
    exit_fullscreen_btn: "EXIT FULL",
    zoom_hint: "Ctrl+Scroll to Zoom",

    theme_current: "Studio Aluminum (Default)",
    theme_dark: "Dark Mode (Carbon)",
    theme_light: "Light Mode (Silver)",
    theme_wood: "Vintage Wood (Walnut & Brass)",

    tab_modules: "MODULES",
    tab_presets: "PRESETS",
    drawer_modules_title: "MODULE LIBRARY",
    drawer_modules_hint: "Drag to row/column or click to add",
    section_dsp: "DSP CORE MODULES",
    section_fx: "EFFECTS / FX RACK",

    drawer_presets_title: "FACTORY PRESETS (5)",
    drawer_presets_hint: "Click a preset to load full patch",
    modal_overwrite_title: "OVERWRITE CURRENT PATCH?",
    modal_overwrite_line1_prefix: "Loading",
    modal_overwrite_line2: "will replace all active modules and cables.",
    modal_cancel: "CANCEL",
    modal_confirm: "OVERWRITE & LOAD",

    trash_drop_label: "DROP HERE TO REMOVE MODULE",
    ctx_delete_module: "Remove this module",
    port_input_badge: "INPUT JACK",
    port_output_badge: "OUTPUT JACK",
};

pub fn module_info(kind: ModuleKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ModuleKind::MidiToCv => ("MIDI -> CV", "DAW INTERFACE", "V/Oct, Gate, Trig, Velocity, Bend"),
        ModuleKind::Vco => ("VCO-1", "OSCILLATOR", "Sin, Tri, Saw, Square + FM/PWM"),
        ModuleKind::Vcf => ("VCF-24", "LADDER FILTER", "24dB Low/Band/High-pass + Drive"),
        ModuleKind::Adsr => ("ADSR", "ENVELOPE", "Attack, Decay, Sustain, Release"),
        ModuleKind::Lfo => ("LFO", "MODULATOR", "0.05-40 Hz + Sample & Hold"),
        ModuleKind::Vca => ("VCA", "AMPLIFIER", "Linear / Exponential gain control"),
        ModuleKind::AudioOutput => ("AUDIO OUT", "STEREO MASTER", "Stereo output + soft limiter"),
        ModuleKind::Overdrive => ("OVERDRIVE", "TUBE SATURATION", "Warm asymmetric clip + tone"),
        ModuleKind::Distortion => ("DISTORTION", "WAVEFOLDER FX", "Hard clip, wavefold & post-filter"),
        ModuleKind::Chorus => ("CHORUS", "STEREO ENSEMBLE", "Dual BBD modulated delay voices"),
        ModuleKind::Phaser => ("PHASER", "6-STAGE NOTCH", "All-pass phase shifter + feedback"),
    }
}

pub fn preset_info(idx: usize) -> (&'static str, &'static str, &'static str) {
    match idx {
        0 => ("1. Classic Saw Lead", "LEAD", "Warm sawtooth, 24dB filter & snappy envelope"),
        1 => ("2. Acid 303 Squelch", "BASS", "Resonant acid bass with LFO pulse modulation"),
        2 => ("3. Cosmic FM Bells", "FM SYNTH", "2-operator audio-rate FM crystalline bells"),
        3 => ("4. Hypnotic S&H Sci-Fi", "MODULATION", "Random Sample & Hold pitch & bandpass sweep"),
        _ => ("5. Dual-Row Stereo Pad", "PAD (2 ROWS)", "Detuned dual-VCO stereo pad across 2 rows"),
    }
}

pub fn port_tooltip(kind: ModuleKind, dir: PortDirection, port_idx: usize) -> (&'static str, &'static str) {
    match (kind, dir, port_idx) {
        // MIDI -> CV
        (ModuleKind::MidiToCv, PortDirection::Output, 0) => ("V/OCT Output", "1V/octave pitch control voltage from incoming MIDI notes."),
        (ModuleKind::MidiToCv, PortDirection::Output, 1) => ("GATE Output", "+5V gate signal held high while a MIDI key is pressed."),
        (ModuleKind::MidiToCv, PortDirection::Output, 2) => ("TRIG Output", "Short 5ms +5V trigger pulse fired on every new note."),
        (ModuleKind::MidiToCv, PortDirection::Output, 3) => ("VEL Output", "0 to +5V voltage proportional to MIDI note-on velocity."),
        (ModuleKind::MidiToCv, PortDirection::Output, 4) => ("BEND Output", "Bipolar ±2V pitch wheel modulation signal."),

        // VCO
        (ModuleKind::Vco, PortDirection::Input, 0) => ("V/OCT Pitch CV", "Controls oscillator frequency at 1 Volt per octave."),
        (ModuleKind::Vco, PortDirection::Input, 1) => ("FM Input", "Linear frequency modulation input scaled by the FM knob."),
        (ModuleKind::Vco, PortDirection::Input, 2) => ("PWM CV Input", "Modulates the pulse width of the Square wave output."),
        (ModuleKind::Vco, PortDirection::Output, 0) => ("Sine Output", "Pure fundamental sine wave (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 1) => ("Triangle Output", "Soft odd-harmonic triangle wave (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 2) => ("Sawtooth Output", "Bright harmonic-rich saw wave (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 3) => ("Square Output", "Hollow square/pulse wave with variable width (±5V)."),

        // VCF
        (ModuleKind::Vcf, PortDirection::Input, 0) => ("Audio Input", "Main audio signal routed into the 4-pole ladder filter."),
        (ModuleKind::Vcf, PortDirection::Input, 1) => ("Cutoff CV", "Modulates filter cutoff frequency (scaled by CV AMT)."),
        (ModuleKind::Vcf, PortDirection::Input, 2) => ("Resonance CV", "Modulates filter resonance peak and self-oscillation."),
        (ModuleKind::Vcf, PortDirection::Output, 0) => ("LP 24dB Output", "24dB/octave low-pass filtered audio output."),
        (ModuleKind::Vcf, PortDirection::Output, 1) => ("BP 12dB Output", "12dB/octave band-pass filtered audio output."),
        (ModuleKind::Vcf, PortDirection::Output, 2) => ("HP 24dB Output", "24dB/octave high-pass filtered audio output."),

        // ADSR
        (ModuleKind::Adsr, PortDirection::Input, 0) => ("GATE Input", "Starts Attack/Decay/Sustain when high (>1V), Release when low."),
        (ModuleKind::Adsr, PortDirection::Input, 1) => ("TRIG Input", "Retriggers the envelope attack cycle from zero."),
        (ModuleKind::Adsr, PortDirection::Output, 0) => ("ENV Output", "0 to +5V unipolar envelope control voltage."),
        (ModuleKind::Adsr, PortDirection::Output, 1) => ("INV Output", "Inverted +5V to 0V envelope signal for ducking/filter sweeps."),

        // LFO
        (ModuleKind::Lfo, PortDirection::Input, 0) => ("RESET Input", "Resets LFO phase to 0 on rising edge (>1V)."),
        (ModuleKind::Lfo, PortDirection::Input, 1) => ("RATE CV Input", "Modulates LFO oscillation speed (1V/octave)."),
        (ModuleKind::Lfo, PortDirection::Output, 0) => ("LFO Main Out", "Primary low-frequency modulation wave (±5V or 0..+5V)."),
        (ModuleKind::Lfo, PortDirection::Output, 1) => ("S&H Output", "Stepped random voltage sampled once per LFO cycle."),
        (ModuleKind::Lfo, PortDirection::Output, 2) => ("Inverted Out", "Phase-inverted copy of the main LFO waveform."),

        // VCA
        (ModuleKind::Vca, PortDirection::Input, 0) => ("Audio / CV In", "Signal whose amplitude is controlled by the VCA."),
        (ModuleKind::Vca, PortDirection::Input, 1) => ("Gain CV In", "0..+5V control voltage opening the amplifier (e.g. from ADSR)."),
        (ModuleKind::Vca, PortDirection::Output, 0) => ("VCA Output", "Amplitude-modulated output signal."),

        // Audio Out
        (ModuleKind::AudioOutput, PortDirection::Input, 0) => ("Left Audio In", "Main left channel sent to your DAW track."),
        (ModuleKind::AudioOutput, PortDirection::Input, 1) => ("Right Audio In", "Main right channel (normalized to Left if unpatched)."),

        // Overdrive
        (ModuleKind::Overdrive, PortDirection::Input, 0) => ("Audio Input", "Clean signal fed into the warm tube overdrive stage."),
        (ModuleKind::Overdrive, PortDirection::Input, 1) => ("Drive CV", "Modulates saturation drive intensity in real time."),
        (ModuleKind::Overdrive, PortDirection::Output, 0) => ("Overdrive Out", "Warm harmonically saturated output signal."),

        // Distortion
        (ModuleKind::Distortion, PortDirection::Input, 0) => ("Audio Input", "Signal routed into the wavefolder and hard clipper."),
        (ModuleKind::Distortion, PortDirection::Input, 1) => ("Fold / Gain CV", "Modulates distortion gain and wavefolding symmetry."),
        (ModuleKind::Distortion, PortDirection::Output, 0) => ("Distortion Out", "Aggressive wavefolded & filtered output signal."),

        // Chorus
        (ModuleKind::Chorus, PortDirection::Input, 0) => ("Mono Audio In", "Input signal split into dual modulated delay lines."),
        (ModuleKind::Chorus, PortDirection::Input, 1) => ("Rate CV", "Modulates internal chorus LFO sweep speed."),
        (ModuleKind::Chorus, PortDirection::Output, 0) => ("Stereo Left Out", "Left channel of the wide stereo BBD chorus."),
        (ModuleKind::Chorus, PortDirection::Output, 1) => ("Stereo Right Out", "Phase-offset right channel for wide stereo imaging."),

        // Phaser
        (ModuleKind::Phaser, PortDirection::Input, 0) => ("Audio Input", "Signal processed through 6 cascaded all-pass stages."),
        (ModuleKind::Phaser, PortDirection::Input, 1) => ("Sweep CV", "External CV modulation for the all-pass notch frequencies."),
        (ModuleKind::Phaser, PortDirection::Output, 0) => ("Phaser Out", "Sweeping comb-filtered phaser output signal."),

        _ => ("Patch Jack", "3.5mm Eurorack audio / CV patch connection."),
    }
}