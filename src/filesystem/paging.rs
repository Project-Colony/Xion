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

    pub fn slice<T>(self, entries: &[T]) -> &[T] {
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

#[cfg(test)]
mod tests {
    use super::{PageRequest};

    #[test]
    fn page_request_applies_offset_and_limit() {
        let entries = vec![1, 2, 3, 4, 5];
        let page = PageRequest::new(1, 2).apply(entries);

        assert_eq!(page.items, vec![2, 3]);
        assert_eq!(page.total, 5);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, 2);
    }

    #[test]
    fn page_request_slice_bounds() {
        let entries = vec!["a", "b", "c", "d"];
        let slice = PageRequest::new(2, 10).slice(&entries);

        assert_eq!(slice, &["c", "d"]);
    }

    #[test]
    fn page_has_more_and_next_offset() {
        let entries = vec![1, 2, 3, 4];
        let page = PageRequest::new(0, 3).apply(entries);

        assert!(page.has_more());
        assert_eq!(page.next_offset(), 3);
    }
}
