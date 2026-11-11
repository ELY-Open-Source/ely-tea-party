use super::*;

struct Setup {
    router: Router,
    directory: std::path::PathBuf,
    tokens: HashMap<u64, String>,
}
impl Setup {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("ely-router-{}", uuid::Uuid::new_v4()));
        let mut router = Router::new(Mail::open(&directory.join("mail.db")).unwrap());
        router
            .refresh(vec![snapshot(1, "a"), snapshot(2, "b")])
            .unwrap();
        Self {
            router,
            directory,
            tokens: HashMap::from([(1, "token-a".into()), (2, "token-b".into())]),
        }
    }
    fn run(&mut self, session: u64, command: Command) -> anyhow::Result<Value> {
        self.router.execute(
            Request {
                protocol: protocol::VERSION,
                session,
                token: self.tokens[&session].clone(),
                command,
            },
            &self.tokens,
        )
    }
}
impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn snapshot(session: u64, identity: &str) -> Snapshot {
    Snapshot {
        session,
        identity: identity.into(),
        agent: if session == 1 { "Claude Code" } else { "Codex" }.into(),
        folder: "/work/shop".into(),
        title: format!("session {session}"),
        state: "idle".into(),
    }
}
fn send(to: String, body: &str) -> Command {
    Command::Send {
        to,
        body: body.into(),
        key: uuid::Uuid::new_v4().to_string(),
    }
}

#[test]
fn two_sessions_discover_send_read_reply_and_ack() {
    let mut setup = Setup::new();
    let peers = setup
        .run(
            1,
            Command::List {
                agent: Some("codex".into()),
                folder: Some("shop".into()),
                query: Some("session 2".into()),
            },
        )
        .unwrap();
    assert_eq!(peers.as_array().unwrap().len(), 1);
    let target = peers[0]["address"].as_str().unwrap().to_string();
    let first = setup.run(1, send(target, "登录接口已更新")).unwrap();
    let id = first["message"]["id"].as_str().unwrap().to_string();
    assert_eq!(first["state"], "stored");
    let received = setup.run(2, Command::Inbox { all: false }).unwrap();
    assert_eq!(received[0]["body"], "登录接口已更新");
    assert_eq!(received[0]["acknowledged"], false);
    setup
        .run(
            2,
            Command::Reply {
                message: id.clone(),
                body: "已同步".into(),
                key: uuid::Uuid::new_v4().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        setup.run(1, Command::Inbox { all: false }).unwrap()[0]["reply_to"],
        id
    );
    assert!(
        setup
            .run(
                1,
                Command::Ack {
                    message: id.clone()
                }
            )
            .is_err()
    );
    setup.run(2, Command::Ack { message: id }).unwrap();
    assert!(
        setup
            .run(2, Command::Inbox { all: false })
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rename_keeps_address_but_replacement_cannot_take_old_mail() {
    let mut setup = Setup::new();
    let address = setup.router.peers[&2].address.clone();
    setup.run(1, send(address.clone(), "hello")).unwrap();
    let mut renamed = snapshot(2, "b");
    renamed.title = "new name".into();
    renamed.folder = "/work/other".into();
    setup
        .router
        .refresh(vec![snapshot(1, "a"), renamed])
        .unwrap();
    assert_eq!(setup.router.peers[&2].address, address);
    setup
        .router
        .refresh(vec![snapshot(1, "a"), snapshot(2, "new-generation")])
        .unwrap();
    assert_ne!(setup.router.peers[&2].address, address);
    assert!(setup.run(1, send(address, "wrong recipient")).is_err());
    assert!(
        setup
            .run(2, Command::Inbox { all: true })
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_credentials_and_protocol_cannot_mutate_mail() {
    let mut setup = Setup::new();
    let command = send(setup.router.peers[&2].address.clone(), "hello");
    assert!(
        setup
            .router
            .execute(
                Request {
                    protocol: protocol::VERSION,
                    session: 1,
                    token: "wrong".into(),
                    command: command.clone()
                },
                &setup.tokens
            )
            .is_err()
    );
    assert!(
        setup
            .router
            .execute(
                Request {
                    protocol: protocol::VERSION + 1,
                    session: 1,
                    token: "token-a".into(),
                    command
                },
                &setup.tokens
            )
            .is_err()
    );
    assert!(
        setup
            .run(2, Command::Inbox { all: true })
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retry_after_recipient_closes_returns_existing_receipt_without_duplicate() {
    let mut setup = Setup::new();
    let address = setup.router.peers[&2].address.clone();
    let command = send(address.clone(), "hello");
    let first = setup.run(1, command.clone()).unwrap();
    setup.router.refresh(vec![snapshot(1, "a")]).unwrap();
    assert_eq!(setup.run(1, command).unwrap(), first);
    assert_eq!(setup.router.mail.inbox(&address, true).unwrap().len(), 1);
}
