//! Which agent a session's foreground job runs, read from the job leader's command line.

pub(crate) mod process;

use std::path::Path;

use super::Kind;

/// Programs that run an agent's script named by their next argument.
const RUNNERS: [&str; 6] = ["node", "bun", "deno", "python", "python3", "tsx"];
/// Shells, whose holding the keys means no agent runs; macOS starts the login shell under `login`.
const SHELLS: [&str; 12] = [
    "sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "nu", "pwsh", "elvish", "xonsh",
];
/// Package runners whose next argument names a package.
const FETCHERS: [&str; 6] = ["npx", "bunx", "pnpx", "uvx", "pnpm", "yarn"];

/// What holds a session's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foreground {
    /// The session's own shell, so no agent runs.
    Shell,
    Agent(Kind),
    /// Some other job, or one this platform cannot read.
    Other,
}

/// The foreground of the session whose shell is `shell`, the terminal handing keys to `group`.
pub fn foreground(shell: u32, group: Option<u32>) -> Foreground {
    match group {
        Some(group) if group == shell => Foreground::Shell,
        Some(group) => match command(group) {
            Some(argv) if is_shell(&argv) => Foreground::Shell,
            Some(argv) => classify(&argv).map_or(Foreground::Other, Foreground::Agent),
            None => Foreground::Other,
        },
        None => Foreground::Other,
    }
}

/// The command line starts a shell, `-zsh` as a login shell too.
fn is_shell(argv: &[String]) -> bool {
    let program = argv.first().map(|program| stem(program));
    program.is_some_and(|program| SHELLS.contains(&program.trim_start_matches('-')))
}

/// The agent a command line runs, if any.
pub fn classify(argv: &[String]) -> Option<Kind> {
    let mut args = argv.iter().map(String::as_str);
    let program = stem(args.next()?);
    let program = program.trim_start_matches('-');
    if let Some(kind) = Kind::named(program) {
        return Some(kind);
    }
    let mut operands = args.filter(|arg| !arg.starts_with('-'));
    if RUNNERS.contains(&program) {
        let script = operands.next()?;
        if let Some(kind) = Kind::named(&stem(script)) {
            return Some(kind);
        }
        if FETCHERS.contains(&stem(script).as_str()) {
            return operands.next().and_then(package);
        }
        return installed(script).or_else(|| standalone(script));
    }
    if FETCHERS.contains(&program) {
        return operands
            .find(|arg| *arg != "exec" && *arg != "dlx")
            .and_then(package);
    }
    None
}

/// A file's name without folder or extension, lowercased.
fn stem(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .map_or(path.into(), |name| name.to_string_lossy().into_owned());
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    let name = [".js", ".mjs", ".cjs", ".ts", ".py"]
        .iter()
        .find_map(|ext| name.strip_suffix(ext))
        .unwrap_or(name);
    name.to_ascii_lowercase()
}

/// An npm package spec, `@scope/name@version`, as the agent its name is.
fn package(spec: &str) -> Option<Kind> {
    let name = spec.rsplit('/').next()?;
    let name = name.split('@').find(|part| !part.is_empty())?;
    Kind::named(name)
}

/// A script inside an installed package: `…/node_modules/@scope/name/…`.
fn installed(script: &str) -> Option<Kind> {
    let mut parts = script
        .split(['/', '\\'])
        .skip_while(|part| *part != "node_modules");
    parts.next()?;
    let first = parts.next()?;
    let name = if first.starts_with('@') {
        parts.next()?
    } else {
        first
    };
    Kind::named(name)
}

/// dscode's self-contained install, whose launcher runs `<root>/scripts/standalone.mjs` on its own Node.
fn standalone(script: &str) -> Option<Kind> {
    let root = Path::new(script).parent()?.parent()?;
    (stem(script) == "standalone" && root.join(".dscode-standalone.json").is_file())
        .then_some(Kind::Deepseek)
}

fn command(pid: u32) -> Option<Vec<String>> {
    process::read(pid).map(|process| process.argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(line: &str) -> Option<Kind> {
        let argv: Vec<String> = line.split(' ').map(Into::into).collect();
        classify(&argv)
    }

    #[test]
    fn a_program_names_its_agent() {
        assert_eq!(kind("claude"), Some(Kind::Claude));
        assert_eq!(
            kind("/Users/a/.local/bin/claude --resume"),
            Some(Kind::Claude)
        );
        assert_eq!(kind("codex"), Some(Kind::Gpt));
        assert_eq!(kind("/opt/homebrew/bin/opencode"), Some(Kind::OpenCode));
        assert_eq!(kind("omp"), Some(Kind::Pi));
        assert_eq!(kind("-zsh"), None);
        assert_eq!(
            kind("vim pi.md"),
            None,
            "an agent's name as an argument is no agent"
        );
    }

    #[test]
    fn a_runner_names_its_script() {
        assert_eq!(kind("node /opt/homebrew/bin/codex"), Some(Kind::Gpt));
        assert_eq!(
            kind("node --no-warnings /usr/local/bin/gemini"),
            Some(Kind::Gemini)
        );
        assert_eq!(
            kind("node /usr/lib/node_modules/@anthropic-ai/claude-code/cli.js"),
            Some(Kind::Claude)
        );
        assert_eq!(
            kind("node /home/a/pi/server.js"),
            None,
            "a folder is no package"
        );
        assert_eq!(kind("node server.js"), None);
    }

    #[test]
    fn a_fetcher_names_its_package() {
        assert_eq!(kind("npx @openai/codex"), Some(Kind::Gpt));
        assert_eq!(kind("npx -y @google/gemini-cli@latest"), Some(Kind::Gemini));
        assert_eq!(
            kind("node /opt/homebrew/bin/npx opencode-ai"),
            Some(Kind::OpenCode)
        );
        assert_eq!(
            kind("pnpm dlx @anthropic-ai/claude-code"),
            Some(Kind::Claude)
        );
        assert_eq!(kind("npx prettier"), None);
    }

    #[test]
    fn dscode_standalone_runs_its_launcher_on_its_own_node() {
        let root = std::env::temp_dir().join(format!("ely-dscode-{}", std::process::id()));
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        let line = |root: &Path| {
            format!(
                "{0}/runtime/bin/node {0}/scripts/standalone.mjs --resume",
                root.display()
            )
        };
        assert_eq!(kind(&line(&root)), None, "no marker, no dscode");
        std::fs::write(root.join(".dscode-standalone.json"), "{}").unwrap();
        assert_eq!(kind(&line(&root)), Some(Kind::Deepseek));
        assert_eq!(
            kind(&format!("node {}/scripts/server.mjs", root.display())),
            None,
            "only its launcher"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_shell_holding_its_keys_runs_no_agent() {
        assert_eq!(foreground(42, Some(42)), Foreground::Shell);
        assert_eq!(foreground(42, None), Foreground::Other);
        let shell = |line: &str| is_shell(&line.split(' ').map(Into::into).collect::<Vec<_>>());
        assert!(shell("-zsh"));
        assert!(shell("/opt/homebrew/bin/fish -l"));
        assert!(!shell("node /usr/local/bin/codex"));
    }

    #[cfg(unix)]
    #[test]
    fn a_running_process_reads_its_own_command() {
        let argv = command(std::process::id()).expect("this test's own command line");
        assert!(!argv.is_empty());
    }
}
