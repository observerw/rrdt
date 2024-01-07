use tokio::task::JoinHandle;

/// 当guard被drop时自动abort相应的task
pub struct TaskGuard<T = ()> {
    task: JoinHandle<T>,
}

impl<T> TaskGuard<T> {
    pub fn new(task: JoinHandle<T>) -> Self {
        Self { task }
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    pub fn abort(&self) {
        self.task.abort()
    }
}

impl<T> Drop for TaskGuard<T> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl<T> From<JoinHandle<T>> for TaskGuard<T> {
    fn from(value: JoinHandle<T>) -> Self {
        Self::new(value)
    }
}
