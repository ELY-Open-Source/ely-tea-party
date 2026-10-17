//! `ely-tea-party hooks install [AGENT…]`: each agent on this machine reports to Ely Tea Party; outside it, its hooks do nothing.

mod codex;
mod files;
mod groups;
mod kimi;
mod plugins;

use std::path::{Path, PathBuf};

pub use groups::claude;

/// What each hook and plugin the app installs carries, so a new install finds its own.
pub const MARK: &str = "ely-tea-party";

/// Claude Code's hook command: a no-op in any terminal but Ely Tea Party.
pub fn command(agent: &str) -> String {
    format!(r#"[ -z "$ELY_HOOK" ] || "$ELY_HOOK" hook {agent} || true"#)
}

/// Another agent's hook for `event`: `printenv` and `${h:-}`, as Grok refuses plain `$VAR`; `{}` as its reply.
pub fn command_for(agent: &str, event: &str) -> String {
    format!(
        r#"h=$(printenv ELY_HOOK); if [ -n "${{h:-}}" ]; then "${{h:-}}" hook {agent} {event} 2>/dev/null; else cat >/dev/null; fi; echo '{{}}'; : {MARK}"#
    )
}

/// Where an install wrote, and how many hooks it added; none means it was current.
pub struct Installed {
    pub path: PathBuf,
    pub added: usize,
}

/// An agent the installer knows: its name, its program, its installer, and what its owner should know.
struct Agent(
    &'static str,
    &'static str,
    fn(&Path) -> anyhow::Result<Installed>,
    Option<&'static str>,
);

const AGENTS: [Agent; 10] = [
    Agent("claude", "claude", groups::claude_file, None),
    Agent(
        "codex",
        "codex",
        codex::install,
        Some("Codex trusts hooks by place: install again after another tool changes its hooks."),
    ),
    Agent("gemini", "gemini", groups::gemini, None),
    Agent("agy", "agy", groups::antigravity, None),
    Agent("grok", "grok", groups::grok, None),
    Agent("kimi", "kimi", kimi::install, None),
    Agent(
        "dscode",
        "dscode",
        groups::dscode,
        Some("dscode's hooks see their session only with DSCODE_RUNTIME=in-process."),
    ),
    Agent("opencode", "opencode", plugins::opencode, None),
    Agent("pi", "pi", plugins::pi, None),
    Agent("omp", "omp", plugins::omp, None),
];

/// The names `hooks install` takes.
pub fn names() -> Vec<&'static str> {
    AGENTS.iter().map(|agent| agent.0).collect()
}

/// Installs for each agent in `only`, else for each whose program is on `PATH`; a failure stops none of the rest.
pub fn run(home: &Path, only: &[String]) -> anyhow::Result<()> {
    if let Some(unknown) = only.iter().find(|name| !names().contains(&name.as_str())) {
        anyhow::bail!(
            "install: no agent {unknown:?}; known: {}",
            names().join(", ")
        );
    }
    let mut failed = Vec::new();
    for Agent(name, program, install, note) in &AGENTS {
        let named = only.iter().any(|only| only == name);
        if !only.is_empty() && !named {
            continue;
        }
        if !named && crate::settings::program_path(program).is_err() {
            println!("{name:<9} skipped: {program} is not on PATH");
            continue;
        }
        match install(home) {
            Ok(Installed { path, added: 0 }) => {
                println!("{name:<9} up to date in {}", path.display())
            }
            Ok(Installed { path, .. }) => {
                println!("{name:<9} installed: {}", path.display());
                if let Some(note) = note {
                    println!("{:<9} {note}", "");
                }
            }
            Err(error) => {
                println!("{name:<9} failed: {error:#}");
                failed.push(*name);
            }
        }
    }
    anyhow::ensure!(failed.is_empty(), "install: {} failed", failed.join(", "));
    println!("Restart running agents so they load their hooks.");
    Ok(())
}

/// The folder `variable` names when it is set, else `default`.
fn folder(variable: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map_or(default, PathBuf::from)
}
