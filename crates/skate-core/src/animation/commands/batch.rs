use super::{
    buffers::{BufferError, PoseBuffers},
    set_data::{self, SetDataCommand},
};
use std::num::NonZeroU16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobId {
    owner: usize,
    index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    CommandCapacityExceeded,
    DependencyCapacityExceeded,
    UnavailableDependency,
}

struct Job {
    bone_count: u16,
    commands: Vec<SetDataCommand>,
    dependencies: Vec<JobId>,
}
impl Job {
    fn new() -> Self {
        Self {
            bone_count: 0,
            commands: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

pub struct SetDataQueue {
    owner: usize,
    buffers: PoseBuffers,
    max_work_units: NonZeroU16,
    prepared: Vec<Job>,
    current: Job,
    command_count: usize,
}
impl SetDataQueue {
    pub fn new(buffers: PoseBuffers, max_work_units: NonZeroU16) -> Self {
        Self {
            owner: super::buffers::new_owner(),
            buffers,
            max_work_units,
            prepared: Vec::new(),
            current: Job::new(),
            command_count: 0,
        }
    }

    pub fn enqueue(
        &mut self,
        bone_count: u16,
        command: SetDataCommand,
        dependencies: &[Option<JobId>],
    ) -> Result<JobId, QueueError> {
        if self.command_count == usize::from(u16::MAX) {
            return Err(QueueError::CommandCapacityExceeded);
        }
        let full = self.current.commands.len() == usize::from(self.max_work_units.get());
        let id = JobId {
            owner: self.owner,
            index: self.prepared.len() + usize::from(full),
        };
        let mut collected = if full {
            Vec::new()
        } else {
            self.current.dependencies.clone()
        };
        for dependency in dependencies.iter().flatten().copied() {
            if dependency == id {
                continue;
            }
            if dependency.owner != self.owner || dependency.index >= id.index {
                return Err(QueueError::UnavailableDependency);
            }
            if !collected.contains(&dependency) {
                if collected.len() == usize::from(u8::MAX) {
                    return Err(QueueError::DependencyCapacityExceeded);
                }
                collected.push(dependency);
            }
        }
        if full {
            self.flush();
        }
        self.current.bone_count = bone_count;
        self.current.commands.push(command);
        self.current.dependencies = collected;
        self.command_count += 1;
        Ok(id)
    }

    pub fn flush(&mut self) -> bool {
        if self.current.commands.is_empty() {
            return false;
        }
        self.prepared
            .push(std::mem::replace(&mut self.current, Job::new()));
        true
    }

    pub fn submit(mut self) -> PendingSetData {
        self.flush();
        PendingSetData {
            owner: self.owner,
            buffers: self.buffers,
            jobs: self.prepared,
        }
    }

    pub fn cancel(self) -> PoseBuffers {
        self.buffers
    }
}

pub struct PendingSetData {
    owner: usize,
    buffers: PoseBuffers,
    jobs: Vec<Job>,
}
impl PendingSetData {
    pub fn complete(mut self) -> Result<CompletedSetData, ExecutionFailure> {
        for (index, job) in self.jobs.iter().enumerate() {
            if let Err(cause) = set_data::execute(&mut self.buffers, job.bone_count, &job.commands)
            {
                return Err(ExecutionFailure {
                    buffers: self.buffers,
                    job: JobId {
                        owner: self.owner,
                        index,
                    },
                    cause,
                });
            }
        }
        Ok(CompletedSetData {
            buffers: self.buffers,
        })
    }
}

pub struct ExecutionFailure {
    pub buffers: PoseBuffers,
    pub job: JobId,
    pub cause: BufferError,
}

pub struct CompletedSetData {
    buffers: PoseBuffers,
}
impl CompletedSetData {
    pub fn buffers(&self) -> &PoseBuffers {
        &self.buffers
    }
    pub fn into_buffers(self) -> PoseBuffers {
        self.buffers
    }
}
