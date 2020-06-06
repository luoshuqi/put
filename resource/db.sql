CREATE TABLE IF NOT EXISTS request (
    group_id TEXT NOT NULL,
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT "",
    sort INTEGER NOT NULL,
    request TEXT NOT NULL,
    response TEXT NOT NULL,
    UNIQUE(group_id, method, url)
);