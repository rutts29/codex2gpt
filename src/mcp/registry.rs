use serde_json::{Value, json};

use crate::config::{AppConfig, ToolSurface};
use crate::widgets;

use super::handlers;
use super::input_schemas::*;
use super::output_schemas::*;
use super::{AppState, error};

type ToolHandler = fn(&AppState, Value, Value) -> Value;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolAudience {
    Full,
    Advisor,
}

struct ToolDefinition {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input_schema: fn() -> Value,
    output_schema: fn() -> Value,
    read_only: bool,
    destructive: bool,
    open_world: bool,
    audience: ToolAudience,
    listed: bool,
    widget_uri: Option<&'static str>,
    handler: ToolHandler,
}

impl ToolDefinition {
    fn allowed_on(&self, surface: ToolSurface) -> bool {
        surface == ToolSurface::Full || self.audience == ToolAudience::Advisor
    }

    fn descriptor(&self) -> Value {
        let mut descriptor = json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": (self.input_schema)(),
            "outputSchema": (self.output_schema)(),
            "annotations": {
                "readOnlyHint": self.read_only,
                "destructiveHint": self.destructive,
                "openWorldHint": self.open_world
            }
        });
        if let Some(uri) = self.widget_uri {
            descriptor["_meta"] = json!({
                "ui": {
                    "resourceUri": uri,
                    "visibility": ["model", "app"]
                },
                "openai/outputTemplate": uri,
                "openai/toolInvocation/invoking": "Preparing Codex view...",
                "openai/toolInvocation/invoked": "Codex view ready."
            });
        }
        attach_security_metadata(&mut descriptor);
        descriptor
    }
}

pub(crate) fn descriptors(config: &AppConfig) -> Vec<Value> {
    TOOLS
        .iter()
        .filter(|tool| tool.listed && tool.allowed_on(config.tool_surface))
        .map(ToolDefinition::descriptor)
        .collect()
}

pub(crate) fn call(state: &AppState, id: Value, name: &str, params: Value) -> Value {
    let Some(tool) = TOOLS.iter().find(|tool| tool.name == name) else {
        return error(id, -32602, "Unknown tool");
    };
    if !tool.allowed_on(state.config.tool_surface) {
        return error(id, -32602, "Tool is not available on this tool surface");
    }
    (tool.handler)(state, id, params)
}

