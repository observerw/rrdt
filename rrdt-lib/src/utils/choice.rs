use rand::Rng;

pub struct ChoiceIter<T> {
    rng: rand::rngs::ThreadRng,
    values: Vec<T>,
}

impl<T> Iterator for ChoiceIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.values.is_empty() {
            return None;
        }

        let index = self.rng.gen_range(0..self.values.len());
        Some(self.values.swap_remove(index))
    }
}

pub trait Choice<T> {
    fn choice(self) -> ChoiceIter<T>;
}

impl<T> Choice<T> for Vec<T> {
    /// 从数组中不重复的随机选择一个元素
    ///
    /// 当所有元素都被选择过后，会返回None
    fn choice(self) -> ChoiceIter<T> {
        ChoiceIter {
            rng: rand::thread_rng(),
            values: self,
        }
    }
}
