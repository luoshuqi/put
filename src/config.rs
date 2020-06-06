use crate::app_dir;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub base_url: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

static mut STATE: Option<State> = None;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub window_width: Option<i32>,
    pub window_height: Option<i32>,
    pub window_maximized: Option<bool>,
}

impl State {
    pub fn get() -> &'static mut State {
        unsafe {
            if STATE.is_none() {
                STATE = Some(Self::load());
            }

            match &mut STATE {
                Some(s) => s,
                _ => unreachable!(),
            }
        }
    }

    pub fn load() -> Self {
        match fs::read_to_string(state_path()) {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(s) => s,
                _ => Default::default(),
            },
            _ => Default::default(),
        }
    }

    pub fn save() {
        let s = serde_json::to_string(Self::get()).unwrap();
        fs::write(state_path(), &s).expect("write failed");
    }
}

pub fn get_group_with_default() -> Vec<Group> {
    let default = Group {
        id: "".to_string(),
        name: "默认分组".to_string(),
        base_url: None,
        env: None,
    };

    match get_group() {
        Some(mut v) => {
            v.push(default);
            v
        }
        None => vec![default],
    }
}

pub fn get_group() -> Option<Vec<Group>> {
    serde_json::from_str(&read_group()?).ok()
}

pub fn read_group() -> Option<String> {
    fs::read_to_string(group_path()).ok()
}

pub fn read_group_fallback() -> Cow<'static, str> {
    match read_group() {
        Some(g) => Cow::Owned(g),
        None => Cow::Borrowed(include_str!("../resource/group.json")),
    }
}

pub fn read_css_fallback() -> Cow<'static, str> {
    match std::fs::read_to_string(css_path()) {
        Ok(s) => Cow::Owned(s),
        _ => Cow::Borrowed(include_str!("../resource/style.css")),
    }
}

pub fn save_css(s: &str) -> std::io::Result<()> {
    fs::write(css_path(), s)
}

pub fn save_group(s: &str) -> std::io::Result<()> {
    fs::write(group_path(), s)
}

fn group_path() -> PathBuf {
    let mut file = app_dir();
    file.push("group.json");
    file
}

fn css_path() -> PathBuf {
    let mut file = app_dir();
    file.push("style.css");
    file
}

fn state_path() -> PathBuf {
    let mut file = app_dir();
    file.push("state.json");
    file
}
