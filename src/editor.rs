use glib::GString;
use gtk::prelude::*;
use sourceview::prelude::*;
use sourceview::{Buffer, LanguageManager, StyleSchemeManager, View};

pub trait Editor {
    fn set_language(&self, language: &str) -> Option<()>;

    fn set_theme(&self, theme: &str) -> Option<()>;

    fn buffer(&self) -> Option<Buffer>;

    fn text(&self) -> Option<GString>;

    fn set_text(&self, text: &str) -> Option<()>;
}

impl Editor for View {
    fn set_language(&self, language: &str) -> Option<()> {
        let buffer = self.buffer()?;
        let lang = LanguageManager::get_default()?.get_language(language);
        buffer.set_language(lang.as_ref());
        Some(())
    }

    fn set_theme(&self, theme: &str) -> Option<()> {
        let buffer = self.buffer()?;
        let s = StyleSchemeManager::get_default()?.get_scheme(theme);
        buffer.set_style_scheme(s.as_ref());
        Some(())
    }

    fn buffer(&self) -> Option<Buffer> {
        let buffer = self.get_buffer()?;
        buffer.downcast::<Buffer>().ok()
    }

    fn text(&self) -> Option<GString> {
        let buffer = self.get_buffer()?;
        buffer.get_text(&buffer.get_start_iter(), &buffer.get_end_iter(), true)
    }

    fn set_text(&self, text: &str) -> Option<()> {
        let buffer = self.get_buffer()?;
        buffer.set_text(text);
        Some(())
    }
}
