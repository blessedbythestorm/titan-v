use std::{any::{Any, TypeId}, future::Future, pin::Pin, sync::atomic::AtomicBool, task::Poll};

use crate::{tasks::{self, TasksSubsystem}, ArcLock};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, Stream};
use log::{error, trace};
use tokio::{
    sync::{mpsc, oneshot}, time::Instant
};
use tracing::info;

#[async_trait]
pub trait Task<S>: Send + 'static
where
    S: Subsystem,
{
    type Output: Send + 'static;

    async fn execute(self, _subsystem: &S) -> Result<Self::Output>
    where Self: Sized {
        panic!("This should never be called: exec");
    }

    async fn execute_mut(self, _subsystem: &mut S) -> Result<Self::Output>
    where Self: Sized {
        panic!("This should never be called: exec_mut")
    }

    fn name() -> &'static str;

    fn is_mut() -> bool;

    fn log() -> bool {
        true
    }

    fn benchmark() -> bool {
        false
    }

    fn io() -> bool {
        false
    }
}

#[async_trait]
pub trait TaskMessage<S>: Send + 'static
where
    S: Subsystem,
{
    fn gen_id(&self) -> String;

    fn log(&self) -> bool;

    fn name(&self) -> &'static str;

    fn benchmark(&self) -> bool;

    fn io(&self) -> bool;

    fn is_mut(&self) -> bool;

    async fn execute_boxed(self: Box<Self>, subsystem: ArcLock<S>) -> Result<()>;
    
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
    fn gen_id(&self) -> String {
        format!("{}_{}", T::name(), nanoid::nanoid!(16))
    }

    fn log(&self) -> bool {
        T::log()
    }

    fn name(&self) -> &'static str {
        T::name()
    }

    fn benchmark(&self) -> bool {
        T::benchmark()
    }

    fn io(&self) -> bool {
        T::io()
    }

    fn is_mut(&self) -> bool {
        T::is_mut()
    }

    async fn execute_boxed(self: Box<Self>, subsystem: ArcLock<S>) -> Result<()> {

        let task_name = self.name();
        
        if self.is_mut() {
            
            trace!("{}: Pre-WriteLock", &task_name);
            
            let mut subsystem_ref = subsystem.lock().await;

            trace!("{}: Post-WriteLock", &task_name);

            trace!("{}: Pre-ExecuteMut", &task_name);

            let result = self.task.execute_mut(&mut *subsystem_ref)
                .await;

            trace!("{}: Post-ExecuteMut", &task_name);

            trace!("{}: Pre-Response", &task_name);

            self.sender.send(result)
                .map_err(|_| anyhow!("Failed to send mut task result!"))?;

            trace!("{}: Post-Response", &task_name);
            
        } else {
            trace!("{}: Pre-ReadLock", &task_name);
            
            let subsystem_ref = subsystem.read().await;

            trace!("{}: Post-ReadLock", &task_name);

            trace!("{}: Pre-Execute", &task_name);
            
            let result = self.task.execute(&subsystem_ref)
                .await;

            trace!("{}: Post-Execute", &task_name);
            
            trace!("{}: Pre-Respone", &task_name);
            
            self.sender.send(result)
                .map_err(|_| anyhow!("Failed to send task result!"))?;

            trace!("{}: Post-Response", &task_name);
        }
        
        Ok(())
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
            Poll::Ready(Err(err)) => Poll::Ready(Err(anyhow::anyhow!("Task cancelled: {:?}", err))),
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
    fn start_quiet<S>(subsystem: S, mut subsystem_receiver: SubsystemReceiver<S>) -> ArcLock<bool>
    where
        S: Subsystem,
    {
        let should_close = ArcLock::new(false);
        let should_close_ext = should_close.clone();
        
        tokio::spawn(async move {
            let subsystem_inst = ArcLock::new(subsystem);
            let subsystem = subsystem_inst.clone();
                            
            while let Some(task_message) = subsystem_receiver.recv().await {
                let subsystem = subsystem.clone();

                trace!("{}: Received on Subsystem", task_message.name());
                
                tokio::spawn(async move {
                    task_message.execute_boxed(subsystem)
                        .await;
                });

                if *should_close.read().await {
                    break;
                }
            }

            info!("Subsystem stopped!");
        });

        should_close_ext
    }

    fn start<S>(
        subsystem: S,
        mut subsystem_receiver: SubsystemReceiver<S>,
        tasks: SubsystemRef<TasksSubsystem>,
    ) -> ArcLock<bool>
    where
        S: Subsystem,
    {
        let should_close = ArcLock::new(false);
        let should_close_ext = should_close.clone();
        
        tokio::spawn(async move {
            let subsystem = ArcLock::new(subsystem);
            let subsystem = subsystem.clone();
            
            while let Some(task_message) = subsystem_receiver.recv().await {
                let subsystem = subsystem.clone();
                let tasks = tasks.clone();

                trace!("{}: Received on Subsystem", task_message.name());

                match task_message.io() {
                    false => {
                        tokio::spawn(subsystem_run_task(subsystem, tasks, task_message));
                    },
                    true => {
                        tokio::task::spawn_blocking(move || {
                            tokio::runtime::Handle::current()
                                .block_on(subsystem_run_task(subsystem, tasks, task_message));
                        });
                    },
                };

                if *should_close.read().await {
                    break;
                }
            }

            info!("Subsystem stopped!");                
        });

        should_close_ext
    }
}


