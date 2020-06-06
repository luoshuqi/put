use crate::{config, db, CssDialog, Editor, Group, GroupDialog, Request, RequestStore, Response};
use gdk::EventButton;
use gio::prelude::*;
use gio::SimpleAction;
use glade_macro::ui;
use glib::{BindingFlags, MainContext, Sender, SignalHandlerId};
use gtk::{
    prelude::*, ApplicationWindow, Button, CellRendererTextBuilder, ComboBoxText, CssProvider,
    Entry, InfoBar, Label, Menu, MenuItem, Popover, ResponseType, Revealer, SearchBar, SearchEntry,
    Spinner, StyleContext, TextView, ToggleButton, TreeIter, TreeView, TreeViewColumn,
};
use serde_json::Value;
use std::{cell::RefCell, ops::Deref, ops::DerefMut, rc::Rc};

macro_rules! action {
    ($window:expr, $name:expr, $slot:block) => {{
        let action = SimpleAction::new($name, None);
        action.connect_activate(move |_, _| $slot);
        $window.add_action(&action);
    }};
}

#[ui("../resource/window.glade")]
struct UI {
    window: ApplicationWindow,
    group: ComboBoxText,
    search: ToggleButton,
    search_bar: SearchBar,
    search_entry: SearchEntry,
    request: sourceview::View,
    response: sourceview::View,
    run: Button,
    spinner: Spinner,
    cancel: Button,
    revealer: Revealer,
    info_bar: InfoBar,
    error: Label,
    status: Label,
    time: Label,
    header_popover: Popover,
    header: TextView,
    tree: TreeView,
    tree_menu: Menu,
    menu_delete: MenuItem,
    menu_rename: MenuItem,
    rename_popover: Popover,
    rename_entry: Entry,
}

type Msg = (Result<Response, String>, u32);

#[derive(Default)]
struct State {
    group: Option<Vec<Group>>,
    group_id: Option<String>,
    request: Option<Request>,
    header: Option<String>,
    sender: Option<Sender<Msg>>,
    store: RefCell<RequestStore>,
    iter: Option<TreeIter>,
    sig_group: Option<SignalHandlerId>,
    provider: Option<CssProvider>,
}

pub struct Window {
    ui: UI,
    state: RefCell<State>,
}

impl Window {
    pub fn new() -> Self {
        Window {
            ui: UI::new(),
            state: RefCell::new(Default::default()),
        }
    }

    pub fn setup(self: &Rc<Self>) {
        db::init();
        self.restore_window_size();
        self.setup_channel();
        self.setup_signal();
        self.setup_info_bar();
        self.setup_editor();
        self.create_action();
        self.setup_tree();
        self.setup_group();

        match self.apply_css() {
            Err(e) => self.show_error(e.to_string()),
            _ => (),
        }
    }

    fn restore_window_size(&self) {
        let s = config::State::get();

        if let (Some(with), Some(height)) = (s.window_width, s.window_height) {
            self.ui.window.set_default_size(with, height);
        } else {
            self.ui.window.set_default_size(1024, 600);
        }

        if let Some(true) = s.window_maximized {
            self.ui.window.maximize();
        }
    }

    fn setup_channel(self: &Rc<Self>) {
        let (sender, receiver) = MainContext::channel(glib::PRIORITY_DEFAULT);
        self.state.borrow_mut().sender = Some(sender);

        let this = self.clone();
        receiver.attach(None, move |response| {
            this.on_response(response);
            glib::Continue(true)
        });
    }

