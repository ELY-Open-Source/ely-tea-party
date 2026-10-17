// ely-tea-party: written by `ely-tea-party hooks install`; Ely Tea Party's characters follow __AGENT__ through it.
// Outside Ely Tea Party it does nothing.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname } from "node:path";

function send(payload: Record<string, unknown>) {
  const hook = process.env.ELY_HOOK;
  if (!hook) return;
  try {
    const child = spawn(hook, ["hook", "__AGENT__"], {
      stdio: ["pipe", "ignore", "ignore"],
      detached: true,
    });
    child.on("error", () => {});
    child.stdin?.on("error", () => {});
    child.stdin?.end(JSON.stringify(payload));
    child.unref();
  } catch {}
}

// A subagent's session file sits in a folder named after its parent's.
function nested(ctx: any): boolean {
  const file = ctx?.sessionManager?.getSessionFile?.();
  return typeof file === "string" && existsSync(dirname(file) + ".jsonl");
}

export default function (pi: any) {
  const on = (name: string, payload: (event: any) => Record<string, unknown> | undefined) =>
    pi.on(name, async (event: any, ctx: any) => {
      if (nested(ctx)) return;
      const body = payload(event ?? {});
      const session_id = ctx?.sessionManager?.getSessionId?.();
      if (body) send({ ...body, session_id });
    });
  on("before_agent_start", () => ({ hook_event_name: "UserPromptSubmit" }));
  on("tool_execution_start", (event) => ({
    hook_event_name: "PreToolUse",
    tool_name: event.toolName ?? event.tool_name ?? event.name,
    tool_input: event.args ?? event.input ?? {},
  }));
  on("tool_execution_end", (event) => ({
    hook_event_name: event.isError ? "PostToolUseFailure" : "PostToolUse",
    tool_name: event.toolName ?? event.tool_name ?? event.name,
  }));
  on("agent_end", (event) => (event.willContinue ? undefined : { hook_event_name: "Stop" }));
  on("session_shutdown", () => ({ hook_event_name: "SessionEnd" }));
}
