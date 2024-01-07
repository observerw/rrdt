use std::{
    cmp,
    cmp::Ordering,
    collections::{btree_map, BTreeMap},
    ops::{
        Bound::{Excluded, Included},
        Range,
    },
};

/// 一个u64值的集合，针对长时间运行和随机插入/删除/包含进行了优化
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RangeSet {
    map: BTreeMap<u64, u64>,
}

impl RangeSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn contains(&self, x: u64) -> bool {
        self.pred(x).map_or(false, |(_, end)| end > x)
    }

    pub fn contains_range(&self, x: &Range<u64>) -> bool {
        self.pred(x.start).map_or(false, |(_, end)| end >= x.end)
    }

    pub fn insert_one(&mut self, x: u64) -> bool {
        if let Some((start, end)) = self.pred(x) {
            match end.cmp(&x) {
                // Wholly contained
                Ordering::Greater => {
                    return false;
                }
                Ordering::Equal => {
                    // Extend existing
                    self.map.remove(&start);
                    let mut new_end = x + 1;
                    if let Some((next_start, next_end)) = self.succ(x) {
                        if next_start == new_end {
                            self.map.remove(&next_start);
                            new_end = next_end;
                        }
                    }
                    self.map.insert(start, new_end);
                    return true;
                }
                _ => {}
            }
        }
        let mut new_end = x + 1;
        if let Some((next_start, next_end)) = self.succ(x) {
            if next_start == new_end {
                self.map.remove(&next_start);
                new_end = next_end;
            }
        }
        self.map.insert(x, new_end);
        true
    }

    pub fn insert(&mut self, mut x: Range<u64>) -> bool {
        if x.is_empty() {
            return false;
        }
        if let Some((start, end)) = self.pred(x.start) {
            if end >= x.end {
                return false;
            } else if end >= x.start {
                self.map.remove(&start);
                x.start = start;
            }
        }
        while let Some((next_start, next_end)) = self.succ(x.start) {
            if next_start > x.end {
                break;
            }
            self.map.remove(&next_start);
            x.end = cmp::max(next_end, x.end);
        }
        self.map.insert(x.start, x.end);
        true
    }

    pub fn remove_one(&mut self, x: u64) -> bool {
        self.remove(x..x + 1)
    }

    pub fn remove(&mut self, x: Range<u64>) -> bool {
        if x.is_empty() {
            return false;
        }

        let before = match self.pred(x.start) {
            Some((start, end)) if end > x.start => {
                self.map.remove(&start);
                if start < x.start {
                    self.map.insert(start, x.start);
                }
                if end > x.end {
                    self.map.insert(x.end, end);
                }

                if end >= x.end {
                    return true;
                }
                true
            }
            Some(_) | None => false,
        };
        let mut after = false;
        while let Some((start, end)) = self.succ(x.start) {
            if start >= x.end {
                break;
            }
            after = true;
            self.map.remove(&start);
            if end > x.end {
                self.map.insert(x.end, end);
                break;
            }
        }
        before || after
    }

    pub fn replace(&mut self, mut range: Range<u64>) -> Replace<'_> {
        let pred = if let Some((prev_start, prev_end)) = self
            .pred(range.start)
            .filter(|&(_, end)| end >= range.start)
        {
            self.map.remove(&prev_start);
            let replaced_start = range.start;
            range.start = range.start.min(prev_start);
            let replaced_end = range.end.min(prev_end);
            range.end = range.end.max(prev_end);
            if replaced_start != replaced_end {
                Some(replaced_start..replaced_end)
            } else {
                None
            }
        } else {
            None
        };
        Replace {
            set: self,
            range,
            pred,
        }
    }

    pub fn union(&mut self, other: &Self) {
        for (&start, &end) in &other.map {
            self.insert(start..end);
        }
    }

    pub fn difference(&mut self, other: &Self) {
        for (&start, &end) in &other.map {
            self.remove(start..end);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// range数量
    ///
    /// 两个相邻的range会被视为同一个range
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter(self.map.iter())
    }

    pub fn elements(&self) -> ElementIter<'_> {
        ElementIter {
            inner: self.map.iter(),
            next: 0,
            end: 0,
        }
    }

    pub fn elements_len(&self) -> usize {
        self.map
            .iter()
            .map(|(&start, &end)| (end - start) as usize)
            .sum()
    }

    pub fn min(&self) -> Option<u64> {
        self.iter().next().map(|x| x.start)
    }

    pub fn max(&self) -> Option<u64> {
        self.iter().next_back().map(|x| x.end - 1)
    }

    pub fn first(&self) -> Option<Range<u64>> {
        let (&start, &end) = self.map.iter().next()?;
        Some(start..end)
    }

    pub fn last(&self) -> Option<Range<u64>> {
        let (&start, &end) = self.map.iter().next_back()?;
        Some(start..end)
    }

    pub fn pop_front(&mut self) -> Option<Range<u64>> {
        let result = self.first()?;
        self.map.remove(&result.start);
        Some(result)
    }

    pub fn pop_back(&mut self) -> Option<Range<u64>> {
        let (&start, &end) = self.map.iter().next_back()?;
        self.map.remove(&start);
        Some(start..end)
    }

    fn pred(&self, x: u64) -> Option<(u64, u64)> {
        self.map
            .range((Included(0), Included(x)))
            .next_back()
            .map(|(&x, &y)| (x, y))
    }

    fn succ(&self, x: u64) -> Option<(u64, u64)> {
        self.map
            .range((Excluded(x), Included(u64::max_value())))
            .next()
            .map(|(&x, &y)| (x, y))
    }
}