    fn setup_signal(self: &Rc<Self>) {
        self.ui
            .search
            .bind_property("active", &self.ui.search_bar, "search-mode-enabled")
            .flags(BindingFlags::BIDIRECTIONAL)
            .build();

        let this = self.clone();
        self.ui.search_entry.connect_search_changed(move |_| {
            this.refresh_tree();
        });

        let this = self.clone();
        self.ui.run.connect_clicked(move |_| {
            this.on_request();
        });

        let this = self.clone();
        self.ui.cancel.connect_clicked(move |_| {
            this.post_request(None);
        });

        let this = self.clone();
        self.ui.status.connect_activate_link(move |_, _| {
            if let Some(header) = &this.state.borrow().header {
                this.ui
                    .header
                    .get_buffer()
                    .iter()
                    .for_each(|x| x.set_text(&header));
                this.ui.header_popover.show();
            }
            Inhibit(true)
        });

        let this = self.clone();
        let id = self.ui.group.connect_changed(move |_| {
            this.state.borrow_mut().group_id = this.ui.group.get_active_id().map(|x| x.to_string());
            this.refresh_tree();
        });
        self.state.borrow_mut().sig_group = Some(id);

        let this = self.clone();
        self.ui.menu_delete.connect_activate(move |_| {
            this.handle_tree_menu(MenuAction::Delete);
        });

        let this = self.clone();
        self.ui.menu_rename.connect_activate(move |_| {
            this.handle_tree_menu(MenuAction::Rename);
        });

        let this = self.clone();
        self.ui
            .rename_entry
            .connect_key_press_event(move |entry, key| {
                if key.get_keyval() == gdk::enums::key::Return {
                    if let Some(text) = entry.get_text() {
                        this.handle_tree_menu(MenuAction::DoRename(text.to_string()));
                        this.ui.rename_popover.hide();
                    }
                }
                Inhibit(false)
            });

        let this = self.clone();
        self.ui.window.connect_size_allocate(move |_, _| {
            let mut s = config::State::get();
            if s.window_maximized != Some(true) {
                let (w, h) = this.ui.window.get_size();
                s.window_width = Some(w);
                s.window_height = Some(h);
            }
        });

        self.ui.window.connect_window_state_event(|_, e| {
            config::State::get().window_maximized = Some(
                e.get_new_window_state()
                    .contains(gdk::WindowState::MAXIMIZED),
            );
            Inhibit(false)
        });

        self.ui.window.connect_delete_event(|_, _| {
            config::State::save();
            Inhibit(false)
        });
    }

    fn handle_tree_menu(&self, action: MenuAction) -> Option<()> {
        let state = self.state.borrow();
        let iter = state.iter.as_ref()?;
        let mut store = state.store.borrow_mut();
        match action {
            MenuAction::Delete => store.delete(iter),
            MenuAction::DoRename(name) => store.rename(iter, &name),
            MenuAction::Rename => {
                let tree = &self.ui.tree;
                let model = tree.get_model()?;
                let title = model.get_value(iter, 2).get::<String>().ok()??;
                self.ui.rename_entry.set_text(&title);

                let path = model.get_path(iter);
                let rect = tree.get_cell_area(path.as_ref(), tree.get_column(0).as_ref());
                self.ui.rename_popover.set_pointing_to(&rect);
                self.ui.rename_popover.set_relative_to(Some(tree));
                self.ui.rename_popover.popup();
                Some(())
            }
        }
    }

    fn create_action(self: &Rc<Self>) {
        let this = self.clone();
        action!(self, "group", {
            this.handle_group_action();
        });

        let this = self.clone();
        action!(self, "font", {
            this.handle_font_action();
        });
    }

    fn handle_group_action(&self) {
        let dlg = GroupDialog::new();
        let response = dlg.run();
        dlg.hide();
        if response == ResponseType::Ok {
            self.setup_group();
        }
    }

    fn handle_font_action(&self) {
        let dlg = CssDialog::new();
        let response = dlg.run();
        dlg.hide();
        if response == ResponseType::Ok {
            if let Err(e) = self.apply_css() {
                self.show_error(e.to_string());
            }
        }
    }

    fn setup_editor(&self) {
        self.ui.request.set_language("yaml");
        self.ui.request.set_theme("kate");
        self.ui.response.set_theme("kate");
    }

