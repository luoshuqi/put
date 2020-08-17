use crate::{db, Request, Response};
use glib::Value;
use gtk::prelude::*;
use gtk::{ListStore, TreeIter, TreeModel, TreeView};
use sqlite::State;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const EMPTY_GROUP: String = String::new();

pub struct RequestStore {
    map: HashMap<String, ()>,
    store: ListStore,
    group_id: String,
}

impl Default for RequestStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestStore {
    pub fn new() -> Self {
        RequestStore {
            map: HashMap::new(),
            store: ListStore::new(&[
                String::static_type(),
                String::static_type(),
                String::static_type(),
            ]),
            group_id: EMPTY_GROUP,
        }
    }

    pub fn get_store(&self) -> &ListStore {
        &self.store
    }

    pub fn load<T: AsRef<str>, U: AsRef<str>>(
        &mut self,
        group_id: Option<T>,
        filter: Option<U>,
    ) -> sqlite::Result<()> {
        let group_id = group_id
            .as_ref()
            .map(|x| x.as_ref().to_string())
            .unwrap_or(EMPTY_GROUP);
        let rows = select(&group_id, filter.as_ref().map(|x| x.as_ref()))?;

        self.store.clear();
        self.map.clear();
        for row in rows {
            self.map.insert(format!("{} {}", row.method, row.url), ());

            let mut title = row.title;
            if title.is_empty() {
                title.push_str(&row.method);
                title.push(' ');
                title.push_str(&row.url);
            }
            self.store
                .insert_with_values(None, &[0, 1, 2], &[&row.method, &row.url, &title]);
        }
        self.group_id = group_id;
        Ok(())
    }

    pub fn put(&mut self, request: &Request, response: &Response) -> sqlite::Result<()> {
        let key = format!("{} {}", request.method, request.url);
        if let None = self.map.get(&key) {
            self.store.insert_with_values(
                Some(0),
                &[0, 1, 2],
                &[&request.method, &request.url, &key],
            );
            self.map.insert(key, ());
        }

        update(request, &response.body, &self.group_id)
    }

    pub fn select_set_top(&self, method: &str, url: &str, tree_view: &TreeView) {
        fn handle_select(
            model: &TreeModel,
            iter: &TreeIter,
            method: &str,
            url: &str,
            tree_view: &TreeView,
        ) -> Option<bool> {
            let m = model.get_value(iter, 0).get::<String>().ok()??;
            let u = model.get_value(iter, 1).get::<String>().ok()??;
            if m == method && u == url {
                let title = model.get_value(iter, 2).get::<String>().ok()??;
                let model = model.downcast_ref::<ListStore>()?;
                model.remove(&iter);
                let pos = model.insert_with_values(Some(0), &[0, 1, 2], &[&m, &u, &title]);
                tree_view.get_selection().select_iter(&pos);
                Some(true)
            } else {
                Some(false)
            }
        }

        self.store.foreach(|model, _, iter| {
            handle_select(model, iter, method, url, tree_view).unwrap_or(false)
        });
    }

    #[inline]
    pub fn find(&self, method: &str, url: &str) -> sqlite::Result<Option<(String, String)>> {
        find(&self.group_id, method, url)
    }

    pub fn delete(&mut self, iter: &TreeIter) -> Option<()> {
        let store = self.store.downcast_ref::<ListStore>()?;
        let (method, url) = self.get_method_url(iter)?;
        store.remove(&iter);

        let key = format!("{} {}", method, url);
        self.map.remove(&key);

        delete(&self.group_id, &method, &url).ok()
    }

    pub fn rename(&self, iter: &TreeIter, name: &str) -> Option<()> {
        let store = self.store.downcast_ref::<ListStore>()?;
        let (method, url) = self.get_method_url(iter)?;

        if name.is_empty() {
            store.set_value(iter, 2, &Value::from(&format!("{} {}", method, url)));
        } else {
            store.set_value(iter, 2, &Value::from(name));
        }

        rename(&self.group_id, &method, &url, name).ok()
    }

    fn get_method_url(&self, iter: &TreeIter) -> Option<(String, String)> {
        let method = self.store.get_value(iter, 0).get::<String>().ok()??;
        let url = self.store.get_value(iter, 1).get::<String>().ok()??;
        Some((method, url))
    }
}

struct Row {
    method: String,
    url: String,
    title: String,
}

