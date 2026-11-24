pub mod dsp;
mod editor;
pub mod engine;
pub mod i18n;
pub mod physics;
pub mod ui;

use editor::ResizableViziaEditor;
use engine::{AudioCommand, AudioEngine, AudioGarbage, MidiMessage, TelemetrySnapshot};
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::{Arc, Mutex};
use triple_buffer::TripleBuffer;
use ui::{RackCanvas, RackUiState};

struct UiBridgeEndpoints {
    cmd_tx: Option<rtrb::Producer<AudioCommand>>,
    garbage_rx: Option<rtrb::Consumer<AudioGarbage>>,
    telemetry_rx: Option<triple_buffer::Output<TelemetrySnapshot>>,
}

#[derive(Params)]
pub struct ModularParams {
    #[persist = "editor-state"]
    editor_state: Arc<ViziaState>,

    #[id = "cable_alpha"]
    pub cable_alpha: FloatParam,
}

impl Default for ModularParams {
    fn default() -> Self {
        Self {
            editor_state: ViziaState::new(|| (1360, 620)),
            cable_alpha: FloatParam::new(
                "Transparence Câbles",
                0.85,
                FloatRange::Linear { min: 0.25, max: 1.0 },
            )
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
        }
    }
}

pub struct RustModularSynth {
    params: Arc<ModularParams>,
    audio_engine: AudioEngine,
    ui_endpoints: Arc<Mutex<UiBridgeEndpoints>>,
    ui_state_holder: Arc<Mutex<Option<Arc<Mutex<RackUiState>>>>>,
}

impl Default for RustModularSynth {
    fn default() -> Self {
        let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(256);
        let (garbage_tx, garbage_rx) = rtrb::RingBuffer::new(256);
        let (telemetry_tx, telemetry_rx) = TripleBuffer::new(&TelemetrySnapshot::default()).split();

        Self {
            params: Arc::new(ModularParams::default()),
            audio_engine: AudioEngine::new(cmd_rx, garbage_tx, telemetry_tx),
            ui_endpoints: Arc::new(Mutex::new(UiBridgeEndpoints {
                cmd_tx: Some(cmd_tx),
                garbage_rx: Some(garbage_rx),
                telemetry_rx: Some(telemetry_rx),
            })),
            ui_state_holder: Arc::new(Mutex::new(None)),
        }
    }
}

impl Plugin for RustModularSynth {
    const NAME: &'static str = "Rust Modular Rack";
    const VENDOR: &'static str = "Modular DSP Studio";
    const URL: &'static str = "https://example.com";
    const EMAIL: &'static str = "contact@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.audio_engine.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let endpoints = self.ui_endpoints.clone();
        let state_holder = self.ui_state_holder.clone();
        let initial_size = self.params.editor_state.inner_logical_size();
        let window_size = Arc::new(Mutex::new(initial_size));
        let window_size_for_view = window_size.clone();

        let editor = create_vizia_editor(
            self.params.editor_state.clone(),
            ViziaTheming::Custom,
            move |cx, gui_context| {
                let ui_state = {
                    let mut holder = state_holder.lock().unwrap();
                    if let Some(existing) = holder.as_ref() {
                        existing.clone()
                    } else {
                        let mut ep = endpoints.lock().unwrap();
                        let created = Arc::new(Mutex::new(RackUiState::new(
                            ep.cmd_tx.take().expect("cmd_tx déjà consommé"),
                            ep.garbage_rx.take().expect("garbage_rx déjà consommé"),
                            ep.telemetry_rx.take().expect("telemetry_rx déjà consommé"),
                        )));
                        *holder = Some(created.clone());
                        created
                    }
                };

                RackCanvas::new(
                    cx,
                    ui_state,
                    params.clone(),
                    gui_context.clone(),
                    window_size_for_view.clone(),
                )
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
            },
        )?;

        Some(ResizableViziaEditor::new(editor, window_size))
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.audio_engine.poll_commands();

        let num_samples = buffer.samples();
        let output = buffer.as_slice();
        if output.len() < 2 {
            return ProcessStatus::Normal;
        }

        let (left_slice, right_slice) = output.split_at_mut(1);
        let left_ch = &mut left_slice[0];
        let right_ch = &mut right_slice[0];

        let mut next_event = context.next_event();

        for sample_idx in 0..num_samples {
            while let Some(event) = next_event {
                if event.timing() > sample_idx as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        self.audio_engine.push_midi(MidiMessage::NoteOn {
                            note,
                            velocity: (velocity * 127.0).round() as u8,
                        });
                    }
                    NoteEvent::NoteOff { note, .. } => {
                        self.audio_engine.push_midi(MidiMessage::NoteOff { note });
                    }
                    NoteEvent::MidiPitchBend { value, .. } => {
                        let bipolar = (value - 0.5) * 2.0;
                        self.audio_engine.push_midi(MidiMessage::PitchBend { value: bipolar });
                    }
                    _ => {}
                }

                next_event = context.next_event();
            }

            let (l, r) = self.audio_engine.process_sample();
            left_ch[sample_idx] = l;
            right_ch[sample_idx] = r;
        }

        self.audio_engine.publish_telemetry();

        ProcessStatus::Normal
    }
}

impl ClapPlugin for RustModularSynth {
    const CLAP_ID: &'static str = "com.modulardsp.rust-modular-rack";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Synthétiseur modulaire Eurorack en Rust");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for RustModularSynth {
    const VST3_CLASS_ID: [u8; 16] = *b"RustModularRack1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(RustModularSynth);
nih_export_vst3!(RustModularSynth);