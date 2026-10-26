use std::ops::Range;

use crate::sessions::LinkId;

pub struct Reply {
    pub text: String,
    pub links: Vec<(Range<usize>, LinkId)>,
}

pub fn render(text: &str, open: &[LinkId], streaming: bool) -> Reply {
    let mut reply = Reply {
        text: String::new(),
        links: Vec::new(),
    };
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(label) = rest.strip_prefix('[') {
            let (label, end) = label_end(label);
            if let Some(end) = end {
                let after = &rest[end + 2..];
                if let Some(destination) = after.strip_prefix('(') {
                    let closing = destination.find(')');
                    let start = reply.text.len();
                    reply.text.push_str(&label);
                    if let Some(closing) = closing {
                        if let Some(id) = LinkId::parse_url(&destination[..closing])
                            .filter(|id| open.contains(id))
                            && start < reply.text.len()
                        {
                            reply.links.push((start..reply.text.len(), id));
                        }
                        rest = &destination[closing + 1..];
                        continue;
                    }
                    break;
                }
                if streaming && after.is_empty() {
                    reply.text.push_str(&label);
                    break;
                }
                reply.text.push_str(&rest[..end + 2]);
                rest = after;
                continue;
            } else {
                reply.text.push_str(if streaming { &label } else { rest });
                break;
            }
        }
        let character = rest.chars().next().expect("nonempty text");
        reply.text.push(character);
        rest = &rest[character.len_utf8()..];
    }
    reply
}

fn label_end(text: &str) -> (String, Option<usize>) {
    let mut label = String::new();
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        if escaped {
            label.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ']' {
            return (label, Some(index));
        } else {
            label.push(character);
        }
    }
    (label, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_labels_and_renamed_sessions_route_by_address() {
        let a = LinkId::default();
        let b = LinkId::default();
        let input = format!(
            "主人，[同名]({})卡住了，[同名]({})做完啦。",
            a.url(),
            b.url()
        );
        let reply = render(&input, &[b, a], false);
        assert_eq!(reply.text, "主人，同名卡住了，同名做完啦。");
        assert_eq!(
            reply.links.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
            [a, b]
        );
        for (range, _) in reply.links {
            assert_eq!(&reply.text[range], "同名");
        }
    }

    #[test]
    fn closed_unknown_and_external_links_are_plain_labels() {
        let closed = LinkId::default();
        let input = format!(
            "[closed]({}) [web](https://example.com) [bad](ely://session/1)",
            closed.url()
        );
        let reply = render(&input, &[LinkId::default()], false);
        assert_eq!(reply.text, "closed web bad");
        assert!(reply.links.is_empty());
        assert!(render("同名完成啦", &[closed], false).links.is_empty());
    }

    #[test]
    fn partial_stream_does_not_expose_link_syntax_or_destination() {
        let id = LinkId::default();
        let input = format!("主人，[鲸鱼]({})", id.url());
        for end in input
            .char_indices()
            .map(|(index, _)| index)
            .filter(|end| *end > "主人，[".len())
        {
            let reply = render(&input[..end], &[id], true);
            assert!(!reply.text.contains("ely://"));
            assert!(!reply.text.contains('['));
            assert!(reply.links.is_empty());
        }
        assert_eq!(render(&input, &[id], false).links.len(), 1);
    }

    #[test]
    fn escaped_title_characters_and_ordinary_brackets_are_preserved() {
        let id = LinkId::default();
        let input = format!(r"[notes] [Agent \[A\] \\ test]({})", id.url());
        let reply = render(&input, &[id], false);
        assert_eq!(reply.text, r"[notes] Agent [A] \ test");
        assert_eq!(&reply.text[reply.links[0].0.clone()], r"Agent [A] \ test");
    }
}
