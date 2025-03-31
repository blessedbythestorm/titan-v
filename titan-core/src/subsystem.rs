use std::{any::Any, future::Future, pin::Pin, task::Poll};
use anyhow::Result;
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, Stream};
use log::{error, trace};
use tokio::{
    sync::{mpsc, oneshot}, time::Instant
};
use tracing::{debug, info};
use crate::{chrono, tasks::{self, TasksSubsystem}, ArcLock, Channels};

pub trait Event: Send + 'static {}

pub trait Task: Clone + Send + 'static {
    type Subsystem: Subsystem;
    type Event: Event;

    type Inputs: Clone + Send + Sync + 'static;
    
    type Output: Send + Sync + 'static;    
        
    fn name() -> &'static str;

    fn log() -> bool {
        true
    }

    fn benchmark() -> bool {
        false
    }

    fn io() -> bool {
        false
    }

    fn inputs(&self) -> Self::Inputs;
 }


pub trait TaskInfo: Send + 'static {
    fn name(&self) -> &'static str;
    fn log(&self) -> bool;
    fn benchmark(&self) -> bool;
    fn io(&self) -> bool;
    fn new_id(&self) -> String;
}

impl<T> TaskInfo for T
where
    T: Task,
{
    fn name(&self) -> &'static str {
        T::name()
    }

    fn log(&self) -> bool {
        T::log()
    }

    fn benchmark(&self) -> bool {
        T::benchmark()
    }

    fn io(&self) -> bool {
        T::io()
    }

    fn new_id(&self) -> String {
        format!("{}_{}", T::name(), nanoid::nanoid!(16))
    }
}


#[async_trait]
pub trait ImmutableTask: Task {
    async fn execute(self, _subsystem: &Self::Subsystem) -> Self::Output;
}

#[async_trait]
pub trait MutableTask: Task {
    async fn execute(self, _subsystem: &mut Self::Subsystem) -> Self::Output;
}

#[async_trait]
pub trait SubsystemMessage<S>: Send + 'static
where
    S: Subsystem,
{
    fn task(&self) -> &dyn TaskInfo;
     
    async fn execute(self: Box<Self>, subsystem: ArcLock<S>) -> Result<()>;
}

struct ImmutableTaskMessage<T>
where
    T: ImmutableTask,
{
    task: T,
    sender: oneshot::Sender<T::Output>,
}

impl<T> ImmutableTaskMessage<T>
where
    T: ImmutableTask
{
    pub fn from(task: T) -> (Box<dyn SubsystemMessage<T::Subsystem>>, oneshot::Receiver<T::Output>) {
        let (sender, receiver) = oneshot::channel();
        
        let message = ImmutableTaskMessage { task, sender };
        
        (Box::new(message), receiver)
    }
}

#[async_trait]
impl<T> SubsystemMessage<T::Subsystem> for ImmutableTaskMessage<T>
where
    T: ImmutableTask,
{
    fn task(&self) -> &dyn TaskInfo {
        &self.task
    }
    
    async fn execute(self: Box<Self>, subsystem: ArcLock<T::Subsystem>) -> Result<()> {

        let task_name = T::name();
        
        trace!("{}: Pre-ReadLock", &task_name);
        
        let subsystem_ref = subsystem.read()
            .await;

        trace!("{}: Post-ReadLock", &task_name);

        // subsystem_ref.channels()
        //     .publish(self.task.clone())
        //     .await;
        
        trace!("{}: Pre-Execute", &task_name);
        
        let task_result = self.task.execute(&subsystem_ref)
            .await;
        
        trace!("{}: Post-Execute", &task_name);
        trace!("{}: Pre-Response", &task_name);

        let send_result = self.sender.send(task_result);

        if let Err(_err) = send_result {
            error!("{}: Failed to send result back to task executor", &task_name);
        }

        trace!("{}: Post-Response", &task_name);
        
        Ok(())
    }    
}


struct MutableTaskMessage<T>
where
    T: MutableTask,
{
    task: T,
    sender: oneshot::Sender<T::Output>,
}

