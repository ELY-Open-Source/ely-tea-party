//! `elyteaparty://skin/<id>` from the skin market: the pack is fetched and checked, then offered on the Characters page.

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{App, BorrowAppContext, Global};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{Slot, files};

pub const SCHEME: &str = "elyteaparty";
const SITE: &str = "https://elyteaparty.com";
const MOST_PACK_BYTES: u64 = 44 * 1024 * 1024;

/// The pack a market link offers, as far as it got.
#[derive(Clone, Debug, PartialEq)]
pub enum Offer {
    Fetching(String),
    Ready(Ready),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ready {
    pub id: String,
    pub name: String,
    pub author: String,
    /// The character it was made for: the mascot's pack fits the mascot alone.
    pub target: String,
    pub path: PathBuf,
}

impl Ready {
    pub fn fits(&self, slot: Slot) -> bool {
        (self.target == Slot::Whale.id()) == (slot == Slot::Whale)
    }

    /// The slot to show first: the pack's own character when there is one.
    pub fn slot(&self, current: Slot) -> Slot {
        Slot::ALL
            .into_iter()
            .find(|slot| slot.id() == self.target)
            .unwrap_or(if self.fits(current) {
                current
            } else {
                Slot::Claude
            })
    }
}

#[derive(Default)]
pub struct Market {
    pub offer: Option<Offer>,
}

impl Global for Market {}

/// The skin id in `url`, or why it is no market link.
pub fn skin_id(url: &str) -> anyhow::Result<String> {
    let rest = url
        .strip_prefix(&format!("{SCHEME}://skin/"))
        .with_context(|| format!("market: {url} is not a skin link"))?;
    let id = rest.trim_end_matches('/');
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    anyhow::ensure!(
        (3..=64).contains(&id.len())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
            && edge(id.chars().next())
            && edge(id.chars().last()),
        "market: {url} names no valid skin"
    );
    Ok(id.to_string())
}

/// Takes a market link: fetches its pack in the background and offers it on the Characters page.
pub fn open(url: &str, cx: &mut App) {
    log::info!("market: opening {url}");
    let id = match skin_id(url) {
        Ok(id) => id,
        Err(error) => {
            log::error!("{error:#}");
            set(Some(Offer::Failed(format!("{error:#}"))), cx);
            crate::settings::window::open_characters(None, cx);
            return;
        }
    };
    set(Some(Offer::Fetching(id.clone())), cx);
    crate::settings::window::open_characters(None, cx);
    let fetching = id.clone();
    let task = cx
        .background_executor()
        .spawn(async move { fetch(&fetching) });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            // A newer link replaced this one meanwhile.
            if cx.global::<Market>().offer != Some(Offer::Fetching(id.clone())) {
                return;
            }
            match result {
                Ok(ready) => {
                    log::info!("market: {} is ready to install", ready.id);
                    let slot = ready.slot(Slot::Claude);
                    set(Some(Offer::Ready(ready)), cx);
                    crate::settings::window::open_characters(Some(slot), cx);
                }
                Err(error) => {
                    log::error!("{error:#}");
                    set(Some(Offer::Failed(format!("{error:#}"))), cx);
                }
            }
        });
    })
    .detach();
}

pub fn set(offer: Option<Offer>, cx: &mut App) {
    cx.update_default_global::<Market, _>(|market, _| market.offer = offer);
    cx.refresh_windows();
}

#[derive(Deserialize)]
struct Answer {
    skin: Listing,
}

#[derive(Deserialize)]
struct Listing {
    id: String,
    name: String,
    author: String,
    target: String,
    sha256: String,
    size: u64,
}

fn site() -> String {
    std::env::var("ELY_MARKET_URL").unwrap_or_else(|_| SITE.into())
}

/// The pack's listing and file, its bytes matching the listing's hash and its contents a pack this build reads.
fn fetch(id: &str) -> anyhow::Result<Ready> {
    let site = site();
    let body = crate::curl::get("market", &format!("{site}/api/skins/{id}"), "")?;
    let Answer { skin } =
        serde_json::from_slice(&body).context("market: the listing does not parse")?;
    anyhow::ensure!(
        skin.id == id,
        "market: asked for {id}, the market answered {}",
        skin.id
    );
    anyhow::ensure!(
        skin.size <= MOST_PACK_BYTES,
        "market: {id} is larger than 44 MiB"
    );
    let path = std::env::temp_dir().join(format!("ely-tea-party-{id}.elyskin"));
    crate::curl::download("market", &format!("{site}/api/skins/{id}/pack"), &path)?;
    let bytes =
        std::fs::read(&path).with_context(|| format!("market: cannot read {}", path.display()))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    anyhow::ensure!(
        hash == skin.sha256.to_ascii_lowercase(),
        "market: {id}'s file does not match its listing"
    );
    let slot = if skin.target == Slot::Whale.id() {
        Slot::Whale
    } else {
        Slot::Claude
    };
    files::read_pack(&path, slot)?;
    log::info!("market: fetched {id} ({} bytes)", bytes.len());
    Ok(Ready {
        id: skin.id,
        name: skin.name,
        author: skin.author,
        target: skin.target,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_name_one_skin() {
        assert_eq!(
            skin_id("elyteaparty://skin/tester.blue-otter").unwrap(),
            "tester.blue-otter"
        );
        assert_eq!(
            skin_id("elyteaparty://skin/tester.blue-otter/").unwrap(),
            "tester.blue-otter"
        );
        for bad in [
            "elyteaparty://skin/",
            "elyteaparty://skin/../etc",
            "elyteaparty://skin/a/b",
            "elyteaparty://skin/Tester.x",
            "https://elyteaparty.com/skins/tester.x",
            "elyteaparty://theme/tester.x",
        ] {
            assert!(skin_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_pack_goes_to_its_own_character_or_a_fitting_one() {
        let ready = |target: &str| Ready {
            id: "a.b".into(),
            name: "B".into(),
            author: "A".into(),
            target: target.into(),
            path: PathBuf::new(),
        };
        assert_eq!(ready("gpt").slot(Slot::Claude), Slot::Gpt);
        assert_eq!(ready("someday").slot(Slot::Kimi), Slot::Kimi);
        assert_eq!(ready("someday").slot(Slot::Whale), Slot::Claude);
        assert_eq!(ready("whale").slot(Slot::Gpt), Slot::Whale);
        assert!(ready("gpt").fits(Slot::Pi));
        assert!(!ready("gpt").fits(Slot::Whale));
        assert!(!ready("whale").fits(Slot::Pi));
    }
}
