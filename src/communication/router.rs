use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, ensure};
use serde::Serialize;
use serde_json::{Value, json};

use super::{
    mail::Mail,
    protocol::{self, Command, Request},
};

pub struct Snapshot {
    pub session: u64,
    pub identity: String,
    pub agent: String,
    pub folder: String,
    pub title: String,
    pub state: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Peer {
    pub address: String,
    pub session: u64,
    pub agent: String,
    pub folder: String,
    pub title: String,
    pub state: String,
    pub unread: u64,
    #[serde(skip)]
    identity: String,
}

pub struct Router {
    instance: uuid::Uuid,
    pub peers: BTreeMap<u64, Peer>,
    pub mail: Mail,
}

impl Router {
    pub fn new(mail: Mail) -> Self {
        Self {
            instance: uuid::Uuid::new_v4(),
            peers: BTreeMap::new(),
            mail,
        }
    }

    pub fn refresh(&mut self, snapshots: Vec<Snapshot>) -> anyhow::Result<()> {
        let mut peers = BTreeMap::new();
        for snapshot in snapshots {
            let address = self
                .peers
                .get(&snapshot.session)
                .filter(|peer| peer.identity == snapshot.identity)
                .map(|peer| peer.address.clone())
                .unwrap_or_else(|| {
                    format!(
                        "{}:s{}:{}",
                        self.instance,
                        snapshot.session,
                        uuid::Uuid::new_v4()
                    )
                });
            peers.insert(
                snapshot.session,
                Peer {
                    unread: self.mail.unread(&address)?,
                    address,
                    session: snapshot.session,
                    agent: snapshot.agent,
                    folder: snapshot.folder,
                    title: snapshot.title,
                    state: snapshot.state,
                    identity: snapshot.identity,
                },
            );
        }
        self.peers = peers;
        Ok(())
    }

    pub fn execute(
        &mut self,
        request: Request,
        tokens: &HashMap<u64, String>,
    ) -> anyhow::Result<Value> {
        ensure!(
            tokens
                .get(&request.session)
                .is_some_and(|token| token == &request.token),
            "messages: invalid session credentials"
        );
        ensure!(
            request.protocol == protocol::VERSION,
            "messages: protocol mismatch (app {}, CLI {}); use the CLI bundled with this app and read msg guide again",
            protocol::VERSION,
            request.protocol
        );
        let sender = self
            .peers
            .get(&request.session)
            .context("messages: session is no longer open")?
            .clone();
        match request.command {
            Command::Guide => Ok(protocol::guide()),
            Command::Whoami => Ok(serde_json::to_value(sender)?),
            Command::List {
                agent,
                folder,
                query,
            } => {
                let matches = |needle: &Option<String>, value: &str| {
                    needle
                        .as_ref()
                        .is_none_or(|needle| value.to_lowercase().contains(&needle.to_lowercase()))
                };
                let mut peers: Vec<_> = self
                    .peers
                    .values()
                    .filter(|peer| {
                        matches(&agent, &peer.agent)
                            && matches(&folder, &peer.folder)
                            && matches(
                                &query,
                                &format!("{} {} {}", peer.agent, peer.folder, peer.title),
                            )
                    })
                    .cloned()
                    .collect();
                peers.sort_by_key(|peer| {
                    (
                        peer.folder != sender.folder,
                        peer.session == sender.session,
                        peer.session,
                    )
                });
                Ok(json!(peers))
            }
            Command::Send { to, body, key } => self.send(&sender.address, &to, &body, None, &key),
            Command::Inbox { all } => Ok(json!(self.mail.inbox(&sender.address, all)?)),
            Command::Reply { message, body, key } => {
                let original = self.mail.get(&message)?;
                ensure!(
                    original.to == sender.address,
                    "messages: only the recipient can reply to this message"
                );
                self.send(&sender.address, &original.from, &body, Some(&message), &key)
            }
            Command::Ack { message } => {
                self.mail.ack(&sender.address, &message)?;
                log::info!("messages: {} acknowledged {message}", sender.address);
                Ok(json!({"id":message,"state":"acknowledged"}))
            }
        }
    }

    fn send(
        &self,
        sender: &str,
        target: &str,
        body: &str,
        reply: Option<&str>,
        key: &str,
    ) -> anyhow::Result<Value> {
        let retry = self.mail.retry(sender, key)?;
        ensure!(
            retry.is_some() || self.peers.values().any(|peer| peer.address == target),
            "messages: target is closed, replaced, or not an exact address; run msg list again"
        );
        let message = self.mail.send(sender, target, body, reply, key)?;
        log::info!("messages: stored {} from {sender} to {target}", message.id);
        Ok(json!({"state":"stored","request_key":key,"message":message}))
    }
}

#[cfg(test)]
mod tests;
