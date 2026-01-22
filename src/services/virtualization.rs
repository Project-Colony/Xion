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
}