impl<T> MutableTaskMessage<T>
where
    T: MutableTask
{
    pub fn from(task: T) -> (Box<dyn SubsystemMessage<T::Subsystem>>, oneshot::Receiver<T::Output>) {
        let (sender, receiver) = oneshot::channel();
        
        let message = MutableTaskMessage { task, sender };
        
        (Box::new(message), receiver)
    }
}

#[async_trait]
impl<T> SubsystemMessage<T::Subsystem> for MutableTaskMessage<T>
where
    T: MutableTask,
{

    fn task(&self) -> &dyn TaskInfo {
        &self.task
    }
    
    async fn execute(self: Box<Self>, subsystem: ArcLock<T::Subsystem>) -> Result<()> {

        let task_name = T::name();
        
        trace!("{}: Pre-WriteLock", &task_name);
        
        let mut subsystem_ref = subsystem.lock()
            .await;

        trace!("{}: Post-WriteLock", &task_name);

        // subsystem_ref.channels()
        //     .publish_mut(self.task.clone())
        //     .await;
        
        trace!("{}: Pre-Execute", &task_name);
        
        let task_result = self.task.execute(&mut subsystem_ref)
            .await;
        
        trace!("{}: Post-Execute", &task_name);
        trace!("{}: Pre-Response", &task_name);

        let send_result = self.sender.send(task_result);

        if let Err(_err) = send_result {
            error!("{}: Failed to send result back to task executor", &task_name);
        }
        
        trace!("{}: Post-Response", &task_name);    
        
        Ok(())
    }    
}

pub struct TaskHandle<T>{
    receiver: oneshot::Receiver<T>,
}

impl<T> Future for TaskHandle<T>
where
    T: Send + 'static
{
    type Output = Result<T>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.receiver).poll(cx) {
            Poll::Ready(Ok(task_result)) => Poll::Ready(Ok(task_result)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(anyhow::anyhow!("Error retrieving task result: {}", err))),
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
    fn name() -> &'static str;
    
    fn channels(&self) -> Channels;

    fn start_quiet<S>(subsystem: S, mut subsystem_receiver: SubsystemReceiver<S>)
    where
        S: Subsystem,
    {        
        tokio::spawn(async move {
            let subsystem_inst = ArcLock::new(subsystem);
            let subsystem = subsystem_inst.clone();
                            
            while let Some(task_message) = subsystem_receiver.recv().await {
                let subsystem = subsystem.clone();
                let subsystem_name = S::name();
                
                trace!("{} - {}: Received", &subsystem_name, task_message.task().name());
            
                launch_task(subsystem, task_message, None);                
            }

            info!("Subsystem stopped!");
        });
    }

    fn start<S>(
        subsystem: S,
        mut subsystem_receiver: SubsystemReceiver<S>,
        tasks: SubsystemRef<TasksSubsystem>,
    ) 
    where
        S: Subsystem,
    {        
        tokio::spawn(async move {
            let subsystem = ArcLock::new(subsystem);
            let subsystem = subsystem.clone();
            
            while let Some(task_message) = subsystem_receiver.recv().await {
                let subsystem = subsystem.clone();
                let subsystem_name = S::name();
                let tasks = tasks.clone();

                trace!("{} - {}: Received", &subsystem_name, task_message.task().name());

                launch_task(subsystem, task_message, Some(tasks));
            }

            info!("Subsystem stopped!");                
        });
    }
}

fn launch_task<S>(
    subsystem: ArcLock<S>,
    task_message: Box<dyn SubsystemMessage<S>>,    
    tasks: Option<SubsystemRef<TasksSubsystem>>,
)
where
    S: Subsystem
{
    let subsystem_name = S::name();
    let task_name = task_message.task().name();
    
    match task_message.task().io() {
        false => {
            tokio::task::spawn(async move {
                let exec_result = subsystem_run_task(subsystem, task_message, tasks)
                    .await;

                if let Err(err) = exec_result {
                    error!("{} - {}: Execution error: {}",                            
                        subsystem_name,
                        task_name,
                        err
                    );
                }
            });
        },
        true => {
            tokio::task::spawn_blocking(move || {
                tokio::runtime::Handle::current()
                    .block_on(async move {
                        let exec_result = subsystem_run_task(subsystem, task_message, tasks)
                            .await;

                        if let Err(err) = exec_result {
                            error!("{} - {}: Execution error: {}",                            
                                subsystem_name,
                                task_name,
                                err
                            );
                        }
                    });
            });
        },
    };
}

