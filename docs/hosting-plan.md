# Hosting the wiki front-end as a Theater actor

Status: **v1 greenlit (Colin, 2026-09-24). Building.** Deploy hop gated on the manager.

## Decision
Host the browser front-end as a Theater actor, cribbing **website-dev's** pattern
(documented on `/wiki/website`). Confirmed with website-dev 2026-09-24.

## Shape (v1)
- **Fork website's TWO-actor skeleton @ commit `02b34d4`**: `acceptor/` + `handler/`.
  - `acceptor` — listens TCP (`server_tls` in prod), per connection `supervisor.spawn`s a
    fresh handler, `rpc`s `actor.init`, then `tcp.transfer-async` hands off the connection.
  - `handler` — receives the transferred connection, reads one HTTP request, routes, responds,
    shuts down. Stateless.
  - (website also ships `/repo/static-server` = a minimal SINGLE-actor variant; we take the
    two-actor fork because the wiki wants real `/wiki/<page>` routing headroom.)
- **Content = baked-in (static) for v1.** Our front-end is already a single self-contained
  HTML file (`frontend/wiki-view.snapshot.html`) with a hand-rolled markdown renderer and
  **zero external deps** — it drops straight into the handler via `include_str!`, exactly
  like website's blog posts. Rebuild-to-update.
- **Routing (v1):** the front-end is a hash-routed SPA, so the handler can serve the one HTML
  document for `/` (and any path), plus a trivial `200` health route. No server-side per-page
  templating needed yet.
- **Inherit, don't re-derive, the 5 spawn gotchas** from `/wiki/website` (client pacts drop
  state; declare `supervisor-error`/`spawn-failure` exactly; `result<T,E>` decode shape;
  manifest `[[handler]] type` `runtime`→`self`; control caps default-deny → `type="inherit"`).
- **Local gate:** `nix build 'path:.'` then spawn under an **ABI-matched** theater binary
  (`theater spawn`), `curl` to verify — a real spawn is the gate, not just a green build.

## Exposure / deploy (manager-owned hop)
- TLS terminated **in-actor** via `server_tls` in the tcp handler manifest (cert/key),
  like website's prod `:9444`.
- The **manager (`claude@`)** owns the public front door: DNS/subdomain (e.g.
  `wiki.colinrozzi.com`) + the VPS deploy. No fixed port/subdomain convention; set per service.
- Deploy = hand the manager the verified `result/*.wasm` + manifests; rollback via snapshots.

## v2 (later)
- **Live content:** handler does outbound HTTPS to the store-wiki backend
  (`mail.colinrozzi.com:19443`, Bearer + pinned cert) per request and renders current page
  content. Path (buildable today, per website-dev): `tcp.connect` + `tcp.upgrade-to-tls-client`
  + send/receive (NOT the deprecated http-client); creds in the manifest/config.
  website-dev offered to pair on this.
- Then edit / history / diff views, and eventually v2's per-page sequence-CRDT live editing.

## Cross-fleet
- website-dev floated a **shared `theater-http` primitive** (two consumers now: static-server
  + wiki). wiki-dev is a willing design/dogfood partner once the fork settles. Manager to sequence.

## Open / blocked
- **GitHub access for wiki-dev** — needed to clone `colinrozzi/website @ 02b34d4` (private) and
  to resolve the private `theater-guest` git build dep, plus a push remote for this repo.
  Requested from the manager 2026-09-24.
