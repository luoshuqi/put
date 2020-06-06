use crate::dialog::Dialog;
use crate::{config, Group};
use std::collections::HashSet;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone)]
pub struct GroupDialog {
    dialog: Rc<Dialog>,
}

impl GroupDialog {
    pub fn new() -> Self {
        let dlg = GroupDialog {
            dialog: Rc::new(Dialog::new(config::read_group_fallback(), "json")),
        };
        let this = dlg.clone();
        dlg.on_save(move |text| this.save(text));
        dlg.setup();
        dlg
    }

    pub fn save(&self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return self.write_group(text);
        }

        match serde_json::from_str::<Vec<Group>>(text) {
            Ok(v) => {
                if let Some(reason) = check_group(&v) {
                    self.show_error(reason);
                    false
                } else {
                    self.write_group(&serde_json::to_string_pretty(&v).unwrap())
                }
            }
            Err(e) => {
                self.show_error(format!("格式错误：{}", e));
                false
            }
        }
    }

    pub fn write_group(&self, s: &str) -> bool {
        if let Err(e) = config::save_group(s) {
            self.show_error(e.to_string());
            false
        } else {
            true
        }
    }
}

impl Deref for GroupDialog {
    type Target = Rc<Dialog>;

    fn deref(&self) -> &Self::Target {
        &self.dialog
    }
}

fn is_group_id_unique(group: &Vec<Group>) -> bool {
    let g: HashSet<&String> = group.iter().map(|x| &x.id).collect();
    g.len() == group.len()
}

fn check_group(group: &Vec<Group>) -> Option<&str> {
    if !is_group_id_unique(group) {
        Some("id不能重复")
    } else if group.iter().find(|x| x.id.is_empty()).is_some() {
        Some("id不能为空")
    } else {
        None
    }
}