const TOOLS: &[ToolDefinition] = &[
    ToolDefinition {
        name: "list_workspaces",
        title: "List Workspaces",
        description: "Use this when you need to see the local workspaces approved for Codex delegation.",
        input_schema: empty_input_schema,
        output_schema: list_workspaces_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Advisor,
        listed: true,
        widget_uri: None,
        handler: handlers::list_workspaces,
    },
    ToolDefinition {
        name: "list_worktrees",
        title: "List Worktrees",
        description: "Use this when you need to inspect Git worktrees for an approved local workspace without changing them.",
        input_schema: workspace_input_schema,
        output_schema: list_worktrees_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: Some(widgets::WORKTREES_WIDGET_URI),
        handler: handlers::list_worktrees_tool,
    },
    ToolDefinition {
        name: "create_worktree",
        title: "Create Worktree",
        description: "Use this when you need to create a managed local Git worktree in an approved writable workspace.",
        input_schema: create_worktree_input_schema,
        output_schema: create_worktree_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::create_worktree_tool,
    },
    ToolDefinition {
        name: "remove_worktree",
        title: "Remove Worktree",
        description: "Use this when you need to remove a managed local Git worktree from an approved writable workspace.",
        input_schema: remove_worktree_input_schema,
        output_schema: remove_worktree_output_schema,
        read_only: false,
        destructive: true,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::remove_worktree_tool,
    },
    ToolDefinition {
        name: "repo_brief",
        title: "Repo Brief",
        description: "Use this when you need a bounded summary of an approved local workspace.",
        input_schema: workspace_input_schema,
        output_schema: repo_brief_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::repo_brief_tool,
    },
    ToolDefinition {
        name: "read_context",
        title: "Read Context",
        description: "Use this when you need bounded text from a specific file in an approved workspace.",
        input_schema: read_context_input_schema,
        output_schema: read_context_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::read_context_tool,
    },
    ToolDefinition {
        name: "search_context",
        title: "Search Context",
        description: "Use this when you need bounded literal text search across an approved local workspace before reading specific files.",
        input_schema: search_input_schema,
        output_schema: search_context_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::search_context_tool,
    },
    ToolDefinition {
        name: "search",
        title: "Search",
        description: "Use this when you need standard read-only search results in one approved local workspace.",
        input_schema: search_input_schema,
        output_schema: search_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Advisor,
        listed: true,
        widget_uri: None,
        handler: handlers::search,
    },
    ToolDefinition {
        name: "fetch",
        title: "Fetch",
        description: "Use this after search when you need the bounded text for one returned local workspace result.",
        input_schema: fetch_input_schema,
        output_schema: fetch_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Advisor,
        listed: true,
        widget_uri: None,
        handler: handlers::fetch,
    },
    ToolDefinition {
        name: "check_connection",
        title: "Check Connection",
        description: "Use this to verify read-only Local Bridge Advisor access to one approved workspace and return one bounded OAuth-related result.",
        input_schema: workspace_input_schema,
        output_schema: readonly_smoke_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Advisor,
        listed: true,
        widget_uri: None,
        handler: handlers::run_readonly_smoke_test,
    },
    ToolDefinition {
        name: "run_readonly_smoke_test",
        title: "Run Read-Only Smoke Test",
        description: "Hidden compatibility alias for checking read-only local bridge access.",
        input_schema: workspace_input_schema,
        output_schema: readonly_smoke_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: false,
        widget_uri: None,
        handler: handlers::run_readonly_smoke_test,
    },
    ToolDefinition {
        name: "list_codex_threads",
        title: "List Codex Threads",
        description: "Show Codex app-server threads for one approved workspace.",
        input_schema: workspace_input_schema,
        output_schema: list_codex_threads_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: Some(widgets::THREADS_WIDGET_URI),
        handler: handlers::list_codex_threads,
    },
    ToolDefinition {
        name: "start_codex_thread",
        title: "Start Codex Thread",
        description: "Start a persistent Codex app-server thread in an approved workspace.",
        input_schema: thread_start_input_schema,
        output_schema: start_thread_output_schema,
        read_only: false,
        destructive: true,
        open_world: true,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::start_codex_thread,
    },
    ToolDefinition {
        name: "start_readonly_codex_thread",
        title: "Start Read-Only Codex Thread",
        description: "Start a read-only Codex app-server thread in an approved workspace.",
        input_schema: thread_start_input_schema,
        output_schema: start_thread_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::start_readonly_codex_thread,
    },
    ToolDefinition {
        name: "resume_codex_thread",
        title: "Resume Codex Thread",
        description: "Resume an existing Codex app-server thread in an approved workspace.",
        input_schema: workspace_thread_target_input_schema,
        output_schema: thread_output_schema,
        read_only: false,
        destructive: true,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::resume_codex_thread,
    },
    ToolDefinition {
        name: "fork_codex_thread",
        title: "Fork Codex Thread",
        description: "Fork an existing Codex app-server thread in an approved workspace.",
        input_schema: workspace_thread_target_input_schema,
        output_schema: thread_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::fork_codex_thread,
    },
    ToolDefinition {
        name: "read_codex_thread",
        title: "Read Codex Thread",
        description: "Read Codex app-server thread history.",
        input_schema: workspace_thread_input_schema,
        output_schema: thread_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::read_codex_thread,
    },
    ToolDefinition {
        name: "send_codex_turn",
        title: "Send Codex Turn",
        description: "Send a follow-up turn to a Codex app-server thread.",
        input_schema: turn_start_input_schema,
        output_schema: send_turn_output_schema,
        read_only: false,
        destructive: true,
        open_world: true,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::send_codex_turn,
    },
    ToolDefinition {
        name: "steer_codex_turn",
        title: "Steer Codex Turn",
        description: "Append guidance to an active in-flight Codex turn.",
        input_schema: turn_steer_input_schema,
        output_schema: steer_turn_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::steer_codex_turn,
    },
    ToolDefinition {
        name: "interrupt_codex_turn",
        title: "Interrupt Codex Turn",
        description: "Interrupt a running Codex app-server turn.",
        input_schema: turn_interrupt_input_schema,
        output_schema: interrupt_turn_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::interrupt_codex_turn,
    },
    ToolDefinition {
        name: "stream_codex_events",
        title: "Stream Codex Events",
        description: "Return persisted app-server events for a Codex thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: stream_events_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::stream_codex_events,
    },
    ToolDefinition {
        name: "review_codex_thread",
        title: "Review Codex Thread",
        description: "Start Codex app-server review mode for a thread and workspace.",
        input_schema: workspace_thread_input_schema,
        output_schema: review_thread_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::review_codex_thread,
    },
    ToolDefinition {
        name: "list_models",
        title: "List Models",
        description: "List Codex app-server models.",
        input_schema: codex_empty_input_schema,
        output_schema: list_models_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::list_models,
    },
    ToolDefinition {
        name: "set_run_options",
        title: "Set Run Options",
        description: "Persist safe allowlisted run options for future Codex delegation.",
        input_schema: set_run_options_input_schema,
        output_schema: set_run_options_output_schema,
        read_only: false,
        destructive: false,
        open_world: true,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::set_run_options,
    },
    ToolDefinition {
        name: "run_in_worktree",
        title: "Run In Worktree",
        description: "Create a managed Git worktree and start a Codex app-server thread inside it.",
        input_schema: run_in_worktree_input_schema,
        output_schema: start_thread_output_schema,
        read_only: false,
        destructive: true,
        open_world: true,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::run_in_worktree,
    },
    ToolDefinition {
        name: "list_hooks_skills_mcp",
        title: "List Hooks Skills MCP",
        description: "Show Codex config, MCP, skills, plugins, and hooks loaded for an approved workspace.",
        input_schema: workspace_input_schema,
        output_schema: list_hooks_skills_mcp_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::list_hooks_skills_mcp,
    },
    ToolDefinition {
        name: "approval_bridge",
        title: "Approval Bridge",
        description: "List pending app-server approvals or send an explicit deny decision.",
        input_schema: approval_input_schema,
        output_schema: approval_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: Some(widgets::APPROVALS_WIDGET_URI),
        handler: handlers::approval_bridge,
    },
    ToolDefinition {
        name: "export_result_bundle",
        title: "Export Result Bundle",
        description: "Export thread history and stored events for a Codex thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: export_result_bundle_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: Some(widgets::RESULT_BUNDLE_WIDGET_URI),
        handler: handlers::export_result_bundle,
    },
    ToolDefinition {
        name: "list_background_terminals",
        title: "List Background Terminals",
        description: "List running app-server background terminals for a loaded Codex thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: list_background_terminals_output_schema,
        read_only: true,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: list_background_terminals,
    },
    ToolDefinition {
        name: "clean_background_terminals",
        title: "Clean Background Terminals",
        description: "Stop all running app-server background terminals for a Codex thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: clean_background_terminals_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: clean_background_terminals,
    },
    ToolDefinition {
        name: "terminate_background_terminal",
        title: "Terminate Background Terminal",
        description: "Terminate one app-server background terminal by process id.",
        input_schema: terminate_terminal_input_schema,
        output_schema: terminate_background_terminal_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::terminate_background_terminal,
    },
    ToolDefinition {
        name: "rollback_thread",
        title: "Rollback Thread",
        description: "Roll back recent in-memory turns for a Codex thread.",
        input_schema: rollback_input_schema,
        output_schema: rollback_thread_output_schema,
        read_only: false,
        destructive: true,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: handlers::rollback_thread,
    },
    ToolDefinition {
        name: "compact_thread",
        title: "Compact Thread",
        description: "Ask Codex app-server to compact a thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: lifecycle_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: compact_thread,
    },
    ToolDefinition {
        name: "archive_thread",
        title: "Archive Thread",
        description: "Ask Codex app-server to archive a thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: lifecycle_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: archive_thread,
    },
    ToolDefinition {
        name: "delete_thread",
        title: "Delete Thread",
        description: "Ask Codex app-server to delete a thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: lifecycle_output_schema,
        read_only: false,
        destructive: true,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: delete_thread,
    },
    ToolDefinition {
        name: "unarchive_thread",
        title: "Unarchive Thread",
        description: "Ask Codex app-server to restore an archived thread.",
        input_schema: workspace_thread_input_schema,
        output_schema: rollback_thread_output_schema,
        read_only: false,
        destructive: false,
        open_world: false,
        audience: ToolAudience::Full,
        listed: true,
        widget_uri: None,
        handler: unarchive_thread,
    },
];

fn list_background_terminals(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(
        state,
        id,
        params,
        "thread/backgroundTerminals/list",
        "Background terminals returned.",
    )
}

fn clean_background_terminals(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(
        state,
        id,
        params,
        "thread/backgroundTerminals/clean",
        "Background terminals cleaned.",
    )
}

fn compact_thread(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(
        state,
        id,
        params,
        "thread/compact/start",
        "Thread compaction started.",
    )
}

fn archive_thread(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(state, id, params, "thread/archive", "Thread archived.")
}

fn delete_thread(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(state, id, params, "thread/delete", "Thread deleted.")
}

fn unarchive_thread(state: &AppState, id: Value, params: Value) -> Value {
    handlers::thread_lifecycle_call(state, id, params, "thread/unarchive", "Thread unarchived.")
}

fn attach_security_metadata(tool: &mut Value) {
    let schemes = json!([{"type": "oauth2", "scopes": []}]);
    tool["securitySchemes"] = schemes.clone();
    if !tool.get("_meta").is_some_and(Value::is_object) {
        tool["_meta"] = json!({});
    }
    tool["_meta"]["securitySchemes"] = schemes;
}
