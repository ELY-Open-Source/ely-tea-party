#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anchor {
    pub left: f32,
    pub bottom: bool,
    pub height: f32,
}

pub struct Placement {
    pub left: f32,
    pub width: f32,
    pub tail: f32,
    pub offset: f32,
    pub content_height: f32,
}

impl Anchor {
    pub fn place(self, width: f32, height: f32, rem: f32) -> Placement {
        let gap = rem * 0.5;
        let bubble_width = (rem * 25.).min((width - 2. * gap).max(0.));
        let left = self.left.clamp(gap, (width - bubble_width - gap).max(gap));
        let top_room = rem * 3.;
        let padding = rem * 2.5 + 2.;
        let offset = if self.bottom {
            self.height + rem * 0.75
        } else {
            top_room
        };
        let offset = offset.min((height - top_room - padding - rem).max(gap));
        let other = if self.bottom { top_room } else { gap };
        Placement {
            left,
            width: bubble_width,
            tail: (self.left + 44. - left).clamp(rem, (bubble_width - rem * 3.).max(rem)),
            offset,
            content_height: (height - offset - other - padding).max(0.),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_bubble_leaves_room_for_the_whale_and_grows_upward() {
        let anchor = Anchor {
            left: 140.,
            bottom: true,
            height: 123.4,
        };
        let placed = anchor.place(1200., 760., 16.);
        assert!(placed.offset > anchor.height);
        assert!(placed.content_height + 42. + placed.offset <= 760. - 48.);
        assert_eq!(placed.left, 140.);
    }

    #[test]
    fn both_edges_fit_narrow_windows_and_keep_the_tail_inside() {
        for bottom in [false, true] {
            for left in [-20., 140., 1200.] {
                let placed = Anchor {
                    left,
                    bottom,
                    height: 123.4,
                }
                .place(300., 300., 16.);
                assert!(placed.left >= 8.);
                assert!(placed.left + placed.width <= 292.);
                assert!(placed.tail >= 16. && placed.tail <= placed.width - 48.);
                assert!(placed.offset + placed.content_height + 42. <= 300.);
            }
        }
    }
}
