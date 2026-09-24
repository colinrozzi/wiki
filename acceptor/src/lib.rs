//! Acceptor: listens on a TCP port; on each connection, spawns a handler
//! actor and transfers the connection to it. The handler then handles one
//! HTTP request and shuts down.

#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use packr_guest::{export, import, pack_types, GraphValue, Value, ValueType};
use theater_guest::State;

packr_guest::setup_guest!();

/// The acceptor's state lives in-module now (`docs/in-module-state.md`): a
/// `#[derive(State)]` cell, set once in `init` and read on each connection —
/// nothing is threaded through the call. The derive also emits the
/// `theater:simple/actor.get-state` export (declared in `pack_types!` below so
/// the runtime discovers it), keeping the acceptor inspectable.
#[derive(Clone, GraphValue, State)]
pub struct AcceptorState {
    pub listener_id: String,
    pub handler_manifest: String,
}

/// `result<_, string>::ok(())` — the no-state success return.
fn ok_unit() -> Value {
    let unit = Value::Tuple(vec![]);
    Value::Result {
        ok_type: unit.infer_type(),
        err_type: ValueType::String,
        value: Ok(Box::new(unit)),
    }
}

/// `result<_, string>::err(msg)` — the no-state failure return.
fn err_unit(msg: String) -> Value {
    Value::Result {
        ok_type: Value::Tuple(vec![]).infer_type(),
        err_type: ValueType::String,
        value: Err(Box::new(Value::String(msg))),
    }
}

pack_types! {
    // Mirror theater:simple/supervisor's error types EXACTLY (case names + order)
    // so the interface subset hash for `spawn` matches the handler-crate pact.
    // supervisor.spawn now returns result<string, supervisor-error> (structured
    // errors), not result<string, string> — get this wrong and the actor
    // compiles but fails to instantiate (interface-hash mismatch at spawn).
    variant spawn-failure {
        bad-manifest(string),
        wasm-fetch(string),
        handler-registry(string),
        wasm-invalid(string),
        interface-mismatch(string),
        missing-interface(string),
        missing-metadata(string),
        init-failed(string),
        child-failed(string),
        child-stopped(string),
        timeout(string),
        internal(string),
    }

    variant supervisor-error {
        actor-not-found(string),
        out-of-view(string),
        permission-denied(string),
        invalid-argument(string),
        spawn-failed(spawn-failure),
        runtime-unavailable,
        internal(string),
    }

    imports {
        theater:simple/self {
            log: func(msg: string),
        }
        theater:simple/tcp {
            listen: func(address: string) -> result<string, string>,
            transfer: func(connection-id: string, target-actor: string) -> result<_, string>,
            transfer-async: func(connection-id: string, target-actor: string) -> result<_, string>,
        }
        theater:simple/supervisor {
            spawn: func(manifest: string, init-state: option<value>, wasm-bytes: option<list<u8>>) -> result<string, supervisor-error>,
        }
        theater:simple/rpc {
            call: func(actor-id: string, function: string, params: value, options: value) -> value,
        }
    }
    exports {
        theater:simple/actor.init: func(config: value) -> result<_, string>,
        theater:simple/actor.get-state: func() -> value,
        theater:simple/tcp-client.handle-connection: func(connection-id: string) -> result<_, string>,
    }
}

#[import(module = "theater:simple/self", name = "log")]
fn log(msg: String);

#[import(module = "theater:simple/tcp", name = "listen")]
fn tcp_listen(address: String) -> Result<String, String>;

#[import(module = "theater:simple/tcp", name = "transfer")]
fn tcp_transfer(connection_id: String, target_actor: String) -> Result<(), String>;

#[import(module = "theater:simple/tcp", name = "transfer-async")]
fn tcp_transfer_async(connection_id: String, target_actor: String) -> Result<(), String>;

// spawn now returns result<string, supervisor-error> (a structured variant), a
// complex return type — import it as a raw Value and decode: Ok(string) = the
// child id, Err(supervisor-error) = surface the variant case name.
#[import(module = "theater:simple/supervisor", name = "spawn")]
fn supervisor_spawn_raw(
    manifest: String,
    init_state: Option<Value>,
    wasm_bytes: Option<Vec<u8>>,
) -> Value;

fn supervisor_spawn(
    manifest: String,
    init_state: Option<Value>,
    wasm_bytes: Option<Vec<u8>>,
) -> Result<String, String> {
    // result<string, supervisor-error> comes back as Value::Result (packr-abi
    // 0.24), NOT a generic Variant.
    match supervisor_spawn_raw(manifest, init_state, wasm_bytes) {
        Value::Result { value: Ok(ok), .. } => match *ok {
            // Ok(string) — the new child's id.
            Value::String(id) => Ok(id),
            _ => Err(String::from("unexpected ok payload shape")),
        },
        Value::Result { value: Err(err), .. } => {
            // The supervisor-error is a variant; surface its case name.
            let case = match *err {
                Value::Variant { case_name, .. } => case_name,
                _ => String::from("unknown"),
            };
            Err(format!("supervisor-error: {}", case))
        }
        _ => Err(String::from("unexpected result format")),
    }
}

