// ely-tea-party: written by `ely-tea-party hooks install`; Ely Tea Party's characters follow OpenCode through it.
// Outside Ely Tea Party it does nothing.
import { spawn } from "node:child_process";

// Subagents run in child sessions; their idling ends no turn.
const children = new Set();

function send(payload) {
  const hook = process.env.ELY_HOOK;
  if (!hook) return;
  try {
    const child = spawn(hook, ["hook", "opencode"], {
      stdio: ["pipe", "ignore", "ignore"],
      detached: true,
    });
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
    child.unref();
  } catch {}
}

function stop(sessionID, event) {
  if (!children.has(sessionID)) send({ hook_event_name: event, session_id: sessionID });
}

export const ElyTeaParty = async () => ({
  "chat.message": async (input) =>
    send({ hook_event_name: "UserPromptSubmit", session_id: input?.sessionID }),
  "tool.execute.before": async (input, output) =>
    send({
      hook_event_name: "PreToolUse",
      session_id: input?.sessionID,
      tool_name: input?.tool,
      tool_input: output?.args ?? {},
    }),
  "tool.execute.after": async (input) =>
    send({ hook_event_name: "PostToolUse", session_id: input?.sessionID, tool_name: input?.tool }),
  event: async ({ event }) => {
    const p = event?.properties ?? {};
    switch (event?.type) {
      case "session.created":
        if (p.info?.parentID) children.add(p.info.id);
        break;
      case "session.idle":
        stop(p.sessionID, "Stop");
        break;
      case "session.status":
        if (p.status?.type === "idle") stop(p.sessionID, "Stop");
        break;
      case "session.error":
        stop(p.sessionID, "StopFailure");
        break;
      case "permission.asked":
      case "permission.updated": {
        const what = p.permission ?? p.type ?? "a tool";
        send({
          hook_event_name: "PermissionRequest",
          tool_name: what,
          message: `OpenCode asks to use ${what}`,
        });
        break;
      }
      case "permission.replied":
        send({ hook_event_name: "PostToolUse" });
        break;
    }
  },
});