    fn setup_group(&self) {
        glib::signal_handler_block(
            &self.ui.group,
            self.state.borrow().sig_group.as_ref().unwrap(),
        );
        self.ui.group.remove_all();

        let groups = config::get_group_with_default();
        let mut current_valid = false;
        for group in &groups {
            self.ui.group.append(Some(&group.id), &group.name);
            current_valid = current_valid || option_eq(&self.state.borrow().group_id, &group.id);
        }
        glib::signal_handler_unblock(
            &self.ui.group,
            self.state.borrow().sig_group.as_ref().unwrap(),
        );

        if !current_valid {
            self.ui.group.set_active(Some(0));
            self.state.borrow_mut().group_id = Some(groups[0].id.clone())
        } else {
            let group_id = self.state.borrow().group_id.as_ref().map(|x| x.clone());
            self.ui
                .group
                .set_active_id(group_id.as_ref().map(|x| x.as_str()));
        }

        self.state.borrow_mut().group = Some(groups);
    }

    fn setup_info_bar(self: &Rc<Self>) {
        let this = self.clone();
        self.ui.info_bar.connect_response(move |_, _| {
            this.ui.revealer.set_reveal_child(false);
        });
    }

    fn show_error(&self, error: impl AsRef<str>) {
        self.ui.error.set_text(error.as_ref());
        self.ui.revealer.set_reveal_child(true);
    }

    fn pre_request(&self, request: Request) {
        self.state.borrow_mut().request = Some(request);

        self.ui.revealer.set_reveal_child(false);
        self.ui.run.set_visible(false);
        self.ui.cancel.set_visible(true);
        self.ui.spinner.set_visible(true);
        self.ui.status.set_visible(false);
        self.ui.time.set_visible(false);
        self.ui.response.set_text("");
    }

    fn render_response(&self, s: &str) {
        if let Some(json) = json_pretty(s) {
            self.ui.response.set_language("json");
            self.ui.response.set_text(&json);
        } else {
            self.ui.response.set_language("");
            self.ui.response.set_text(s);
        }
    }

    fn post_request(&self, response: Option<Response>) {
        self.ui.run.set_visible(true);
        self.ui.cancel.set_visible(false);
        self.ui.spinner.set_visible(false);

        if let Some(response) = response {
            if let Some(request) = &self.state.borrow().request {
                let state = self.state.borrow();
                let mut store = state.store.borrow_mut();
                match store.put(request, &response) {
                    Ok(..) => (),
                    Err(e) => self.show_error(e.to_string()),
                };
                store.select_set_top(&request.method, &request.url, &self.ui.tree);
            }

            if let Some(status) = response.status() {
                self.ui
                    .status
                    .set_markup(&format!("<a href='#'>{}</a>", status));
                self.ui.status.set_visible(true);
                self.state.borrow_mut().header = Some(response.header);
            }
            self.ui.time.set_visible(true);
            self.ui.time.set_text(&format!("{} ms", response.time));

            self.render_response(&response.body);
        }

        self.state.borrow_mut().request = None;
    }

    fn on_request(&self) {
        let text = String::from(self.ui.request.text().unwrap());
        if text.as_str().trim().is_empty() {
            return;
        }

        match Request::parse(text, self.group()) {
            Ok(req) => {
                self.pre_request(req.clone());
                let sender = self.sender();
                std::thread::spawn(move || {
                    sender
                        .send((req.perform().map_err(|e| e.to_string()), req.id))
                        .expect("send failed");
                });
            }
            Err(e) => self.show_error(e.to_string()),
        };
    }

    fn on_response(&self, msg: Msg) {
        let req_id = self.state.borrow().request.as_ref().map(|x| x.id);
        if Some(msg.1) != req_id {
            return;
        }

        match msg.0 {
            Ok(response) => self.post_request(Some(response)),
            Err(msg) => {
                self.show_error(msg);
                self.post_request(None);
            }
        }
    }

