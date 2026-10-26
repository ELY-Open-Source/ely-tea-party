use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinkId(Uuid);

impl Default for LinkId {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}

impl LinkId {
    pub fn url(self) -> String {
        format!("ely://session/{}", self.0)
    }

    pub fn parse_url(url: &str) -> Option<Self> {
        let id = Self(Uuid::parse_str(url.strip_prefix("ely://session/")?).ok()?);
        (!id.0.is_nil() && id.url() == url).then_some(id)
    }

    pub fn ensure_unique(&mut self, taken: &mut std::collections::HashSet<Self>) {
        if self.0.is_nil() || !taken.insert(*self) {
            log::warn!("session: replacing an empty or duplicate saved link ID");
            loop {
                *self = Self::default();
                if taken.insert(*self) {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_unique_and_survive_serialization() {
        let a = LinkId::default();
        assert_ne!(a, LinkId::default());
        let restored: LinkId = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a.url(), restored.url());
        assert_eq!(LinkId::parse_url(&a.url()), Some(a));
    }

    #[test]
    fn only_exact_internal_urls_are_accepted() {
        let url = LinkId::default().url();
        for bad in [
            format!("{url}/"),
            format!("{url}?x=1"),
            format!("{url}#tab"),
            url.replace("ely:", "https:"),
            url.to_uppercase(),
            "ely://session/00000000-0000-0000-0000-000000000000".into(),
        ] {
            assert_eq!(LinkId::parse_url(&bad), None);
        }
    }

    #[test]
    fn duplicate_restored_id_gets_a_new_address() {
        let original = LinkId::default();
        let mut duplicate = original;
        duplicate.ensure_unique(&mut std::collections::HashSet::from([original]));
        assert_ne!(duplicate, original);
    }
}
