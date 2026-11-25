use crate::ui::win32_window;
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use std::any::Any;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

pub struct ResizableViziaEditor {
    inner: Box<dyn Editor>,
    parent_hwnd: Arc<AtomicIsize>,
}

impl ResizableViziaEditor {
    pub fn new(inner: Box<dyn Editor>, parent_hwnd: Arc<AtomicIsize>) -> Box<dyn Editor> {
        Box::new(Self { inner, parent_hwnd })
    }
}

impl Editor for ResizableViziaEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        let handle = self.inner.spawn(parent, context);

        #[cfg(target_os = "windows")]
        if let ParentWindowHandle::Win32Hwnd(hwnd) = parent {
            let hwnd_val = hwnd as isize;
            self.parent_hwnd.store(hwnd_val, Ordering::Release);
            win32_window::install_parent_resize_hook(hwnd_val);
        }

        handle
    }

    fn size(&self) -> (u32, u32) {
        self.inner.size()
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

impl Drop for ResizableViziaEditor {
    fn drop(&mut self) {
        let hwnd = self.parent_hwnd.swap(0, Ordering::AcqRel);
        if hwnd != 0 {
            win32_window::remove_parent_resize_hook(hwnd);
        }
    }
}