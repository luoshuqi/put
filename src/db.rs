use crate::app_dir;
use sqlite::Connection;

pub fn init() {
    let conn = connection();
    let sql = include_str!("../resource/db.sql");
    conn.execute(sql).expect("create table failed");
}

pub fn connection() -> Connection {
    let mut dir = app_dir();
    dir.push("database");
    sqlite::open(dir).expect("open database failed")
}
