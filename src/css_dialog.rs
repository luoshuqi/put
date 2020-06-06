use crate::config;
use crate::dialog::Dialog;
use gtk::{prelude::*, CssProvider};
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone)]
pub struct CssDialog {
    dialog: Rc<Dialog>,
}

impl CssDialog {
    pub fn new() -> Self {
        let dlg = CssDialog {
            dialog: Rc::new(Dialog::new(config::read_css_fallback(), "css")),
        };
        let this = dlg.clone();
        dlg.on_save(move |text| this.save(text));
        dlg.setup();
        dlg
    }

    fn save(&self, text: &str) -> bool {
        let provider = CssProvider::new();
        match provider.load_from_data(text.as_bytes()) {
            Ok(_) => {
                if let Err(e) = config::save_css(text) {
                    self.show_error(e.to_string());
                    false
                } else {
                    true
                }
            }
            Err(e) => {
                self.show_error(e.to_string());
                false
            }
        }
    }
}

impl Deref for CssDialog {
    type Target = Rc<Dialog>;

    fn deref(&self) -> &Self::Target {
        &self.dialog
    }
}
