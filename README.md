# codex2gpt

`codex2gpt` is a local Rust bridge that lets ChatGPT delegate coding work to
Codex without giving ChatGPT a raw local shell by default.

## Direction

- Run a Rust MCP/ChatGPT App server on the Mac.
- Expose it through Cloudflare Tunnel for HTTPS access from ChatGPT.
- Let ChatGPT inspect bounded context from approved workspaces.
- Delegate local execution to `codex app-server` for persistent threads,
  resume/fork, turns, streamed events, approvals, reviews, models, skills,
  hooks, MCP status, and Codex-managed conversation history.
- Use DevSpace-style worktrees as managed execution targets while keeping
  ChatGPT away from raw local shell access.

## DevSpace Comparison

DevSpace gives ChatGPT direct local coding tools: read, edit, search, shell,
worktrees, skills, and widgets. This project keeps the best primitives:
allowlisted workspaces, OAuth-style approval, audit logs, worktrees, and UI
cards, but changes the power boundary. ChatGPT becomes the planner and reviewer;
Codex remains the local executor.

## Status and Evidence

The local Rust server, MCP routing, OAuth/PKCE flow, workspace policy, managed
worktrees, and Codex app-server transport are implemented. The test suite
exercises those boundaries with local fake app-server, Git, and ripgrep
fixtures; it is not evidence of a live Codex, ChatGPT, or tunnel integration.
See [the local demo guide](docs/local-demo.md) for the exact boundary and a
repeatable local demonstration.

## Dependency Policy

Dependencies are exact-pinned in `Cargo.toml` and locked in `Cargo.lock`.
Before adding or upgrading crates:

1. Check current versions with `cargo search <crate> --limit 1`.
2. Regenerate `Cargo.lock`.
3. Query OSV for every locked crate/version.
4. Run `cargo fetch --locked` only after the advisory check.
5. Run `cargo test`, `cargo check`, and `local-sec scan`.

Socket CLI does not currently score Cargo crates, so OSV and lockfile review are
the Cargo dependency gates for now.

## Local Run

Start the private MCP server with an explicit local Bearer token:

```sh
CODEX2GPT_BEARER_TOKEN="$(openssl rand -base64 32)" \
CODEX2GPT_OAUTH_APPROVAL_TOKEN="$(openssl rand -base64 32)" \
CODEX2GPT_BASE_URL="https://your-tunnel.example" \
  cargo run -- serve --config codex2gpt.example.json
```

Set `CODEX2GPT_BASE_URL` to the HTTPS tunnel origin, such as a Cloudflare
Tunnel URL, before connecting it from ChatGPT. Use `<base-url>/mcp` as the MCP
server URL.

For ChatGPT app submission-style testing, set optional config field
`widget_domain` to the dedicated HTTPS origin that should host the app widgets.
When present, the bridge includes both standard `_meta.ui.domain` and ChatGPT
compatibility `_meta["openai/widgetDomain"]` metadata on widget resources.

`CODEX2GPT_BEARER_TOKEN` is for direct local calls and smoke tests. ChatGPT app
connections use OAuth authorization-code with PKCE, dynamic client registration,
and issued Bearer tokens. Set `CODEX2GPT_OAUTH_APPROVAL_TOKEN` to enable OAuth
discovery and `/oauth/*` routes; the authorize step requires that local approval
token before an authorization code is issued.

Set config field `tool_surface` to `advisor` for a Pro-safe read-only connector.
That mode advertises only `list_workspaces`, `search`, `fetch`, and
`check_connection`; all advertised tools are marked read-only,
non-destructive, and closed-world. It does not advertise ChatGPT widget
resources. Keep `tool_surface` as `full` for the complete Codex delegation
surface.

Current MCP tools:

- `list_workspaces`: list approved local workspace ids and write permissions.
- `list_worktrees`: list sanitized Git worktrees for an approved workspace.
- `create_worktree`: create a managed local Git worktree for an approved
  writable workspace.
- `remove_worktree`: remove a managed local Git worktree from an approved
  writable workspace.
- `search`: standard read-only search in one approved workspace for ChatGPT
  compatibility.
- `fetch`: standard read-only fetch for a document returned by `search`.
- `repo_brief`: return a bounded top-level summary for one workspace.
- `search_context`: run bounded literal search in an approved workspace.
- `read_context`: read bounded text from a specific workspace file.
- `list_codex_threads`: list app-server threads for an approved workspace.
- `start_codex_thread`: create a persistent Codex thread in a workspace.
- `resume_codex_thread`: attach to an existing Codex thread, optionally inside
  a managed worktree.
- `fork_codex_thread`: branch a thread for experiments, optionally inside a
  managed worktree.
- `read_codex_thread`: inspect Codex thread history.
- `send_codex_turn`: send a follow-up turn to a thread.
- `steer_codex_turn`: add guidance to an active in-flight turn.
- `interrupt_codex_turn`: stop a running turn.
- `stream_codex_events`: return persisted app-server events and a normalized
  summary for a thread.
- `review_codex_thread`: start Codex review mode.
- `list_models`: list models exposed by Codex app-server.
- `set_run_options`: persist allowlisted run options. Local path options such
  as `extra_read_dirs` and `images` must stay inside the selected workspace.
- `run_in_worktree`: create a managed worktree and start a thread there.
- `list_hooks_skills_mcp`: show Codex config, feature flags, hooks, skills,
  plugins, and MCP status for a workspace.
- `approval_bridge`: list pending Codex approval requests or send an explicit
  deny decision scoped to a workspace. Allow decisions must stay outside
  MCP/model control.
- `export_result_bundle`: export thread history, final message, changed files,
  branch, diff summary, commands, tests, token usage, status, and stored event
  evidence.
- `list_background_terminals`, `clean_background_terminals`, and
  `terminate_background_terminal`: inspect or stop Codex-managed background
  terminals for a thread.
- `compact_thread`, `rollback_thread`, `archive_thread`, `unarchive_thread`,
  and `delete_thread`: lifecycle controls delegated to app-server.

In full mode, ChatGPT Apps widget resources are also exposed through
`resources/list` and `resources/read`:

- `ui://codex2gpt/threads-v2.html`: workspace-filtered thread list.
- `ui://codex2gpt/worktrees-v2.html`: managed worktree list.
- `ui://codex2gpt/approvals-v2.html`: pending approval requests.
- `ui://codex2gpt/result-bundle-v2.html`: final status, branch, diff summary,
  files, commands, tests, token usage, and event evidence.

New work uses the app-server thread and turn tools because they preserve Codex
conversation history and approval state.
