use crate::TaskId;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    TaskAdded(TaskId),
    TaskMaturated(TaskId),
    TaskClaimed(TaskId),
    TaskAcked(TaskId),
}

impl Event {
    pub fn get_task_id(&self) -> &TaskId {
        match self {
            Event::TaskAcked(id) => id,
            Event::TaskAdded(id) => id,
            Event::TaskClaimed(id) => id,
            Event::TaskMaturated(id) => id,
        }
    }
}
