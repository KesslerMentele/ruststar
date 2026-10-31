CREATE TABLE IF NOT EXISTS todos (
    id        BLOB    PRIMARY KEY NOT NULL,
    title     TEXT    NOT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS users (
    id        BLOB PRIMARY KEY NOT NULL,
    name      TEXT NOT NULL,
    pass_hash TEXT NOT NULL
);
