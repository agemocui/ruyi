use futures::Future;

use slab;

pub type Task = Box<Future<Item = (), Error = ()>>;

pub trait IntoTask {
    fn into_task(self) -> Task;
}

impl<F: Future<Error = ()> + 'static> IntoTask for F {
    #[inline]
    fn into_task(self) -> Task {
        Box::new(self.map(drop))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskId {
    inner: usize,
}

impl From<usize> for TaskId {
    #[inline]
    fn from(index: usize) -> Self {
        TaskId { inner: index }
    }
}

impl From<TaskId> for usize {
    #[inline]
    fn from(task_id: TaskId) -> Self {
        task_id.inner
    }
}

impl TaskId {
    #[inline]
    pub fn is_valid(self) -> bool {
        self.inner != slab::invalid_index()
    }

    #[inline]
    pub fn invalid() -> Self {
        slab::invalid_index()
    }
}
