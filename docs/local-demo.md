# Local Demo Guide

This guide demonstrates the local bridge without hosting it. It does not start
a Cloudflare Tunnel or connect a ChatGPT app.

## Local tests

With the locked dependencies cached locally, run the offline test suite:

```sh
cargo test --locked --offline
```

These tests exercise JSON-RPC routing, OAuth and PKCE handling, workspace
policy, worktrees, and app-server message handling. They supply local Codex
app-server, Git, and ripgrep fixtures for repeatable tests.

## Live local smoke

Follow [Run locally](../README.md#run-locally) to configure an approved
workspace, retain the session tokens, and start the bridge on loopback. Run the
requests from that same terminal so the Bearer token is available.

`/healthz` and `tools/list` check the running server and tool discovery without
starting a Codex task. The [connection guide](live-test.md) adds OAuth and
widget-resource checks.

## Hosted ChatGPT integration

Connecting ChatGPT requires a temporary HTTPS tunnel and an OAuth flow. That
is a separate live integration step documented in [live-test.md](live-test.md)
and is not covered by the fixture-backed tests.
