use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use crate::tasks::{self, TaskLog, TasksSubsystem};
use anyhow::Result;
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, Stream};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

#[async_trait]
pub trait Task<S>: Send + 'static
where
    S: Subsystem,
{
    type Output: Send + 'static;

    async fn execute(self, subsystem: &S) -> Result<Self::Output>;

    fn id(&self) -> String {
        nanoid::nanoid!(16)
    }

    fn name() -> String {
        let parts: Vec<&str> = std::any::type_name::<Self>()
            .split("::")
            .collect();

        parts
            .iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("::")
    }

    fn log() -> bool {
        true
    }

    fn benchmark() -> bool {
        false
    }
}

#[async_trait]
pub trait TaskMessage<S>: Send + 'static
where
    S: Subsystem,
{
    fn id(&self) -> String;

    fn log(&self) -> bool;

    fn name(&self) -> String;

    fn benchmark(&self) -> bool;

    async fn execute(self: Box<Self>, subsystem: &S);
}

struct TaskMessageImpl<S, T>
where
    S: Subsystem,
    T: Task<S>,
{
    task: T,
    sender: oneshot::Sender<Result<T::Output>>,
}

#[async_trait]
impl<S, T> TaskMessage<S> for TaskMessageImpl<S, T>
where
    S: Subsystem,
    T: Task<S>,
{
    fn id(&self) -> String {
        self.task.id()
    }

    fn log(&self) -> bool {
        T::log()
    }

    fn name(&self) -> String {
        T::name()
    }

    fn benchmark(&self) -> bool {
        T::benchmark()
    }

    async fn execute(self: Box<Self>, subsystem: &S) {
        let task_result = self.task.execute(subsystem).await;
        let _ = self.sender.send(task_result);
    }
}

pub struct TaskHandle<T> {
    receiver: oneshot::Receiver<Result<T>>,
}

impl<T> Future for TaskHandle<T> {
    type Output = Result<T>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.receiver).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(anyhow::anyhow!("Task cancelled"))),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct BatchHandle<T> {
    handles: FuturesUnordered<TaskHandle<T>>,
}

impl<T> BatchHandle<T> {
    pub fn new(handles: Vec<TaskHandle<T>>) -> Self {
        Self {
            handles: handles.into_iter().collect(),
        }
    }
}

impl<T> Future for BatchHandle<T>
where
    T: Send + 'static,
{
    type Output = Vec<Result<T>>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let mut results = Vec::new();
        while let Poll::Ready(Some(result)) = Pin::new(&mut this.handles).poll_next(cx) {
            results.push(result);
        }

        if this.handles.is_empty() {
            Poll::Ready(results)
        } else {
            Poll::Pending
        }
    }
}

// Subsystem trait definition
pub trait Subsystem: Sized + Send + Sync + 'static {
    fn start_quiet<S>(subsystem: S, mut subsystem_receiver: SubsystemReceiver<S>)
    where
        S: Subsystem,
    {
        let subsystem_inst = Arc::new(subsystem);
        let subsystem = subsystem_inst.clone();
        tokio::spawn(async move {
            while let Some(task_message) = subsystem_receiver.recv().await {
                // let task_message = receive_result.expect("Failed to get task!");

                let subsystem = subsystem.clone();
                tokio::spawn(async move {
                    task_message.execute(&*subsystem).await;
                });
            }
        });
    }

    fn start<S>(
        subsystem: S,
        mut subsystem_receiver: SubsystemReceiver<S>,
        tasks: SubsystemRef<TasksSubsystem>,
    ) where
        S: Subsystem,
    {
        let subsystem = Arc::new(subsystem);
        let subsystem = subsystem.clone();
        tokio::spawn(async move {
            while let Some(task_message) = subsystem_receiver.recv().await {
                let subsystem = subsystem.clone();
                let tasks = tasks.clone();
                tokio::spawn(async move {
                    let task_id = task_message.id();
                    let task_name = task_message.name();
                    let task_logs = task_message.log();
                    let task_benchmarks = task_message.benchmark();

                    let time_start = Instant::now();

                    if task_logs && !task_benchmarks {
                        tasks.send(tasks::StartTask {
                            id: task_id.clone(),
                            name: task_name.clone(),
                        });
                    }

                    if task_benchmarks {
                        tasks.send(tasks::StartBenchmark {
                            name: task_name.clone(),
                        });
                    }

                    task_message.execute(&*subsystem).await;

                    if task_logs && !task_benchmarks {
                        tasks.send(tasks::EndTask {
                            id: task_id.clone(),
                            end: time_start.elapsed().as_secs_f64(),
                            display: Box::new(|task: tasks::TaskLog| {
                                format!("{:.6}s", task.duration)
                            }),
                        });
                    }

                    if task_benchmarks {
                        tasks.send(tasks::EndBenchmark {
                            name: task_name.clone(),
                            end: time_start.elapsed().as_secs_f64(),
                            display: Box::new(|bench: tasks::BenchmarkLog| {
                                format!(
                                    "{:.6}s ~{:.6}s [{:.6}s - {:.6}s]",
                                    bench.duration, bench.average, bench.min, bench.max
                                )
                            }),
                        });
                    }
                });
            }
        });
    }
}

pub type SubsystemReceiver<S> = mpsc::Receiver<Box<dyn TaskMessage<S>>>;
pub type SubsystemSender<S> = mpsc::Sender<Box<dyn TaskMessage<S>>>;

pub struct SubsystemRef<S>
where
    S: Subsystem,
{
    sender: SubsystemSender<S>,
}

impl<S> Clone for SubsystemRef<S>
where
    S: Subsystem,
{
    fn clone(&self) -> Self {
        SubsystemRef {
            sender: self.sender.clone(),
        }
    }
}
impl<S> SubsystemRef<S>
where
    S: Subsystem,
{
    pub fn new() -> (Self, SubsystemReceiver<S>) {
        let (sender, receiver) = mpsc::channel(100000);
        let subsystem_ref = SubsystemRef { sender };

        (subsystem_ref, receiver)
    }

    pub fn send<T>(&self, task: T) -> TaskHandle<T::Output>
    where
        S: Subsystem,
        T: Task<S>,
    {
        let (sender, receiver) = oneshot::channel();

        let task_message = TaskMessageImpl { task, sender };

        let boxed_task_message: Box<dyn TaskMessage<S>> = Box::new(task_message);

        let send_res = self.sender.try_send(boxed_task_message);

        if send_res.is_err() {
            println!(
                "Failed to send task to subsystem: {}",
                send_res.err().unwrap()
            );
        }

        TaskHandle { receiver }
    }

    pub fn send_batch<T>(&self, tasks: Vec<T>) -> BatchHandle<T::Output>
    where
        S: Send + Sync + 'static,
        T: Task<S> + Send + Sync + 'static,
        T::Output: Send + 'static,
    {
        let handles = tasks
            .into_iter()
            .map(|task| self.send(task))
            .collect();

        BatchHandle::new(handles)
    }
}
