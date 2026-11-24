use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use std::any::Any;
use std::sync::{Arc, Mutex};

pub struct ResizableViziaEditor {
    inner: Box<dyn Editor>,
    window_size: Arc<Mutex<(u32, u32)>>,
}

impl ResizableViziaEditor {
    pub fn new(inner: Box<dyn Editor>, window_size: Arc<Mutex<(u32, u32)>>) -> Box<dyn Editor> {
        Box::new(Self { inner, window_size })
    }
}

impl Editor for ResizableViziaEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        self.inner.spawn(parent, context)
    }

    fn size(&self) -> (u32, u32) {
        *self.window_size.lock().unwrap()
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        self.inner.set_scale_factor(factor)
    }

    fn param_value_changed(&self, id: &str, normalized_value: f32) {
        self.inner.param_value_changed(id, normalized_value);
    }

    fn param_modulation_changed(&self, id: &str, modulation_offset: f32) {
        self.inner.param_modulation_changed(id, modulation_offset);
    }

    fn param_values_changed(&self) {
        self.inner.param_values_changed();
    }
}