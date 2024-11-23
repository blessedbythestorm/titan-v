mod channels;
mod engine;
mod graphics;
mod terminal;

use ad_astra::export;
use ad_astra::runtime::PackageMeta;
use channels::Channels;
use channels::CHANNELS;
use engine::EngineSubsystem;
use graphics::GraphicsSubsystem;
use orx_concurrent_option::ConcurrentOption;
use std::sync::{atomic::AtomicBool, Arc};
use tasks::TasksSubsystem;
use terminal::{TermView, TerminalSubsystem};
use titan_core::{
    anyhow,
    runtime::{runtime::Builder, sync::Mutex},
    tasks, IndexMap, Subsystem, SubsystemRef,
};
pub use titan_core::{async_trait, Result};

#[export(package)]
#[derive(Default)]
pub struct TitanPackage;

#[async_trait]
pub trait App: Send + Sync + 'static {
    async fn init(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
    fn reflection(&self) -> &'static PackageMeta;
}

pub fn run(app: impl App) -> Result<()> {
    let runtime = Builder::new_multi_thread()
        .enable_time()
        .build()?;

    let run_result: Result<()> = runtime.block_on(async move {
        let channels = init_subsystems(app)?;

        channels
            .engine
            .send(engine::Init)
            .await?;

        channels
            .engine
            .send(engine::Run)
            .await?;

        channels
            .engine
            .send(engine::Shutdown)
            .await?;

        Ok(())
    });

    run_result
}

pub fn init_subsystems(app: impl App) -> Result<Channels> {
    let (engine_ref, engine_receiver) = SubsystemRef::<EngineSubsystem>::new();
    let (graphics_ref, graphics_receiver) = SubsystemRef::<GraphicsSubsystem>::new();
    let (terminal_ref, terminal_receiver) = SubsystemRef::<TerminalSubsystem>::new();
    let (tasks_ref, tasks_receiver) = SubsystemRef::<TasksSubsystem>::new();

    let channels = Channels {
        engine: engine_ref,
        graphics: graphics_ref,
        terminal: terminal_ref,
        tasks: tasks_ref,
    };

    TerminalSubsystem::start_quiet(
        TasksSubsystem {
            tasks: Arc::new(Mutex::new(IndexMap::new())),
            benchmarks: Arc::new(Mutex::new(IndexMap::new())),
        },
        tasks_receiver,
    );

    TerminalSubsystem::start(
        TerminalSubsystem {
            channels: channels.clone(),
            terminal: Arc::new(Mutex::new(None)),
            view: Arc::new(Mutex::new(TermView::Tasks)),
        },
        terminal_receiver,
        channels.tasks.clone(),
    );

    GraphicsSubsystem::start(
        GraphicsSubsystem {
            channels: channels.clone(),
            device: ConcurrentOption::none(),
            queue: ConcurrentOption::none(),
        },
        graphics_receiver,
        channels.tasks.clone(),
    );

    EngineSubsystem::start(
        EngineSubsystem {
            channels: channels.clone(),
            quit: AtomicBool::new(false),
            app: Box::new(app),
        },
        engine_receiver,
        channels.tasks.clone(),
    );

    CHANNELS
        .set(Arc::new(channels.clone()))
        .map_err(|_| anyhow!("Failed to set global CHANNELS"))?;

    Ok(channels)
}
