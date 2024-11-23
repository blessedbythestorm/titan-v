use crate::subsystem::{Subsystem, Task};
use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Display {
    pub name: String,
    pub display: String,
}

#[derive(Clone)]
pub struct TaskLog {
    pub id: String,
    pub name: String,
    pub complete: bool,
    pub start: f64,
    pub duration: f64,
    pub display: String,
}

#[derive(Clone)]
pub struct BenchmarkLog {
    pub name: String,
    pub start: f64,
    pub duration: f64,
    pub max: f64,
    pub min: f64,
    pub average: f64,
    pub runs: u64,
    pub run_time: f64,
    pub display: String,
}

pub struct TasksSubsystem {
    // pub channels: Channels,
    pub tasks: Arc<Mutex<IndexMap<String, TaskLog>>>,
    pub benchmarks: Arc<Mutex<IndexMap<String, BenchmarkLog>>>,
}

impl Subsystem for TasksSubsystem {}

impl TasksSubsystem {
    async fn start_task(&self, task: TaskLog) -> Result<()> {
        self.tasks
            .lock()
            .await
            .entry(task.id.clone())
            .or_insert(task);

        Ok(())
    }

    async fn end_task<F>(&self, id: String, time: f64, display_fn: Box<F>) -> Result<()>
    where
        F: FnOnce(TaskLog) -> String + Send + 'static,
    {
        self.tasks
            .lock()
            .await
            .entry(id)
            .and_modify(|task| {
                task.complete = true;
                task.duration = time;
                task.display = display_fn(task.clone());
            });

        Ok(())
    }

    async fn get_task_displays(&self) -> Vec<Display> {
        let task_lock = self.tasks.lock().await;

        task_lock
            .values()
            .map(|task| Display {
                name: task.name.clone(),
                display: task.display.clone(),
            })
            .collect()
    }

    async fn start_benchmark(&self, bench: BenchmarkLog) -> Result<()> {
        self.benchmarks
            .lock()
            .await
            .entry(bench.name.clone())
            .or_insert(bench);

        Ok(())
    }

    async fn end_benchmark<F>(&self, name: String, time: f64, display_fn: Box<F>) -> Result<()>
    where
        F: FnOnce(BenchmarkLog) -> String + Send + 'static,
    {
        self.benchmarks
            .lock()
            .await
            .entry(name.clone())
            .and_modify(|task| {
                task.duration = time;
                task.start = time;
                task.run_time += task.duration;
                task.runs += 1;
                task.average = task.run_time / task.runs as f64;
                task.max = f64::max(task.duration, task.max);
                task.min = f64::min(task.duration, task.min);
                task.display = display_fn(task.clone())
            });

        Ok(())
    }

    async fn get_benchmark_displays(&self) -> Vec<Display> {
        let bench_lock = self.benchmarks.lock().await;

        bench_lock
            .values()
            .map(|bench| Display {
                name: bench.name.clone(),
                display: bench.display.clone(),
            })
            .collect()
    }
}

pub struct StartTask {
    pub id: String,
    pub name: String,
}

#[async_trait]
impl Task<TasksSubsystem> for StartTask {
    type Output = ();

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        let task = TaskLog {
            id: self.id,
            name: self.name,
            complete: false,
            start: 0.0,
            duration: 0.0,
            display: "Exec...".to_string(),
        };

        tasks.start_task(task).await?;

        Ok(())
    }
}

pub struct EndTask<F>
where
    F: FnOnce(TaskLog) -> String + Send + 'static,
{
    pub id: String,
    pub end: f64,
    pub display: Box<F>,
}

#[async_trait]
impl<F> Task<TasksSubsystem> for EndTask<F>
where
    F: FnOnce(TaskLog) -> String + Send + 'static,
{
    type Output = ();

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        tasks
            .end_task(self.id, self.end, self.display)
            .await?;

        Ok(())
    }
}

pub struct StartBenchmark {
    pub name: String,
}

#[async_trait]
impl Task<TasksSubsystem> for StartBenchmark {
    type Output = ();

    fn log() -> bool {
        false
    }

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        let bench = BenchmarkLog {
            name: self.name.clone(),
            start: 0.0,
            duration: 0.0,
            average: 0.0,
            runs: 0,
            run_time: 0.0,
            display: self.name.clone(),
            max: 0.0,
            min: f64::MAX,
        };

        tasks.start_benchmark(bench).await?;

        Ok(())
    }
}

pub struct EndBenchmark<F>
where
    F: FnOnce(BenchmarkLog) -> String + Send + 'static,
{
    pub name: String,
    pub end: f64,
    pub display: Box<F>,
}

#[async_trait]
impl<F> Task<TasksSubsystem> for EndBenchmark<F>
where
    F: FnOnce(BenchmarkLog) -> String + Send + 'static,
{
    type Output = ();

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        tasks
            .end_benchmark(self.name, self.end, self.display)
            .await?;

        Ok(())
    }
}

pub struct GetTaskDisplays;

#[async_trait]
impl Task<TasksSubsystem> for GetTaskDisplays {
    type Output = Vec<Display>;

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        let task_displays = tasks.get_task_displays().await;

        Ok(task_displays)
    }
}

pub struct GetBenchmarkDisplays;

#[async_trait]
impl Task<TasksSubsystem> for GetBenchmarkDisplays {
    type Output = Vec<Display>;

    async fn execute(self, tasks: &TasksSubsystem) -> anyhow::Result<Self::Output> {
        let task_displays = tasks.get_benchmark_displays().await;

        Ok(task_displays)
    }
}
