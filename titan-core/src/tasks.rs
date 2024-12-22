use crate::{subsystem::{Subsystem, Task}, ArcLock};
use indexmap::IndexMap;
use titan_macro::{subsystem, task};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use tokio::sync::Mutex;
use crate::{Result, async_trait};

pub struct Display {
    pub name: String,
    pub display: String,
}

#[derive(Clone)]
pub struct TaskLog {
    pub id: String,
    pub name: &'static str,
    pub depth: usize,
    pub complete: bool,
    pub start: f64,
    pub duration: f64,
    pub display: String,
}

#[derive(Clone)]
pub struct BenchmarkLog {
    pub name: &'static str,
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
    pub tasks: ArcLock<IndexMap<String, TaskLog>>,
    pub benchmarks: ArcLock<IndexMap<&'static str, BenchmarkLog>>,
}

#[crate::subsystem]
impl TasksSubsystem {

    #[crate::task]
    async fn start_task(&self, id: String, name: &'static str, depth: usize) -> Result<()> {
         
        let task = TaskLog {
            id,
            name,
            depth,
            complete: false,
            start: 0.0,
            duration: 0.0,
            display: "Exec...".to_string(),
        };

        self.tasks
            .lock()
            .await
            .entry(task.id.clone())
            .or_insert(task);

        Ok(())
    }

    #[crate::task]
    async fn end_task<F>(&self, id: String, end: f64, display: Box<F>)
    where
        F: FnOnce(TaskLog) -> String + Send + 'static,
    {
        self.tasks
            .lock()
            .await
            .entry(id)
            .and_modify(|task| {
                task.complete = true;
                task.duration = end;
                task.display = display(task.clone());
            });
    }

    #[crate::task]
    async fn get_task_displays(&self) -> Vec<Display> {
        let task_lock = self.tasks.lock().await;

        task_lock
            .values()
            .map(|task| Display {
                name: format!("{} - {}", task.name, task.depth),
                display: task.display.clone(),
            })
            .collect()
    }

    #[crate::task]
     async fn start_benchmark(&self, name: &'static str) {
        let bench = BenchmarkLog {
            name,
            start: 0.0,
            duration: 0.0,
            average: 0.0,
            runs: 0,
            run_time: 0.0,
            display: String::from(name),
            max: 0.0,
            min: f64::MAX,
        };
         
        self.benchmarks
            .lock()
            .await
            .entry(bench.name)
            .or_insert(bench);
    }

    #[crate::task]
    async fn end_benchmark<F>(&self, name: &'static str, end: f64, display: Box<F>)
    where
        F: FnOnce(BenchmarkLog) -> String + Send + 'static,
    {
        self.benchmarks
            .lock()
            .await
            .entry(name)
            .and_modify(|task| {
                task.duration = end;
                task.run_time += task.duration;
                task.runs += 1;
                task.average = task.run_time / task.runs as f64;
                task.max = f64::max(task.duration, task.max);
                task.min = f64::min(task.duration, task.min);
                task.display = display(task.clone())
            });
    }

    #[crate::task]
    async fn get_benchmark_displays(&self) -> Vec<Display> {
        let bench_lock = self.benchmarks.lock().await;

        bench_lock
            .values()
            .map(|bench| Display {
                name: bench.name.to_string(),
                display: bench.display.clone(),
            })
            .collect()
    }
}