pub struct Iter<'a>(btree_map::Iter<'a, u64, u64>);

impl<'a> Iterator for Iter<'a> {
    type Item = Range<u64>;
    fn next(&mut self) -> Option<Range<u64>> {
        let (&start, &end) = self.0.next()?;
        Some(start..end)
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Range<u64>> {
        let (&start, &end) = self.0.next_back()?;
        Some(start..end)
    }
}

impl<'a> IntoIterator for &'a RangeSet {
    type Item = Range<u64>;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// 按range中的元素顺序迭代的迭代器
pub struct ElementIter<'a> {
    inner: btree_map::Iter<'a, u64, u64>,
    next: u64,
    end: u64,
}

impl<'a> Iterator for ElementIter<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.next == self.end {
            let (&start, &end) = self.inner.next()?;
            self.next = start;
            self.end = end;
        }
        let x = self.next;
        self.next += 1;
        Some(x)
    }
}

impl<'a> DoubleEndedIterator for ElementIter<'a> {
    fn next_back(&mut self) -> Option<u64> {
        if self.next == self.end {
            let (&start, &end) = self.inner.next_back()?;
            self.next = start;
            self.end = end;
        }
        self.end -= 1;
        Some(self.end)
    }
}

pub struct Replace<'a> {
    set: &'a mut RangeSet,
    pred: Option<Range<u64>>,
    range: Range<u64>,
}

impl Iterator for Replace<'_> {
    type Item = Range<u64>;
    fn next(&mut self) -> Option<Range<u64>> {
        if let Some(pred) = self.pred.take() {
            return Some(pred);
        }

        let (next_start, next_end) = self.set.succ(self.range.start)?;
        if next_start > self.range.end {
            return None;
        }
        self.set.map.remove(&next_start);
        let replaced_end = self.range.end.min(next_end);
        self.range.end = self.range.end.max(next_end);
        if next_start == replaced_end {
            None
        } else {
            Some(next_start..replaced_end)
        }
    }
}

impl Drop for Replace<'_> {
    fn drop(&mut self) {
        for _ in &mut *self {}
        self.set.map.insert(self.range.start, self.range.end);
    }
}
