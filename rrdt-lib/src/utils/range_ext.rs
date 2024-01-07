pub trait RangeExt
where
    Self: Sized,
{
    fn len(&self) -> usize;

    fn split_at(&mut self, offset: u64) -> (Self, Self);

    fn split_off(&mut self, offset: u64) -> Self;

    fn split_to(&mut self, offset: u64) -> Self;
}

impl RangeExt for std::ops::Range<u64> {
    fn len(&self) -> usize {
        assert!(self.start <= self.end);

        (self.end - self.start) as usize
    }

    fn split_at(&mut self, offset: u64) -> (Self, Self) {
        assert!(offset >= self.start);
        assert!(offset <= self.end);

        let range = self.start..offset;
        self.start = offset;

        (range, self.clone())
    }

    fn split_off(&mut self, offset: u64) -> Self {
        let end = std::cmp::min(self.start + offset, self.end);
        let range = self.start..end;

        self.end = end;

        range
    }

    fn split_to(&mut self, offset: u64) -> Self {
        let end = std::cmp::min(self.start + offset, self.end);
        let range = self.start..end;

        self.start = end;

        range
    }
}
