# hello-web

A minimal **two-actor HTTP server on [Theater](https://github.com/colinrozzi/theater)** —
the canonical starting template for building a web-facing Theater actor. Fork it,
replace the handler's response, ship.

Built and verified on Theater rev `00b0bf9` (packr 0.24, in-module-state ABI).

## Shape

Actor-per-connection HTTP, split across two actors:

- **`acceptor/`** — listens on TCP, and for each connection `supervisor.spawn`s a
  fresh handler and `tcp.transfer-async`es the connection to it (non-blocking, so
  a slow client can't wedge the accept loop). *Stateful*: `#[derive(State)]` cell
  holding its config, set once in `init`, plus a `get-state` export.
- **`handler/`** — receives the transferred connection, reads one HTTP request,
  responds, and shuts down. *Stateless / opaque*: no state cell. This is where
  your content goes — edit `handler/src/lib.rs`.

The two actors demonstrate both in-module-state patterns: a **stateful** actor
(acceptor) and a **stateless** one (handler).

## Build

```sh
nix build 'path:.'    # -> result/hello_web_acceptor.wasm + result/hello_web_handler.wasm
```

Always use `path:.` (not plain `nix build`) so untracked working-tree files are
included. `nix develop` gives you a shell with the rust toolchain + `theater`.

## Run locally

From the repo root (paths in the local manifests are relative to it):

```sh
theater spawn acceptor/manifest.local.toml
curl localhost:9444
```

You should get the placeholder `hello from Theater` page.

## Make it yours

- **Content**: edit `BODY` in `handler/src/lib.rs`, or add path routing by
  splitting the request line and matching on the path.
- **Baked content** (like a real site): generate `.html` at build time and pull
  it in with `include_str!` — add a small build step and include the outputs.
- **Port / TLS**: the acceptor reads `listen` / `handler` / `tls` from its
  manifest `initial_state`. For public TLS, add `server_tls` (cert/key) to the
  acceptor's `tcp` handler in a prod manifest and pass `tls=on`.

## The five things a fresh actor must get right (baked into this template)

1. Exports take only their own args and return a bare `value` (no state
   threading); imports are `theater:simple/self` (not `runtime`).
2. State lives in a `#[derive(State)]` cell (see the acceptor); `get-state` is
   declared in `pack_types!` so the runtime can discover it.
3. `result<T, E>` returned from an import decodes as `Value::Result { Ok/Err }`,
   not `Value::Variant` (see the acceptor's `supervisor.spawn` decode).
4. Manifests use `[[handler]] type = "self"` (renamed from `runtime`).
5. Control capabilities (`supervisor`, `runtime`) default-DENY — grant them in
   the manifest (`[permission_policy.supervisor] type = "inherit"`) or calls fail
   with `permission-denied`. A freshly accepted connection served in-place must
   also be `activate`d first — the transfer path here handles that for you.
