use super::Translations;
use crate::ui::state::{ModuleKind, PortDirection};

pub static FR: Translations = Translations {
    settings_btn: "PARAMÈTRES",
    settings_title: "PARAMÈTRES DU RACK",
    settings_language: "LANGUE",
    settings_theme: "APPARENCE / THÈME",
    settings_tooltip_delay: "DÉLAI INFO-BULLE PORT",
    fullscreen_btn: "PLEIN ÉCRAN",
    exit_fullscreen_btn: "QUITTER",
    zoom_hint: "Ctrl+Molette pour zoomer",

    theme_current: "Studio Aluminium (Défaut)",
    theme_dark: "Mode Sombre (Carbone)",
    theme_light: "Mode Clair (Argent)",
    theme_wood: "Vintage Bois (Noyer & Laiton)",

    tab_modules: "MODULES",
    tab_presets: "PRESETS",
    drawer_modules_title: "BIBLIOTHÈQUE MODULES",
    drawer_modules_hint: "Glisser en ligne/colonne ou cliquer",
    section_dsp: "MODULES DSP DE BASE",
    section_fx: "EFFETS / RACK FX",

    drawer_presets_title: "PRESETS USINE (5)",
    drawer_presets_hint: "Cliquer pour charger un patch complet",
    modal_overwrite_title: "ÉCRASER LE PATCH ACTUEL ?",
    modal_overwrite_line1_prefix: "Charger",
    modal_overwrite_line2: "va remplacer tous les modules et câbles actifs.",
    modal_cancel: "ANNULER",
    modal_confirm: "ÉCRASER ET CHARGER",

    trash_drop_label: "DÉPOSER ICI POUR RETIRER LE MODULE",
    ctx_delete_module: "Supprimer ce module",
    port_input_badge: "ENTRÉE JACK",
    port_output_badge: "SORTIE JACK",
};

pub fn module_info(kind: ModuleKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ModuleKind::MidiToCv => ("MIDI -> CV", "INTERFACE DAW", "V/Oct, Gate, Trig, Vélocité, Bend"),
        ModuleKind::Vco => ("VCO-1", "OSCILLATEUR", "Sin, Tri, Saw, Square + FM/PWM"),
        ModuleKind::Vcf => ("VCF-24", "FILTRE LADDER", "Passe-bas/bande/haut 24dB + Drive"),
        ModuleKind::Adsr => ("ADSR", "ENVELOPPE", "Attack, Decay, Sustain, Release"),
        ModuleKind::Lfo => ("LFO", "MODULATEUR", "0.05-40 Hz + Sample & Hold"),
        ModuleKind::Vca => ("VCA", "AMPLIFICATEUR", "Contrôle de gain Linéaire / Exp"),
        ModuleKind::AudioOutput => ("AUDIO OUT", "SORTIE STÉRÉO", "Sortie stéréo + limiteur doux"),
        ModuleKind::Overdrive => ("OVERDRIVE", "SATURATION LAMPE", "Saturation chaude asymétrique + tonalité"),
        ModuleKind::Distortion => ("DISTORTION", "WAVEFOLDER FX", "Écrêtage dur, wavefold & filtre"),
        ModuleKind::Chorus => ("CHORUS", "ENSEMBLE STÉRÉO", "Double ligne à retard BBD modulée"),
        ModuleKind::Phaser => ("PHASER", "6 ÉTAGES NOTCH", "Déphaseur passe-tout + réinjection"),
    }
}

pub fn preset_info(idx: usize) -> (&'static str, &'static str, &'static str) {
    match idx {
        0 => ("1. Classic Saw Lead", "LEAD", "Dent de scie chaude, filtre 24dB & enveloppe nerveuse"),
        1 => ("2. Acid 303 Squelch", "BASSE", "Basse acide résonante avec PWM modulée par LFO"),
        2 => ("3. Cosmic FM Bells", "SYNTHÈSE FM", "Cloches cristallines FM à 2 oscillateurs"),
        3 => ("4. Hypnotic S&H Sci-Fi", "MODULATION", "Sample & Hold aléatoire sur le pitch et filtre"),
        _ => ("5. Dual-Row Stereo Pad", "NAPPE (2 RANGÉES)", "Nappe stéréo à 2 VCO désaccordés sur 2 rangées"),
    }
}

