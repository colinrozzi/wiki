//! wiki handler — receives a transferred TCP connection, reads one HTTP
//! request, responds with a placeholder page, and shuts down. Stateless /
//! opaque, actor-per-connection.
//!
//! This is the minimal template: replace `BODY` (or add path routing in
//! `handle_connection_transfer`) with your own content. For a real site you can
//! bake content in via `include_str!` + a build step (see the README).

#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use packr_guest::{export, import, pack_types, Value, ValueType};

packr_guest::setup_guest!();

// Stateless / opaque actor: no `#[derive(State)]`, no `get-state` export.

/// `result<_, string>::ok(())` — the no-state success return every export uses.
fn ok_unit() -> Value {
    let unit = Value::Tuple(vec![]);
    Value::Result {
        ok_type: unit.infer_type(),
        err_type: ValueType::String,
        value: Ok(Box::new(unit)),
    }
}

pack_types! {
    imports {
        theater:simple/self {
            log: func(msg: string),
            shutdown: func(data: option<list<u8>>) -> result<_, string>,
        }
        theater:simple/tcp {
            receive: func(connection-id: string, max-bytes: u32) -> result<list<u8>, string>,
            send: func(connection-id: string, data: list<u8>) -> result<u64, string>,
            close: func(connection-id: string) -> result<_, string>,
        }
    }
    exports {
        theater:simple/actor.init: func(config: value) -> result<_, string>,
        theater:simple/tcp-client.handle-connection-transfer: func(connection-id: string) -> result<_, string>,
    }
}

#[import(module = "theater:simple/self", name = "log")]
fn log(msg: String);

#[import(module = "theater:simple/self", name = "shutdown")]
fn shutdown(data: Option<Vec<u8>>) -> Result<(), String>;

#[import(module = "theater:simple/tcp", name = "receive")]
fn tcp_receive(connection_id: String, max_bytes: u32) -> Result<Vec<u8>, String>;

#[import(module = "theater:simple/tcp", name = "send")]
fn tcp_send(connection_id: String, data: Vec<u8>) -> Result<u64, String>;

#[import(module = "theater:simple/tcp", name = "close")]
fn tcp_close(connection_id: String) -> Result<(), String>;

// Baked fleet-wiki front end (v1 read view): a full standalone HTML document
// (self-contained, no external deps) rendering the fleet wiki pages. Pulled in
// at build time; regenerate via /repo/frontend (see README).
const BODY: &str = include_str!("wiki.html");

#[export(name = "theater:simple/actor.init")]
fn init(_config: Value) -> Value {
    ok_unit()
}

#[export(name = "theater:simple/tcp-client.handle-connection-transfer")]
fn handle_connection_transfer(connection_id: String) -> Value {
    // Read (and ignore) the request line; this template serves one page for any
    // path. Add routing here — split the request line, match on the path.
    let _ = tcp_receive(connection_id.clone(), 8192);

    let body = BODY.as_bytes();
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);

    if let Err(e) = tcp_send(connection_id.clone(), response) {
        log(format!("[wiki-handler] send failed: {}", e));
    }
    let _ = tcp_close(connection_id);
    let _ = shutdown(None);
    ok_unit()
}
