//! Create an in-memory database connection and serve it using [`axum`].
//!
//! [`axum`]: https://docs.rs/axum

use std::fmt::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Extension, Router};
use sqll::{OpenOptions, Prepare, Statement};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

struct Inner {
    select_users: Statement,
}

#[derive(Clone)]
struct Database {
    inner: Arc<Mutex<Inner>>,
}

fn setup_db() -> Result<Database> {
    // SAFETY: We set up an unsynchronized connection which is unsafe, but we
    // provide external syncrhonization so it is fine. This avoids the overhead
    // of sqlite using internal locks.
    let conn = unsafe {
        OpenOptions::new()
            .create()
            .read_write()
            .extended_result_codes()
            .unsynchronized()
            .open_memory()?
    };

    conn.execute(
        r#"
        CREATE TABLE users (
            name TEXT PRIMARY KEY NOT NULL,
            age INTEGER
        );
        INSERT INTO users VALUES ('Alice', 42);
        INSERT INTO users VALUES ('Bob', 69);
        INSERT INTO users VALUES ('Charlie', 21);
        "#,
    )?;

    let select_users = conn.prepare_with("SELECT name, age FROM users", Prepare::PERSISTENT)?;

    let inner = Inner { select_users };

    Ok(Database {
        inner: Arc::new(Mutex::new(inner)),
    })
}

struct WebError {
    kind: WebErrorKind,
}

impl IntoResponse for WebError {
    fn into_response(self) -> axum::response::Response {
        match self.kind {
            WebErrorKind::DatabaseError(err) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
                .into_response(),
            WebErrorKind::Format => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Formatting error",
            )
                .into_response(),
        }
    }
}

enum WebErrorKind {
    DatabaseError(sqll::Error),
    Format,
}

impl From<sqll::Error> for WebError {
    fn from(err: sqll::Error) -> Self {
        WebError {
            kind: WebErrorKind::DatabaseError(err),
        }
    }
}

impl From<fmt::Error> for WebError {
    fn from(_: fmt::Error) -> Self {
        WebError {
            kind: WebErrorKind::Format,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = setup_db()?;

    let app = Router::new().route("/", get(get_user)).layer(Extension(db));
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("Listening on http://{}", listener.local_addr()?);
    axum::serve::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn get_user(Extension(db): Extension<Database>) -> Result<Html<String>, WebError> {
    let mut out = String::with_capacity(1024);

    let mut db = db.inner.lock().await;
    db.select_users.reset()?;

    writeln!(out, "<!DOCTYPE html>")?;
    writeln!(out, "<html>")?;
    writeln!(out, "<head><title>User List</title></head>")?;
    writeln!(out, "<body>")?;

    while let Some(row) = db.select_users.next()? {
        let name: String = row.read(0)?;
        let age: i64 = row.read(1)?;
        writeln!(out, "<div>Name: {name}, Age: {age}</div>")?;
    }

    writeln!(out, "</body>")?;
    writeln!(out, "</html>")?;
    Ok(Html(out))
}
