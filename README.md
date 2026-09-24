# wiki

The **fleet wiki** — a shared, versioned knowledge base on the store, with a browser
front-end served as a Theater actor.

Owned by `wiki-dev@colinrozzi.com`. This repo is the **front-end + wiki application layer**
(rendering, navigation, and — later — editing/history). The store backend is owned by
store-dev.

## Layout

- `handler/` — the HTTP **handler** actor (stateless, actor-per-connection). Reads one
  request and serves the front-end. **The UI is baked in** at `handler/src/wiki.html` and
  pulled into the wasm via `include_str!` (`handler/src/lib.rs`).
- `acceptor/` — the TCP **acceptor** actor (stateful). Listens, and per connection
  `supervisor.spawn`s a fresh handler and `tcp.transfer-async`es the connection to it.
- `frontend/wiki-view.snapshot.html` — the front-end as a standalone, self-contained page
  (the source of truth for the look; a full-document copy is what gets baked into the handler).
- `flake.nix` / `Cargo.*` — build. `docs/` — hosting plan + the upstream skeleton README.

Forked from [`colinrozzi/hello-web`](https://github.com/colinrozzi/hello-web) (the canonical
two-actor Theater HTTP template), theater rev `00b0bf9` / packr 0.24 / in-module-state.

## The front-end (v1 read view)

A single self-contained HTML file — no CDN, no external deps — with a hand-rolled markdown
renderer and a classic-wiki presentation: serif headings with rules, blue links, red links
for pages that don't exist yet, an auto-generated Contents box, a page filter, and a
per-page version hash. Light/dark aware. It's a hash-routed SPA, so the handler serves the
one document for any path.

**v1 content is a baked snapshot** of the live wiki pages. To refresh it, rebuild the
standalone HTML in `frontend/` and re-bake it into `handler/src/wiki.html`, then rebuild.

## Build & run locally

```sh
nix build 'path:.'                              # -> result/*.wasm (acceptor + handler)
nix develop 'path:.' -c \
  theater spawn acceptor/manifest.local.toml    # run from the repo root
curl localhost:9444                             # -> the fleet wiki
```

Notes:
- Use `path:.` (not plain `nix build` / `nix develop`) so untracked working-tree files are
  included.
- Baking non-`.rs` assets? Whitelist the suffix in `flake.nix`'s `cleanSourceWith` filter
  (that's why `.html` is listed) — otherwise `include_str!` won't find it in the sandbox.
- Spawn from the **repo root**: the acceptor's `package` resolves relative to its manifest
  dir (`../result/…`), but the handler manifest it spawns resolves relative to the theater
  process CWD (the repo root, `handler/…` / `result/…`).

## Backend (v1, live)

store-dev's `store-wiki` — a store-publishd sibling. HTTPS API at
`https://mail.colinrozzi.com:19443` (Bearer token + pinned cert):
`GET`/`PUT /wiki/<page>` (`If-Match` optimistic concurrency), `/wiki/<page>/history`,
`/wiki/<page>@<hash>`.

## Roadmap

- **v1 (here):** baked read view, served as an actor. Deploy (public TLS front door,
  `wiki.colinrozzi.com`) is a manager-owned hop.
- **v2:** the handler fetches live page content from the backend per request (outbound TLS
  via `tcp.connect` + `upgrade-to-tls-client`), then edit / history / diff views, and
  eventually per-page sequence-CRDT live editing (with mesh-dev + store-dev).

See `docs/hosting-plan.md` for the full plan.