    #[inline]
    fn sender(&self) -> Sender<Msg> {
        self.state.borrow().sender.as_ref().unwrap().clone()
    }

    fn group(&self) -> Option<Group> {
        let state = self.state.borrow();
        let id = state.group_id.as_ref()?;
        let groups = state.group.as_ref()?;
        let group = groups.iter().find(|x| &x.id == id)?;
        Some(group.clone())
    }

    fn setup_tree(self: &Rc<Self>) {
        let column = TreeViewColumn::new();
        let cell = CellRendererTextBuilder::new().ypad(6).build();
        column.pack_start(&cell, true);
        column.add_attribute(&cell, "text", 2);
        self.ui.tree.append_column(&column);
        self.ui
            .tree
            .set_model(Some(self.state.borrow().store.borrow().get_store()));

        let this = self.clone();

        fn handle_button_press_event(
            view: &TreeView,
            e: &EventButton,
            this: &Window,
        ) -> Option<()> {
            let (x, y) = e.get_position();
            let t = view.get_path_at_pos(x as i32, y as i32)?;
            let path = t.0?;
            let store = view.get_model()?;
            let iter = store.get_iter(&path)?;
            let method = store.get_value(&iter, 0).get::<String>().ok()??;
            let url = store.get_value(&iter, 1).get::<String>().ok()??;
            if e.get_button() == 1 || e.get_button() == 3 {
                match view.get_selection().get_selected() {
                    Some((_, it)) => {
                        if iter != it {
                            this.display_request(&method, &url);
                        }
                    }
                    None => {
                        this.display_request(&method, &url);
                    }
                }

                if e.get_button() == 3 {
                    this.ui.tree_menu.popup_easy(3, e.get_time())
                }

                this.state.borrow_mut().iter = Some(iter);
            }
            Some(())
        }

        self.ui.tree.connect_button_press_event(move |view, e| {
            handle_button_press_event(view, e, &this);
            Inhibit(false)
        });
    }

    fn refresh_tree(&self) {
        let group_id = self.ui.group.get_active_id();
        let filter = self.ui.search_entry.get_text();
        let state = self.state.borrow();
        let mut store = state.store.borrow_mut();
        match store.load(group_id, filter) {
            Err(e) => self.show_error(e.to_string()),
            _ => (),
        };
    }

    fn display_request(&self, method: &str, url: &str) -> Option<()> {
        let state = self.state.borrow();
        let store = state.store.borrow();
        let (req, res) = store.find(method, url).ok()??;
        self.ui.request.set_text(&req);
        self.render_response(&res);
        self.ui.status.hide();
        self.ui.time.hide();
        Some(())
    }

    fn apply_css(&self) -> Result<(), glib::Error> {
        let screen = gdk::Screen::get_default().unwrap();

        let mut state = self.state.borrow_mut();
        let provider = match &state.provider {
            Some(p) => {
                StyleContext::remove_provider_for_screen(&screen, p);
                p
            }
            None => {
                state.provider = Some(CssProvider::new());
                state.provider.as_ref().unwrap()
            }
        };

        provider.load_from_data(config::read_css_fallback().as_bytes())?;
        StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().unwrap(),
            provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );

        Ok(())
    }
}

impl Deref for Window {
    type Target = ApplicationWindow;

    fn deref(&self) -> &Self::Target {
        &self.ui.window
    }
}

impl DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui.window
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        config::State::save();
    }
}

fn option_eq<T: PartialEq<U>, U: ?Sized>(x: &Option<T>, y: &U) -> bool {
    match x {
        Some(x) if x == y => true,
        _ => false,
    }
}

fn json_pretty(s: &str) -> Option<String> {
    let json: Value = serde_json::from_str(s).ok()?;
    serde_json::to_string_pretty(&json).ok()
}

enum MenuAction {
    Delete,
    Rename,
    DoRename(String),
}