const SQL_SELECT_FILTER: &str = "SELECT method, url, title FROM request WHERE group_id=? AND (url LIKE ? OR title LIKE ? ) ORDER BY url LIMIT 200";
const SQL_SELECT: &str =
    "SELECT method, url, title FROM request WHERE group_id=? ORDER BY url LIMIT 200";
const SQL_REPLACE: &str = "REPLACE INTO request (group_id, method, url, sort, request, response, title) VALUES (?, ?, ?, ?, ?, ?, ?)";
const SQL_FIND: &str =
    "SELECT request,response FROM request WHERE group_id=? AND method=? AND url=?";
const SQL_FIND_NAME: &str =
    "SELECT title FROM request WHERE group_id=? AND method=? AND url=?";
const SQL_DELETE: &str = "DELETE FROM request WHERE group_id=? AND method=? AND url=?";
const SQL_RENAME: &str = "UPDATE request SET title=? WHERE group_id=? AND method=? AND url=?";

fn select(group_id: &str, filter: Option<&str>) -> sqlite::Result<Vec<Row>> {
    let conn = db::connection();
    let mut stmt = match filter {
        Some(filter) if !filter.is_empty() => {
            let filter = format!("%{}%", filter);
            let mut stmt = conn.prepare(SQL_SELECT_FILTER)?;
            stmt.bind(1, group_id)?;
            stmt.bind(2, filter.as_str())?;
            stmt.bind(3, filter.as_str())?;
            stmt
        }
        _ => {
            let mut stmt = conn.prepare(SQL_SELECT)?;
            stmt.bind(1, group_id)?;
            stmt
        }
    };

    let mut v = Vec::new();
    while stmt.next()? == State::Row {
        let method: String = stmt.read(0)?;
        let url: String = stmt.read(1)?;
        let title: String = stmt.read(2)?;
        v.push(Row { method, url, title });
    }
    Ok(v)
}

fn update(request: &Request, response: &str, group_id: &str) -> sqlite::Result<()> {
    let name = find_name(group_id, request.method.as_str(), request.url.as_str())?;

    let conn = db::connection();
    let mut stmt = conn.prepare(SQL_REPLACE)?;
    stmt.bind(1, group_id)?;
    stmt.bind(2, request.method.as_str())?;
    stmt.bind(3, request.url.as_str())?;
    //stmt.bind(4, timestamp().unwrap_or(0) as i64)?;
    stmt.bind(4, 0)?;
    stmt.bind(5, request.raw.as_str())?;
    stmt.bind(6, response)?;
    stmt.bind(7, name.as_ref().map(|x| x.as_str()))?;
    stmt.next()?;
    Ok(())
}

pub fn find(group_id: &str, method: &str, url: &str) -> sqlite::Result<Option<(String, String)>> {
    let conn = db::connection();
    let mut stmt = conn.prepare(SQL_FIND)?;
    stmt.bind(1, group_id)?;
    stmt.bind(2, method)?;
    stmt.bind(3, url)?;
    if let State::Row = stmt.next()? {
        Ok(Some((stmt.read(0)?, stmt.read(1)?)))
    } else {
        Ok(None)
    }
}

pub fn find_name(group_id: &str, method: &str, url: &str) -> sqlite::Result<Option<String>> {
    let conn = db::connection();
    let mut stmt = conn.prepare(SQL_FIND_NAME)?;
    stmt.bind(1, group_id)?;
    stmt.bind(2, method)?;
    stmt.bind(3, url)?;
    if let State::Row = stmt.next()? {
        Ok(Some(stmt.read(0)?))
    } else {
        Ok(None)
    }
}

pub fn delete(group_id: &str, method: &str, url: &str) -> sqlite::Result<()> {
    let conn = db::connection();
    let mut stmt = conn.prepare(SQL_DELETE)?;
    stmt.bind(1, group_id)?;
    stmt.bind(2, method)?;
    stmt.bind(3, url)?;
    stmt.next()?;
    Ok(())
}

pub fn rename(group_id: &str, method: &str, url: &str, name: &str) -> sqlite::Result<()> {
    let conn = db::connection();
    let mut stmt = conn.prepare(SQL_RENAME)?;
    stmt.bind(1, name)?;
    stmt.bind(2, group_id)?;
    stmt.bind(3, method)?;
    stmt.bind(4, url)?;
    stmt.next()?;
    Ok(())
}

#[allow(dead_code)]
fn timestamp() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs())
        .ok()
}