async fn subsystem_run_task<S>(
    subsystem: ArcLock<S>,
    task_message: Box<dyn SubsystemMessage<S>>,    
    tasks: Option<SubsystemRef<TasksSubsystem>>,
) -> Result<()>
where
    S: Subsystem,
{
    let task_id = task_message.task().new_id();
    let task_name = task_message.task().name();
    let task_logs = task_message.task().log();
    let task_benchmarks = task_message.task().benchmark();

    let time_start = Instant::now();

    if let Some(tasks) = tasks.as_ref() {
        if task_logs && !task_benchmarks {
            tasks.send(tasks::StartTask {
                id: task_id.clone(),
                name: task_name,
                depth: 0,
            })
            .await?;
        }

        if task_benchmarks {
            tasks.send(tasks::StartBenchmark {
                name: task_name,
            })
            .await?;
        }
    }

    task_message.execute(subsystem)
        .await?;

    if let Some(tasks) = tasks.as_ref() {
        if task_logs && !task_benchmarks {
            tasks.send(tasks::EndTask {
                id: task_id,
                end: time_start.elapsed().as_secs_f64(),
                display: |task| chrono::format_duration(&task.duration),
            });
        }

        if task_benchmarks {
            tasks.send(tasks::EndBenchmark {
                name: task_name,
                end: time_start.elapsed().as_secs_f64(),
                display: |bench| {
                    format!("{} ~ [{}] <=> [{} - {}]",
                        &chrono::format_duration(&bench.duration),
                        &chrono::format_duration(&bench.average),
                        &chrono::format_duration(&bench.min),
                        &chrono::format_duration(&bench.max)
                    )
                },
            })
            .await?;
        }
    }

    Ok(())
}

pub type SubsystemReceiver<S> = mpsc::UnboundedReceiver<Box<dyn SubsystemMessage<S>>>;
pub type SubsystemSender<S> = mpsc::UnboundedSender<Box<dyn SubsystemMessage<S>>>;

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
        T: ImmutableTask<Subsystem = S>,
    {
        let (task_message, task_receiver) = ImmutableTaskMessage::from(task);
        let task_name = task_message.task().name(); 

        trace!("{}: Sender Pre-Send", &task_name);

        let send_res = self.sender.send(task_message);

        if let Err(err) = send_res {
            let subsystem_name = S::name();
        
            debug!("Failed to send task {} to subsystem {:?}: {}",
                task_name,
                subsystem_name,
                err
            );
        }

        trace!("{}: Sender Post-Send", &task_name);

        TaskHandle { receiver: task_receiver }
    }

    
    pub fn send_mut<T>(&self, task: T) -> TaskHandle<T::Output>
    where
        T: MutableTask<Subsystem = S>,
    {
        let (mut_task_message, mut_task_receiver) = MutableTaskMessage::from(task);
        let mut_task_name = mut_task_message.task().name();
        
        trace!("{}: Sender Pre-Send", &mut_task_name);

        let send_res = self.sender.send(mut_task_message);

        if let Err(err) = send_res {
            let subsystem_name = S::name();
            
            debug!("Failed to send task {} to subsystem {:?}: {}",
                mut_task_name,
                subsystem_name,
                err
            );
        }

        trace!("{}: Sender Post-Send", &mut_task_name);

        TaskHandle { receiver: mut_task_receiver }
    }

    pub fn send_batch<T>(&self, tasks: Vec<T>) -> BatchHandle<T::Output>
    where
        T: ImmutableTask<Subsystem = S>,
    {
        let handles = tasks
            .into_iter()
            .map(|task| self.send(task))
            .collect();

        BatchHandle::new(handles)
    }


    pub fn send_batch_mut<T>(&self, tasks: Vec<T>) -> BatchHandle<T::Output>
    where
        T: MutableTask<Subsystem = S>,
    {
        let handles = tasks
            .into_iter()
            .map(|task| self.send_mut(task))
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