pub fn port_tooltip(kind: ModuleKind, dir: PortDirection, port_idx: usize) -> (&'static str, &'static str) {
    match (kind, dir, port_idx) {
        // MIDI -> CV
        (ModuleKind::MidiToCv, PortDirection::Output, 0) => ("Sortie V/OCT", "Tension de hauteur 1V/octave pilotée par les notes MIDI."),
        (ModuleKind::MidiToCv, PortDirection::Output, 1) => ("Sortie GATE", "Signal +5V maintenu actif tant qu'une touche est enfoncée."),
        (ModuleKind::MidiToCv, PortDirection::Output, 2) => ("Sortie TRIG", "Courte impulsion +5V de 5ms déclenchée à chaque note."),
        (ModuleKind::MidiToCv, PortDirection::Output, 3) => ("Sortie VEL", "Tension 0 à +5V proportionnelle à la vélocité MIDI."),
        (ModuleKind::MidiToCv, PortDirection::Output, 4) => ("Sortie BEND", "Signal bipolaire ±2V de la molette de pitch bend."),

        // VCO
        (ModuleKind::Vco, PortDirection::Input, 0) => ("Entrée V/OCT", "Contrôle la fréquence de l'oscillateur à 1 Volt par octave."),
        (ModuleKind::Vco, PortDirection::Input, 1) => ("Entrée FM", "Modulation de fréquence linéaire dosée par le bouton FM."),
        (ModuleKind::Vco, PortDirection::Input, 2) => ("Entrée PWM CV", "Module la largeur d'impulsion de l'onde carrée."),
        (ModuleKind::Vco, PortDirection::Output, 0) => ("Sortie Sinusoïde", "Onde sinusoïdale pure fondamentale (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 1) => ("Sortie Triangle", "Onde triangulaire douce aux harmoniques impaires (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 2) => ("Sortie Dent de scie", "Onde en dent de scie riche en harmoniques (±5V)."),
        (ModuleKind::Vco, PortDirection::Output, 3) => ("Sortie Carrée", "Onde carrée à largeur d'impulsion variable (±5V)."),

        // VCF
        (ModuleKind::Vcf, PortDirection::Input, 0) => ("Entrée Audio", "Signal audio principal envoyé dans le filtre 4 pôles."),
        (ModuleKind::Vcf, PortDirection::Input, 1) => ("Entrée Cutoff CV", "Module la fréquence de coupure (dosée par CV AMT)."),
        (ModuleKind::Vcf, PortDirection::Input, 2) => ("Entrée Résonance CV", "Module le pic de résonance et l'auto-oscillation."),
        (ModuleKind::Vcf, PortDirection::Output, 0) => ("Sortie LP 24dB", "Sortie audio filtrée passe-bas à 24dB/octave."),
        (ModuleKind::Vcf, PortDirection::Output, 1) => ("Sortie BP 12dB", "Sortie audio filtrée passe-bande à 12dB/octave."),
        (ModuleKind::Vcf, PortDirection::Output, 2) => ("Sortie HP 24dB", "Sortie audio filtrée passe-haut à 24dB/octave."),

        // ADSR
        (ModuleKind::Adsr, PortDirection::Input, 0) => ("Entrée GATE", "Lance Attack/Decay/Sustain au-dessus de 1V, Release en dessous."),
        (ModuleKind::Adsr, PortDirection::Input, 1) => ("Entrée TRIG", "Redéclenche le cycle d'attaque de l'enveloppe depuis zéro."),
        (ModuleKind::Adsr, PortDirection::Output, 0) => ("Sortie ENV", "Tension d'enveloppe unipolaire 0 à +5V."),
        (ModuleKind::Adsr, PortDirection::Output, 1) => ("Sortie INV", "Enveloppe inversée +5V à 0V pour effets de pompage."),

        // LFO
        (ModuleKind::Lfo, PortDirection::Input, 0) => ("Entrée RESET", "Remet la phase du LFO à zéro sur front montant (>1V)."),
        (ModuleKind::Lfo, PortDirection::Input, 1) => ("Entrée RATE CV", "Module la vitesse d'oscillation du LFO (1V/octave)."),
        (ModuleKind::Lfo, PortDirection::Output, 0) => ("Sortie LFO Principale", "Onde de modulation basse fréquence (±5V ou 0..+5V)."),
        (ModuleKind::Lfo, PortDirection::Output, 1) => ("Sortie S&H", "Tension aléatoire échantillonnée à chaque cycle du LFO."),
        (ModuleKind::Lfo, PortDirection::Output, 2) => ("Sortie Inversée", "Copie inversée en phase de l'onde principale du LFO."),

        // VCA
        (ModuleKind::Vca, PortDirection::Input, 0) => ("Entrée Audio / CV", "Signal dont l'amplitude est modulée par le VCA."),
        (ModuleKind::Vca, PortDirection::Input, 1) => ("Entrée Gain CV", "Tension 0..+5V ouvrant l'amplificateur (ex. depuis ADSR)."),
        (ModuleKind::Vca, PortDirection::Output, 0) => ("Sortie VCA", "Signal de sortie modulé en amplitude."),

        // Audio Out
        (ModuleKind::AudioOutput, PortDirection::Input, 0) => ("Entrée Audio Gauche", "Canal gauche principal envoyé vers votre piste DAW."),
        (ModuleKind::AudioOutput, PortDirection::Input, 1) => ("Entrée Audio Droite", "Canal droit (copie le canal gauche si non branché)."),

        // Overdrive
        (ModuleKind::Overdrive, PortDirection::Input, 0) => ("Entrée Audio", "Signal envoyé dans l'étage de saturation à lampes."),
        (ModuleKind::Overdrive, PortDirection::Input, 1) => ("Entrée Drive CV", "Module l'intensité de la saturation en temps réel."),
        (ModuleKind::Overdrive, PortDirection::Output, 0) => ("Sortie Overdrive", "Signal saturé chaud riche en harmoniques paires."),

        // Distortion
        (ModuleKind::Distortion, PortDirection::Input, 0) => ("Entrée Audio", "Signal envoyé dans le wavefolder et l'écrêteur dur."),
        (ModuleKind::Distortion, PortDirection::Input, 1) => ("Entrée Fold / Gain CV", "Module le gain et le repliement d'onde (wavefolding)."),
        (ModuleKind::Distortion, PortDirection::Output, 0) => ("Sortie Distortion", "Signal distordu, replié et filtré."),

        // Chorus
        (ModuleKind::Chorus, PortDirection::Input, 0) => ("Entrée Audio Mono", "Signal divisé vers deux lignes à retard modulées."),
        (ModuleKind::Chorus, PortDirection::Input, 1) => ("Entrée Rate CV", "Module la vitesse de balayage du LFO du chorus."),
        (ModuleKind::Chorus, PortDirection::Output, 0) => ("Sortie Stéréo Gauche", "Canal gauche du chorus analogique BBD."),
        (ModuleKind::Chorus, PortDirection::Output, 1) => ("Sortie Stéréo Droite", "Canal droit déphasé pour une image stéréo large."),

        // Phaser
        (ModuleKind::Phaser, PortDirection::Input, 0) => ("Entrée Audio", "Signal traité par 6 filtres passe-tout en cascade."),
        (ModuleKind::Phaser, PortDirection::Input, 1) => ("Entrée Sweep CV", "Modulation externe des fréquences de creux du phaser."),
        (ModuleKind::Phaser, PortDirection::Output, 0) => ("Sortie Phaser", "Signal filtré avec balayage de phase."),

        _ => ("Prise Jack", "Connexion audio / CV Eurorack 3.5mm."),
    }
}