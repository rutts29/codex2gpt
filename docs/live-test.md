# Live Test Runbook

Check the local server first, then connect ChatGPT through a temporary HTTPS
tunnel when ready.

## Local Smoke

Follow [Run locally](../README.md#run-locally) to configure and start the bridge.
Keep using that same terminal so the exported tokens and `bridge_pid` remain
available:

```sh
curl --silent --show-error http://127.0.0.1:8787/healthz
curl --silent --show-error \
  --request POST http://127.0.0.1:8787/mcp \
  --header "Authorization: Bearer $CODEX2GPT_BEARER_TOKEN" \
  --header "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
curl --silent --show-error \
  --request POST http://127.0.0.1:8787/mcp \
  --header "Authorization: Bearer $CODEX2GPT_BEARER_TOKEN" \
  --header "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","id":2,"method":"resources/list"}'
curl --include --silent --show-error \
  --request POST http://127.0.0.1:8787/mcp \
  --header "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","id":3,"method":"tools/list"}'
curl --silent --show-error \
  http://127.0.0.1:8787/.well-known/oauth-protected-resource
curl --silent --show-error \
  http://127.0.0.1:8787/.well-known/oauth-authorization-server
```

Expected result: `healthz` returns `ok`, and `tools/list` includes the Codex
app-server tools. `resources/list` includes the ChatGPT Apps widget templates
for threads, worktrees, approvals, and result bundles. The unauthorized `/mcp`
request returns `401` with a `WWW-Authenticate` header pointing to
`/.well-known/oauth-protected-resource`. The protected resource metadata points
to `<base-url>/mcp`, and the authorization server metadata advertises dynamic
client registration, authorization, token, and PKCE support.

For a local OAuth smoke, register a ChatGPT redirect URI and preserve the
resource value returned by protected resource metadata:

```sh
curl --silent --show-error \
  --request POST http://127.0.0.1:8787/oauth/register \
  --header "Content-Type: application/json" \
  --data '{"redirect_uris":["https://chatgpt.com/connector_platform_oauth_redirect"]}'
```

Then open `/oauth/authorize` with the registered `client_id`, redirect URI,
`response_type=code`, `code_challenge_method=S256`, a PKCE challenge, and
`resource=http://127.0.0.1:8787/mcp`. The page must ask for the local
`CODEX2GPT_OAUTH_APPROVAL_TOKEN` and submit it by form `POST`, not in the
authorization URL query string, before redirecting with an authorization code.
Authorization and token exchange must reject missing or different `resource`
values.

For a Pro-advisor connector, set `"tool_surface": "advisor"` in a copy of the
config and reconnect or refresh the ChatGPT app. `tools/list` should contain
only `list_workspaces`, `search`, `fetch`, and `check_connection`; each
tool should have `readOnlyHint: true`, `destructiveHint: false`, and
`openWorldHint: false`. `resources/list` should be empty in advisor mode.

## Cloudflare Tunnel

Start the tunnel in another terminal and copy the generated
`https://...trycloudflare.com` URL:

```sh
cloudflared tunnel --url http://127.0.0.1:8787 --no-autoupdate
```

In the original bridge terminal, stop the local smoke server and restart it
with fresh tokens and that URL as `CODEX2GPT_BASE_URL`:

```sh
kill "$bridge_pid"
wait "$bridge_pid"
export CODEX2GPT_BEARER_TOKEN="$(openssl rand -hex 32)"
export CODEX2GPT_OAUTH_APPROVAL_TOKEN="$(openssl rand -hex 32)"
export CODEX2GPT_BASE_URL="https://YOUR-TUNNEL.trycloudflare.com"
./target/debug/codex2gpt serve --config .codex2gpt/config.json &
bridge_pid=$!
```

Then connect ChatGPT Developer Mode to:

```text
https://YOUR-TUNNEL.trycloudflare.com/mcp
```

When the authorization page asks for the local approval token, display it in
the original terminal and paste it into that page:

```sh
printf '%s\n' "$CODEX2GPT_OAUTH_APPROVAL_TOKEN"
```

Keep this token private. Stop the bridge with `kill "$bridge_pid"` and stop
the tunnel in its terminal when finished.

## Security Notes

- Do not use a long-lived tunnel for this personal bridge.
- Do not reuse smoke-test Bearer or OAuth approval tokens.
- Keep `allowed_workspaces` narrow.
- Keep write access disabled unless you are testing a specific writable flow.