#[import(module = "theater:simple/rpc", name = "call")]
fn rpc_call(actor_id: String, function: String, params: Value, options: Value) -> Value;

// Defaults reproduce today's hardcoded prod behavior when init-state is absent,
// so a manifest that omits `initial_state` behaves exactly as before.
const DEFAULT_LISTEN: &str = "0.0.0.0:9444";
const DEFAULT_HANDLER: &str = "handler/manifest.toml";
const DEFAULT_TLS: bool = true;

/// Pull `key=value` out of the init-state config string. The config is a set of
/// `key=value` pairs separated by `;` or newlines, e.g.
/// `listen=0.0.0.0:9444;handler=/repo/handler/manifest.local.toml;tls=off`.
/// Values (paths, host:port) never contain `;`/`=`, so this stays unambiguous
/// without pulling in a JSON parser (no new deps -> no lockfile regen needed).
fn field(cfg: &str, key: &str) -> Option<String> {
    for part in cfg.split(|c| c == ';' || c == '\n') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(key) {
            if let Some(val) = rest.trim_start().strip_prefix('=') {
                return Some(String::from(val.trim()));
            }
        }
    }
    None
}

#[export(name = "theater:simple/actor.init")]
fn init(config: Value) -> Value {
    log(String::from("[hello-web-acceptor] init"));

    // The manifest's `initial_state` string arrives here verbatim as a
    // Value::String (see theater-cli spawn.rs). Anything else -> use defaults.
    let cfg = match config {
        Value::String(s) => s,
        _ => String::new(),
    };

    let listen_addr = field(&cfg, "listen").unwrap_or_else(|| String::from(DEFAULT_LISTEN));
    let handler_manifest = field(&cfg, "handler").unwrap_or_else(|| String::from(DEFAULT_HANDLER));
    // TLS is enforced by the tcp handler's `server_tls` in the manifest (the
    // guest's listen() can't toggle it); this flag records intent + logs it so
    // prod (tls=on) and local (tls=off) are self-describing.
    let tls = match field(&cfg, "tls") {
        Some(v) => matches!(v.as_str(), "on" | "true" | "1" | "yes"),
        None => DEFAULT_TLS,
    };

    let listener_id = match tcp_listen(listen_addr.clone()) {
        Ok(id) => id,
        Err(e) => return err_unit(format!("listen failed: {}", e)),
    };
    log(format!(
        "[hello-web-acceptor] listening on {} (id={}, tls={}, handler={})",
        listen_addr, listener_id, tls, handler_manifest
    ));

    // Set the in-module state cell once; handle-connection reads it via `with`.
    AcceptorState::set(AcceptorState {
        listener_id,
        handler_manifest,
    });
    ok_unit()
}

#[export(name = "theater:simple/tcp-client.handle-connection")]
fn handle_connection(connection_id: String) -> Value {
    log(format!(
        "[hello-web-acceptor] new connection {}",
        connection_id
    ));

    // Read the handler-manifest path out of the in-module state cell.
    let handler_manifest = AcceptorState::with(|s| s.handler_manifest.clone());

    // Spawn a handler actor for this connection.
    let handler_id = match supervisor_spawn(handler_manifest, None, None) {
        Ok(id) => id,
        Err(e) => return err_unit(format!("spawn handler failed: {}", e)),
    };
    log(format!(
        "[hello-web-acceptor] spawned handler {}",
        handler_id
    ));

    // Initialize it. supervisor.spawn does not auto-call init for spawned
    // children — we RPC it ourselves before the connection is transferred. The
    // handler is stateless; its init ignores this config.
    let init_params = Value::Tuple(vec![Value::Option {
        inner_type: ValueType::List(Box::new(ValueType::U8)),
        value: None,
    }]);
    let _ = rpc_call(
        handler_id.clone(),
        String::from("theater:simple/actor.init"),
        init_params,
        Value::Tuple(vec![]),
    );

    // Hand the connection off. The runtime will call
    // tcp-client.handle-connection-transfer on the target.
    // Non-blocking hand-off — the accept loop never awaits the handler's session,
    // so a slow/stalled client can't wedge the acceptor (mirrors smtp-acceptor).
    if let Err(e) = tcp_transfer_async(connection_id, handler_id) {
        return err_unit(format!("transfer-async failed: {}", e));
    }

    ok_unit()
}
