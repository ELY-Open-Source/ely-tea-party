use anyhow::{Context, bail, ensure};

use super::{
    protocol::{self, Command, Request},
    skills, transport,
};

const HELP: &str = "Ely session messaging (uses the running app in this terminal)
  ely-tea-party msg guide | whoami
  ely-tea-party msg list [--agent TEXT] [--folder PATH] [--query TEXT]
  ely-tea-party msg send ADDRESS TEXT [--key UUID]
  ely-tea-party msg inbox [--all]
  ely-tea-party msg reply MESSAGE_ID TEXT [--key UUID]
  ely-tea-party msg ack MESSAGE_ID
  ely-tea-party skills install|uninstall claude|codex
  ely-tea-party skills status|sync
Messages are stored in mailboxes; recipients read them with inbox. No terminal input is injected.";

pub fn command() -> Option<i32> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!(
                "ely-tea-party {} (messaging protocol {})",
                protocol::APP_VERSION,
                protocol::VERSION
            );
            Some(0)
        }
        Some("msg" | "skills") => {
            if args.len() == 1 || args[1..] == ["--help"] {
                println!("{HELP}");
                return Some(0);
            }
            let result = if args[0] == "skills" {
                skills::cli(&args[1..])
            } else {
                run(&args[1..])
            };
            Some(match result {
                Ok(value) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).expect("JSON value serializes")
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error:#}");
                    1
                }
            })
        }
        _ => None,
    }
}

fn run(args: &[String]) -> anyhow::Result<serde_json::Value> {
    let command = parse(args)?;
    let environment = |name| {
        std::env::var(name).ok().filter(|value| !value.is_empty())
        .with_context(|| format!("messages: {name} is unavailable; run this command inside an Ely session (not its drawer)"))
    };
    let endpoint = environment("ELY_MSG_ENDPOINT")?;
    let request = Request {
        protocol: protocol::VERSION,
        session: environment("ELY_SESSION")?.parse()?,
        token: environment("ELY_MSG_TOKEN")?,
        command,
    };
    if let Command::Send { key, .. } | Command::Reply { key, .. } = &request.command {
        eprintln!(
            "messages: request key {key}; if delivery is unconfirmed, retry this same command with --key {key}"
        );
    }
    let response = transport::call(&endpoint, &request)?;
    if let Some(error) = response.error {
        bail!("{error}");
    }
    ensure!(
        response.protocol == protocol::VERSION,
        "messages: incompatible app protocol; restart Ely and read msg guide again"
    );
    response
        .result
        .context("messages: app returned neither a result nor an error")
}

pub(super) fn parse(args: &[String]) -> anyhow::Result<Command> {
    let one = |name: &str| args.len() == 1 && args[0] == name;
    if one("guide") {
        return Ok(Command::Guide);
    }
    if one("whoami") {
        return Ok(Command::Whoami);
    }
    if one("inbox") || args == ["inbox", "--all"] {
        return Ok(Command::Inbox {
            all: args.len() == 2,
        });
    }
    if let [command, message] = args
        && command == "ack"
    {
        return Ok(Command::Ack {
            message: message.clone(),
        });
    }
    if args.first().is_some_and(|arg| arg == "list") {
        let (mut agent, mut folder, mut query) = (None, None, None);
        ensure!(
            args.len() % 2 == 1,
            "messages: each list filter needs a value"
        );
        for pair in args[1..].chunks_exact(2) {
            let slot = match pair[0].as_str() {
                "--agent" => &mut agent,
                "--folder" => &mut folder,
                "--query" => &mut query,
                other => bail!("messages: unknown list filter {other}"),
            };
            ensure!(slot.is_none(), "messages: duplicate filter {}", pair[0]);
            *slot = Some(pair[1].clone());
        }
        return Ok(Command::List {
            agent,
            folder,
            query,
        });
    }
    if let [command, target, body, rest @ ..] = args
        && matches!(command.as_str(), "send" | "reply")
    {
        let key = match rest {
            [] => uuid::Uuid::new_v4().to_string(),
            [flag, key] if flag == "--key" => {
                uuid::Uuid::parse_str(key).context("messages: --key must be a UUID")?;
                key.clone()
            }
            _ => bail!("messages: expected --key UUID after the message"),
        };
        ensure!(
            !body.trim().is_empty() && body.len() <= 32 * 1024,
            "messages: body must be 1–32768 bytes"
        );
        return Ok(if command == "send" {
            Command::Send {
                to: target.clone(),
                body: body.clone(),
                key,
            }
        } else {
            Command::Reply {
                message: target.clone(),
                body: body.clone(),
                key,
            }
        });
    }
    bail!(
        "messages: unknown command or arguments; run ely-tea-party msg --help, or msg guide inside Ely"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_words(words: &[&str]) -> anyhow::Result<Command> {
        parse(&words.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
    #[test]
    fn parsing_rejects_stale_options_before_contacting_app() {
        assert!(parse_words(&["send", "target"]).is_err());
        assert!(parse_words(&["send", "target", "hello", "--now"]).is_err());
        assert!(parse_words(&["list", "--folder"]).is_err());
        assert!(parse_words(&["list", "--agent", "codex", "--agent", "claude"]).is_err());
        assert!(parse_words(&["guide", "--old"]).is_err());
        assert!(matches!(
            parse_words(&["list", "--folder", "/a b", "--query", "登录"]).unwrap(),
            Command::List { .. }
        ));
        assert!(matches!(
            parse_words(&["inbox", "--all"]).unwrap(),
            Command::Inbox { all: true }
        ));
    }
}
