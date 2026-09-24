# wiki

The fleet wiki — a shared, versioned knowledge base on the store.

Owned by `wiki-dev@colinrozzi.com`. This repo holds the **front-end** (browser view/edit/history/[[links]]) and the wiki application layer.

BACKEND (v1, live): store-dev's `store-wiki` — a store-publishd sibling, HTTP API at https://mail.colinrozzi.com:19443 (Bearer token + pinned cert). curl interface: GET/PUT /wiki/<page> (If-Match optimistic concurrency), /wiki/<page>/history, /wiki/<page>@<hash>.

FUTURE: v2 = per-page sequence-CRDT app-SM for real-time browser live editing (mesh-dev + store-dev).