async fn subsystem_run_task<S>(
    subsystem: ArcLock<S>,
    tasks: SubsystemRef<TasksSubsystem>,
    task_message: Box<dyn TaskMessage<S>>,
) -> Result<()>
where
    S: Subsystem,
{
    let task_id = task_message.gen_id();
    let task_name = task_message.name();
    let task_logs = task_message.log();
    let task_benchmarks = task_message.benchmark();
    
    let time_start = Instant::now();

    if task_logs && !task_benchmarks {
        tasks.send(tasks::StartTask {
            id: task_id.clone(),
            name: task_name,
            depth: 0,
        }).await?;
    }

    if task_benchmarks {
        tasks.send(tasks::StartBenchmark {
            name: task_name,
        }).await;
    }

    task_message.execute_boxed(subsystem)
        .await?;
        
    if task_logs && !task_benchmarks {
        tasks.send(tasks::EndTask {
            id: task_id,
            end: time_start.elapsed().as_secs_f64(),
            display: Box::new(|task: tasks::TaskLog| format!("{:.6}s", task.duration)),
        }).await?;
    }

    if task_benchmarks {
        tasks.send(tasks::EndBenchmark {
            name: task_name,
            end: time_start.elapsed().as_secs_f64() * 1000.0,
            display: Box::new(|bench: tasks::BenchmarkLog| {
                format!(
                    "{:0>4.2} ms ~ [{:0>4.2} ms] <=> [{:0>4.2} ms - {:0>4.2} ms]",
                    bench.duration, bench.average, bench.min, bench.max
                )
            }),
        }).await?;
    }

    Ok(())
}

pub type SubsystemReceiver<S> = mpsc::UnboundedReceiver<Box<dyn TaskMessage<S>>>;
pub type SubsystemSender<S> = mpsc::UnboundedSender<Box<dyn TaskMessage<S>>>;

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
        let (sender, receiver) = mpsc::unbounded_channel();
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

        let task_name = task_message.name();
        let subsystem_name = TypeId::of::<S>();

        let boxed_task_message: Box<dyn TaskMessage<S>> = Box::new(task_message);

        trace!("{}: Sender Pre-Send", &task_name);

        let send_res = self.sender.send(boxed_task_message);

        if let Err(err) = send_res {
            error!(
                "Failed to send task {} to subsystem {:?}: {}",
                task_name,
                subsystem_name,
                err
            );
        }

        trace!("{}: Sender Post-Send", &task_name);

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

pub trait ErasedSubsystemRef: Send + Sync {    
    fn as_any(&self) -> &dyn Any; 
}

impl<S> ErasedSubsystemRef for SubsystemRef<S>
where
    S: Subsystem,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}
