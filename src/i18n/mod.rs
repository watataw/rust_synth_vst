pub mod en;
pub mod fr;

use crate::ui::state::{ModuleKind, PortDirection, RackTheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    Fr,
}

impl Default for Language {
    fn default() -> Self {
        Self::En
    }
}

impl Language {
    pub fn translations(self) -> &'static Translations {
        match self {
            Language::En => &en::EN,
            Language::Fr => &fr::FR,
        }
    }
}

pub struct Translations {
    pub settings_btn: &'static str,
    pub settings_title: &'static str,
    pub settings_language: &'static str,
    pub settings_theme: &'static str,
    pub settings_tooltip_delay: &'static str,
    pub fullscreen_btn: &'static str,
    pub exit_fullscreen_btn: &'static str,
    pub zoom_hint: &'static str,

    pub theme_current: &'static str,
    pub theme_dark: &'static str,
    pub theme_light: &'static str,
    pub theme_wood: &'static str,

    pub tab_modules: &'static str,
    pub tab_presets: &'static str,
    pub drawer_modules_title: &'static str,
    pub drawer_modules_hint: &'static str,
    pub section_dsp: &'static str,
    pub section_fx: &'static str,

    pub drawer_presets_title: &'static str,
    pub drawer_presets_hint: &'static str,
    pub modal_overwrite_title: &'static str,
    pub modal_overwrite_line1_prefix: &'static str,
    pub modal_overwrite_line2: &'static str,
    pub modal_cancel: &'static str,
    pub modal_confirm: &'static str,

    pub trash_drop_label: &'static str,
    pub ctx_delete_module: &'static str,
    pub port_input_badge: &'static str,
    pub port_output_badge: &'static str,
}

impl Translations {
    pub fn theme_label(&self, theme: RackTheme) -> &'static str {
        match theme {
            RackTheme::Current => self.theme_current,
            RackTheme::Dark => self.theme_dark,
            RackTheme::Light => self.theme_light,
            RackTheme::VintageWood => self.theme_wood,
        }
    }

    pub fn module_info(&self, lang: Language, kind: ModuleKind) -> (&'static str, &'static str, &'static str) {
        match lang {
            Language::En => en::module_info(kind),
            Language::Fr => fr::module_info(kind),
        }
    }

    pub fn preset_info(&self, lang: Language, idx: usize) -> (&'static str, &'static str, &'static str) {
        match lang {
            Language::En => en::preset_info(idx),
            Language::Fr => fr::preset_info(idx),
        }
    }

    pub fn port_tooltip(
        &self,
        lang: Language,
        kind: ModuleKind,
        dir: PortDirection,
        port_idx: usize,
    ) -> (&'static str, &'static str) {
        match lang {
            Language::En => en::port_tooltip(kind, dir, port_idx),
            Language::Fr => fr::port_tooltip(kind, dir, port_idx),
        }
    }
}