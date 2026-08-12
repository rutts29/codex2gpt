# Local Demo Guide

This guide demonstrates the local bridge without hosting it. It does not start
a Cloudflare Tunnel or connect a ChatGPT app.

## Fixture-backed evidence

Run the offline test suite:

```sh
cargo test --offline
```

These tests exercise JSON-RPC routing, OAuth and PKCE handling, workspace
policy, worktrees, and app-server message handling. They use fake Codex
app-server, Git, and ripgrep executables, so a passing result proves the bridge
logic but not compatibility with an installed Codex release.

## Live local smoke

With Codex installed and an approved workspace configured, start the bridge on
loopback only:

```sh
CODEX2GPT_BEARER_TOKEN="$(openssl rand -hex 32)" \
CODEX2GPT_OAUTH_APPROVAL_TOKEN="$(openssl rand -hex 32)" \
CODEX2GPT_BASE_URL="http://127.0.0.1:8787" \
  cargo run -- serve --config codex2gpt.example.json
```

In another terminal, use the bearer token from that command to request
`/healthz` and `tools/list` as shown in [live-test.md](live-test.md). This
validates the running local server and the installed Codex app-server boundary.

## Hosted ChatGPT integration

Connecting ChatGPT requires a temporary HTTPS tunnel and an OAuth flow. That
is a separate live integration step documented in [live-test.md](live-test.md)
and is not covered by the fixture-backed tests.
