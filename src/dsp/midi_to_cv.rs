use crate::engine::{DspModule, MidiMessage, MAX_PORTS_PER_MODULE};

const NOTE_STACK_CAPACITY: usize = 16;

/// Module 1 : Interface MIDI-to-CV (Priorité à la dernière note / Last Note Priority).
/// Ports de sortie :
/// 0: V/Oct (1V/Oct, C4 = 0.0V)
/// 1: Gate (+5.0V tant qu'une note est maintenue)
/// 2: Trigger (Impulsion +5.0V de 1 ms à chaque NoteOn)
/// 3: Velocity (0.0V à +5.0V)
/// 4: Pitch Bend (-5.0V à +5.0V)
pub struct MidiToCvModule {
    note_stack: [u8; NOTE_STACK_CAPACITY],
    stack_len: usize,
    current_note: u8,
    velocity_cv: f32,
    pitch_bend_cv: f32,
    trig_samples_remaining: usize,
    sample_rate: f32,
}

impl MidiToCvModule {
    pub fn new() -> Self {
        Self {
            note_stack: [60; NOTE_STACK_CAPACITY],
            stack_len: 0,
            current_note: 60,
            velocity_cv: 0.0,
            pitch_bend_cv: 0.0,
            trig_samples_remaining: 0,
            sample_rate: 44100.0,
        }
    }
}

impl DspModule for MidiToCvModule {
    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn handle_midi(&mut self, msg: MidiMessage) {
        match msg {
            MidiMessage::NoteOn { note, velocity } => {
                // Retire la note si elle était déjà dans la pile pour la remonter au sommet
                if let Some(pos) = self.note_stack[..self.stack_len].iter().position(|&n| n == note) {
                    for i in pos..self.stack_len - 1 {
                        self.note_stack[i] = self.note_stack[i + 1];
                    }
                    self.stack_len -= 1;
                }

                if self.stack_len < NOTE_STACK_CAPACITY {
                    self.note_stack[self.stack_len] = note;
                    self.stack_len += 1;
                } else {
                    self.note_stack.rotate_left(1);
                    self.note_stack[NOTE_STACK_CAPACITY - 1] = note;
                }

                self.current_note = note;
                self.velocity_cv = (velocity as f32 / 127.0) * 5.0;
                // Impulsion trigger de 1 ms
                self.trig_samples_remaining = (0.001 * self.sample_rate).max(1.0) as usize;
            }
            MidiMessage::NoteOff { note } => {
                if let Some(pos) = self.note_stack[..self.stack_len].iter().position(|&n| n == note) {
                    for i in pos..self.stack_len - 1 {
                        self.note_stack[i] = self.note_stack[i + 1];
                    }
                    self.stack_len -= 1;
                }
                if self.stack_len > 0 {
                    self.current_note = self.note_stack[self.stack_len - 1];
                }
            }
            MidiMessage::PitchBend { value } => {
                self.pitch_bend_cv = value.clamp(-1.0, 1.0) * 5.0;
            }
        }
    }

    fn process_sample(
        &mut self,
        _inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        _connected_inputs: u8,
        _sample_rate: f32,
    ) {
        let v_oct = (self.current_note as f32 - 60.0) / 12.0;
        let gate = if self.stack_len > 0 { 5.0 } else { 0.0 };
        let trigger = if self.trig_samples_remaining > 0 {
            self.trig_samples_remaining -= 1;
            5.0
        } else {
            0.0
        };

        outputs[0] = v_oct;
        outputs[1] = gate;
        outputs[2] = trigger;
        outputs[3] = self.velocity_cv;
        outputs[4] = self.pitch_bend_cv;
    }
}