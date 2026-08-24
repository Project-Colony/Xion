#[derive(Debug, Clone, Copy)]
pub struct VirtualList {
    pub item_height: f32,
    pub viewport_height: f32,
    pub overscan: usize,
}

impl VirtualList {
    pub fn visible_range(&self, scroll_offset: f32, total_items: usize) -> VirtualWindow {
        if total_items == 0 || self.item_height <= 0.0 || self.viewport_height <= 0.0 {
            return VirtualWindow::empty();
        }

        // The offset outlives the content. A scrollable keeps its position when
        // a smaller directory loads under it and reports that position back, so
        // `scroll_offset` can describe a list that no longer exists.
        //
        // Unclamped, `start` then runs past `total_items`: no row is drawn,
        // because the range is inverted — but `padding_top` is still
        // `start × item_height`, and the empty space it invents justifies the
        // very offset that produced it. The result latches: a folder holding
        // one file scrolls for thousands of pixels of nothing.
        let content_height = total_items as f32 * self.item_height;
        let max_offset = (content_height - self.viewport_height).max(0.0);
        let scroll_offset = scroll_offset.clamp(0.0, max_offset);

        let start_index = (scroll_offset / self.item_height).floor().max(0.0) as usize;
        let visible_count = (self.viewport_height / self.item_height).ceil() as usize;
        let start = start_index.saturating_sub(self.overscan);
        let end = (start_index + visible_count + self.overscan).min(total_items);
        let padding_top = start as f32 * self.item_height;
        let padding_bottom = (total_items.saturating_sub(end)) as f32 * self.item_height;

        VirtualWindow {
            start,
            end,
            padding_top,
            padding_bottom,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualWindow {
    pub start: usize,
    pub end: usize,
    pub padding_top: f32,
    pub padding_bottom: f32,
}

impl VirtualWindow {
    pub fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            padding_top: 0.0,
            padding_bottom: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

#[cfg(test)]
mod scroll_clamp_tests {
    use super::*;

    fn list() -> VirtualList {
        VirtualList {
            item_height: 28.0,
            viewport_height: 800.0,
            overscan: 4,
        }
    }

    /// Le bug signalé : un dossier d'une seule entrée dans lequel on peut
    /// défiler sur des milliers de pixels de vide.
    ///
    /// Le décalage survit au widget quand le contenu rétrécit sous lui. Sans
    /// bornage, `start` dépasse `end`, donc aucune ligne n'est dessinée — mais
    /// `padding_top` vaut toujours `start × hauteur`, et ce vide justifie à son
    /// tour le décalage. La boucle se verrouille.
    #[test]
    fn a_stale_offset_cannot_invent_empty_space() {
        let window = list().visible_range(5000.0, 1);
        assert_eq!(window.start, 0);
        assert_eq!(window.end, 1, "l'unique entrée doit être dessinée");
        assert_eq!(window.padding_top, 0.0, "pas de vide au-dessus");
        assert_eq!(window.padding_bottom, 0.0);
    }

    #[test]
    fn the_window_is_never_inverted() {
        for total in [0usize, 1, 5, 37, 1000] {
            for offset in [0.0f32, 100.0, 5_000.0, 1e6] {
                let w = list().visible_range(offset, total);
                assert!(w.start <= w.end, "total={total} offset={offset}");
                assert!(w.end <= total);
                assert!(w.padding_top >= 0.0 && w.padding_bottom >= 0.0);
            }
        }
    }

    /// Le total des trois morceaux doit valoir la hauteur du contenu, sinon la
    /// barre de défilement décrit une liste qui n'existe pas.
    #[test]
    fn the_pieces_add_up_to_the_content_height() {
        for total in [1usize, 12, 90, 5000] {
            for offset in [0.0f32, 250.0, 9_000.0] {
                let w = list().visible_range(offset, total);
                let drawn = (w.end - w.start) as f32 * 28.0;
                assert!(
                    (w.padding_top + drawn + w.padding_bottom - total as f32 * 28.0).abs() < 0.5,
                    "total={total} offset={offset}"
                );
            }
        }
    }

    /// Un défilement légitime dans une longue liste ne doit pas être bridé.
    #[test]
    fn a_legitimate_offset_still_scrolls() {
        let w = list().visible_range(2800.0, 1000);
        assert!(w.start > 90 && w.start < 101, "start={}", w.start);
        assert!(w.padding_top > 0.0);
    }
}
