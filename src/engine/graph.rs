use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

pub const MAX_MODULES: usize = 64;
pub const MAX_PORTS_PER_MODULE: usize = 8;
pub const MAX_STACKED_CABLES: usize = 4;
pub const TOTAL_BUS_SLOTS: usize = MAX_MODULES * MAX_PORTS_PER_MODULE;

#[derive(Debug)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(val: f32) -> Self {
        Self(AtomicU32::new(val.to_bits()))
    }

    #[inline(always)]
    pub fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    #[inline(always)]
    pub fn store(&self, val: f32) {
        self.0.store(val.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MidiMessage {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    PitchBend { value: f32 },
}

pub trait DspModule: Send {
    fn set_sample_rate(&mut self, _sample_rate: f32) {}
    fn handle_midi(&mut self, _msg: MidiMessage) {}

    fn process_sample(
        &mut self,
        inputs: &[f32; MAX_PORTS_PER_MODULE],
        outputs: &mut [f32; MAX_PORTS_PER_MODULE],
        connected_inputs: u8,
        sample_rate: f32,
    );

    fn take_audio_out(&mut self) -> Option<(f32, f32)> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InputRouting {
    pub sources: [u16; MAX_STACKED_CABLES],
    pub count: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExecutionStep {
    pub slot_id: usize,
    pub inputs: [InputRouting; MAX_PORTS_PER_MODULE],
    pub connected_mask: u8,
}

pub struct ExecutionPlan {
    pub steps: Box<[ExecutionStep]>,
}

impl ExecutionPlan {
    pub fn empty() -> Self {
        Self {
            steps: Box::new([]),
        }
    }
}

pub enum AudioCommand {
    InsertModule {
        slot_id: usize,
        module: Box<dyn DspModule>,
    },
    RemoveModule {
        slot_id: usize,
    },
    UpdatePlan(ExecutionPlan),
}

pub enum AudioGarbage {
    DroppedModule(Box<dyn DspModule>),
    DroppedPlan(ExecutionPlan),
}

#[derive(Clone, Copy)]
pub struct TelemetrySnapshot {
    /// Niveau crête lissé (0.0 .. 5.0V) pour les LEDs
    pub port_levels: [f32; TOTAL_BUS_SLOTS],
    /// Tension instantanée signée (-5.0 .. +5.0V) pour les écrans et oscilloscopes
    pub port_values: [f32; TOTAL_BUS_SLOTS],
}

impl Default for TelemetrySnapshot {
    fn default() -> Self {
        Self {
            port_levels: [0.0; TOTAL_BUS_SLOTS],
            port_values: [0.0; TOTAL_BUS_SLOTS],
        }
    }
}

pub struct AudioEngine {
    modules: Vec<Option<Box<dyn DspModule>>>,
    plan: ExecutionPlan,
    signal_bus: [f32; TOTAL_BUS_SLOTS],
    cmd_rx: rtrb::Consumer<AudioCommand>,
    garbage_tx: rtrb::Producer<AudioGarbage>,
    telemetry_tx: triple_buffer::Input<TelemetrySnapshot>,
    telemetry_cache: TelemetrySnapshot,
    sample_rate: f32,
}

impl AudioEngine {
    pub fn new(
        cmd_rx: rtrb::Consumer<AudioCommand>,
        garbage_tx: rtrb::Producer<AudioGarbage>,
        telemetry_tx: triple_buffer::Input<TelemetrySnapshot>,
    ) -> Self {
        let mut modules = Vec::with_capacity(MAX_MODULES);
        for _ in 0..MAX_MODULES {
            modules.push(None);
        }

        Self {
            modules,
            plan: ExecutionPlan::empty(),
            signal_bus: [0.0; TOTAL_BUS_SLOTS],
            cmd_rx,
            garbage_tx,
            telemetry_tx,
            telemetry_cache: TelemetrySnapshot::default(),
            sample_rate: 44100.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for m in self.modules.iter_mut().flatten() {
            m.set_sample_rate(sample_rate);
        }
    }

    pub fn poll_commands(&mut self) {
        while let Ok(cmd) = self.cmd_rx.pop() {
            match cmd {
                AudioCommand::InsertModule { slot_id, mut module } => {
                    if slot_id < MAX_MODULES {
                        module.set_sample_rate(self.sample_rate);
                        if let Some(old) = self.modules[slot_id].replace(module) {
                            let _ = self.garbage_tx.push(AudioGarbage::DroppedModule(old));
                        }
                    }
                }
                AudioCommand::RemoveModule { slot_id } => {
                    if slot_id < MAX_MODULES {
                        let base = slot_id * MAX_PORTS_PER_MODULE;
                        self.signal_bus[base..base + MAX_PORTS_PER_MODULE].fill(0.0);
                        self.telemetry_cache.port_levels[base..base + MAX_PORTS_PER_MODULE].fill(0.0);
                        self.telemetry_cache.port_values[base..base + MAX_PORTS_PER_MODULE].fill(0.0);

                        if let Some(old) = self.modules[slot_id].take() {
                            let _ = self.garbage_tx.push(AudioGarbage::DroppedModule(old));
                        }
                    }
                }
                AudioCommand::UpdatePlan(new_plan) => {
                    let old_plan = std::mem::replace(&mut self.plan, new_plan);
                    let _ = self.garbage_tx.push(AudioGarbage::DroppedPlan(old_plan));
                }
            }
        }
    }

    pub fn push_midi(&mut self, msg: MidiMessage) {
        for m in self.modules.iter_mut().flatten() {
            m.handle_midi(msg);
        }
    }

    #[inline]
    pub fn process_sample(&mut self) -> (f32, f32) {
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        for step in self.plan.steps.iter() {
            let slot = step.slot_id;
            if let Some(module) = self.modules[slot].as_mut() {
                let mut inputs = [0.0f32; MAX_PORTS_PER_MODULE];
                for (port_idx, routing) in step.inputs.iter().enumerate() {
                    let mut sum = 0.0f32;
                    for i in 0..(routing.count as usize) {
                        let bus_idx = routing.sources[i] as usize;
                        sum += self.signal_bus[bus_idx];
                    }
                    inputs[port_idx] = sum;
                }

                let mut outputs = [0.0f32; MAX_PORTS_PER_MODULE];
                module.process_sample(
                    &inputs,
                    &mut outputs,
                    step.connected_mask,
                    self.sample_rate,
                );

                let base = slot * MAX_PORTS_PER_MODULE;
                self.signal_bus[base..base + MAX_PORTS_PER_MODULE].copy_from_slice(&outputs);

                for p in 0..MAX_PORTS_PER_MODULE {
                    let val = outputs[p];
                    let abs_v = val.abs();
                    let prev = self.telemetry_cache.port_levels[base + p];
                    self.telemetry_cache.port_levels[base + p] = abs_v.max(prev * 0.9992);
                    self.telemetry_cache.port_values[base + p] = val;
                }

                if let Some((l, r)) = module.take_audio_out() {
                    out_l += l;
                    out_r += r;
                }
            }
        }

        (out_l * 0.2, out_r * 0.2)
    }

    pub fn publish_telemetry(&mut self) {
        *self.telemetry_tx.input_buffer() = self.telemetry_cache;
        self.telemetry_tx.publish();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalCable {
    pub src_slot: usize,
    pub src_port: usize,
    pub dst_slot: usize,
    pub dst_port: usize,
}

pub fn compile_execution_plan(active_slots: &[usize], cables: &[LogicalCable]) -> ExecutionPlan {
    if active_slots.is_empty() {
        return ExecutionPlan::empty();
    }

    let mut in_degree = [0usize; MAX_MODULES];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); MAX_MODULES];

    for cable in cables {
        if cable.src_slot != cable.dst_slot
            && active_slots.contains(&cable.src_slot)
            && active_slots.contains(&cable.dst_slot)
        {
            if !adj[cable.src_slot].contains(&cable.dst_slot) {
                adj[cable.src_slot].push(cable.dst_slot);
                in_degree[cable.dst_slot] += 1;
            }
        }
    }

    let mut scheduled = [false; MAX_MODULES];
    let mut ordered_slots = Vec::with_capacity(active_slots.len());
    let mut queue = VecDeque::new();

    for &slot in active_slots {
        if in_degree[slot] == 0 {
            queue.push_back(slot);
        }
    }

    while ordered_slots.len() < active_slots.len() {
        if let Some(u) = queue.pop_front() {
            if scheduled[u] {
                continue;
            }
            scheduled[u] = true;
            ordered_slots.push(u);

            for &v in &adj[u] {
                if !scheduled[v] && in_degree[v] > 0 {
                    in_degree[v] -= 1;
                    if in_degree[v] == 0 {
                        queue.push_back(v);
                    }
                }
            }
        } else if let Some(&fallback) = active_slots.iter().find(|&&s| !scheduled[s]) {
            in_degree[fallback] = 0;
            queue.push_back(fallback);
        } else {
            break;
        }
    }

    let mut steps = Vec::with_capacity(ordered_slots.len());
    for slot_id in ordered_slots {
        let mut inputs = [InputRouting::default(); MAX_PORTS_PER_MODULE];
        let mut connected_mask = 0u8;

        for cable in cables {
            if cable.dst_slot == slot_id && cable.dst_port < MAX_PORTS_PER_MODULE {
                let routing = &mut inputs[cable.dst_port];
                if (routing.count as usize) < MAX_STACKED_CABLES {
                    let src_bus_idx = (cable.src_slot * MAX_PORTS_PER_MODULE + cable.src_port) as u16;
                    routing.sources[routing.count as usize] = src_bus_idx;
                    routing.count += 1;
                    connected_mask |= 1 << cable.dst_port;
                }
            }
        }

        steps.push(ExecutionStep {
            slot_id,
            inputs,
            connected_mask,
        });
    }

    ExecutionPlan {
        steps: steps.into_boxed_slice(),
    }
}