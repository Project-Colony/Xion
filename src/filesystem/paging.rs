#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub offset: usize,
    pub limit: usize,
}

impl PageRequest {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self { offset, limit }
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn apply<T>(self, entries: Vec<T>) -> Page<T> {
        let total = entries.len();
        let offset = self.offset.min(total);
        let end = offset.saturating_add(self.limit).min(total);
        let items = entries
            .into_iter()
            .skip(offset)
            .take(end - offset)
            .collect();
        Page {
            items,
            total,
            offset,
            limit: self.limit,
        }
    }

    pub fn slice<'a, T>(self, entries: &'a [T]) -> &'a [T] {
        let total = entries.len();
        let offset = self.offset.min(total);
        let end = offset.saturating_add(self.limit).min(total);
        &entries[offset..end]
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

impl<T> Page<T> {
    pub fn has_more(&self) -> bool {
        self.offset + self.items.len() < self.total
    }

    pub fn next_offset(&self) -> usize {
        (self.offset + self.items.len()).min(self.total)
    }
}
