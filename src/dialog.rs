use crate::Editor;
use glade_macro::ui;
use gtk::{prelude::*, Button, InfoBar, Label, ResponseType, Revealer};
use sourceview::View;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

#[ui("../resource/group_dialog.glade")]
struct UI {
    dialog: gtk::Dialog,
    cancel: Button,
    save: Button,
    editor: View,
    revealer: Revealer,
    info_bar: InfoBar,
    error: Label,
}

struct State {
    text: Cow<'static, str>,
    language: &'static str,
    save_handler: Option<Box<dyn Fn(&str) -> bool + 'static>>,
}

pub struct Dialog {
    ui: UI,
    state: RefCell<State>,
}

impl Dialog {
    pub fn new(text: Cow<'static, str>, language: &'static str) -> Self {
        Dialog {
            ui: UI::new(),
            state: RefCell::new(State {
                text,
                language,
                save_handler: None,
            }),
        }
    }

    pub fn on_save(&self, f: impl Fn(&str) -> bool + 'static) {
        self.state.borrow_mut().save_handler = Some(Box::new(f));
    }

    pub fn show_error(&self, error: impl AsRef<str>) {
        self.ui.error.set_text(error.as_ref());
        self.ui.revealer.set_reveal_child(true);
    }

    pub fn setup(self: &Rc<Self>) {
        self.setup_editor();

        let this = self.clone();
        self.ui.info_bar.connect_response(move |_, _| {
            this.ui.revealer.set_reveal_child(false);
        });

        let this = self.clone();
        self.ui.cancel.connect_clicked(move |_| {
            this.ui.dialog.response(ResponseType::Cancel);
        });

        let this = self.clone();
        self.ui.save.connect_clicked(move |_| {
            let text = this.ui.editor.text().unwrap();
            match &this.state.borrow().save_handler {
                Some(f) if f(text.as_str()) => this.response(ResponseType::Ok),
                None => this.response(ResponseType::Cancel),
                _ => (),
            }
        });
    }

    fn setup_editor(&self) {
        self.ui.editor.set_language(self.state.borrow().language);
        self.ui.editor.set_theme("kate");

        let buffer = self.ui.editor.buffer().unwrap();
        buffer.set_text(&*self.state.borrow().text);
    }
}

impl Deref for Dialog {
    type Target = gtk::Dialog;

    fn deref(&self) -> &Self::Target {
        &self.ui.dialog
    }
}
