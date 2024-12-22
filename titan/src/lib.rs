mod engine;
mod graphics;
mod terminal;

use engine::EngineSubsystem;
use graphics::GraphicsSubsystem;
use std::{path::PathBuf, str::FromStr, sync::{atomic::AtomicBool, Arc}};
use tasks::TasksSubsystem;
use terminal::{TermView, TerminalSubsystem};
use titan_assets::ResourceSubsystem;
use titan_core::{
    runtime::runtime::Builder, tasks, tracing, tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt, Layer}, ArcLock, Channels, IndexMap, Subsystem, SubsystemRef
};

pub use titan_core::{async_trait, Result, info, error, warn};

#[async_trait]
pub trait App: Send + Sync + 'static {
    async fn init(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

pub fn run(app: impl App) -> Result<()> {
    // let subscriber = tracing_subscriber::fmt::layer()
    //     .with_ansi(true)
    //     .with_file(false)
    //     .with_level(true)
    //     .with_line_number(false)
    //     .without_time()
    //     .with_thread_names(false)
    //     .with_target(false);

    // tracing_subscriber::registry()
    //     .with(subscriber)
    //     .init();
            
    let runtime = Builder::new_multi_thread()
        .thread_name("titan")
        .enable_all()
        .build()?;

    let run_result: Result<()> = runtime.block_on(async move {        
        let (channels, stops) = start_subsystems(app)?;

        channels
            .get::<EngineSubsystem>()
            .send(engine::Init)
            .await?;

        channels
            .get::<EngineSubsystem>()
            .send(engine::Run)
            .await?;

        channels
            .get::<EngineSubsystem>()
            .send(engine::Shutdown)
            .await?;

        stop_subsystems(stops)
            .await;

        Ok(())
    });

    run_result
}

pub fn start_subsystems(app: impl App) -> Result<(Channels, Vec<ArcLock<bool>>)> {
    let (engine_ref, engine_receiver) = SubsystemRef::<EngineSubsystem>::new();
    let (graphics_ref, graphics_receiver) = SubsystemRef::<GraphicsSubsystem>::new();
    let (terminal_ref, terminal_receiver) = SubsystemRef::<TerminalSubsystem>::new();
    let (tasks_ref, tasks_receiver) = SubsystemRef::<TasksSubsystem>::new();
    let (resources_ref, resources_receiver) = SubsystemRef::<ResourceSubsystem>::new();

    let mut channels = Channels::default();
    
    channels.add(engine_ref);
    channels.add(graphics_ref);
    channels.add(terminal_ref);
    channels.add(tasks_ref);
    channels.add(resources_ref);

    let task_stop = TasksSubsystem::start_quiet(
        TasksSubsystem {
            tasks: ArcLock::new(IndexMap::new()),
            benchmarks: ArcLock::new(IndexMap::new()),
        },
        tasks_receiver,
    );

    let term_stop = TerminalSubsystem::start(
        TerminalSubsystem {
            channels: channels.clone(),
            terminal: ArcLock::new(None),
            view: ArcLock::new(TermView::Tasks),
            mut_test: false,
        },
        terminal_receiver,
        channels.get::<TasksSubsystem>(),
    );

    let res_stop = ResourceSubsystem::start(
        ResourceSubsystem {
            assets_dir: PathBuf::from("/resources"),
            watcher: ArcLock::new(None),
        },
        resources_receiver,
        channels.get::<TasksSubsystem>(),
    );

    let gfx_stop = GraphicsSubsystem::start(
        GraphicsSubsystem {
            channels: channels.clone(),
            device: ArcLock::new(None),
            queue: ArcLock::new(None),
        },
        graphics_receiver,
        channels.get::<TasksSubsystem>(),
    );

    let eng_stop = EngineSubsystem::start(
        EngineSubsystem {
            channels: channels.clone(),
            quit: AtomicBool::new(false),
            app: Box::new(app),
            mut_test: false,
        },
        engine_receiver,
        channels.get::<TasksSubsystem>(),
    );

    let stops = vec![
        gfx_stop,
        res_stop,
        eng_stop,
        term_stop,
        task_stop,
    ];

    Ok((channels, stops))
}

pub async fn stop_subsystems(stops: Vec<ArcLock<bool>>) {
    for stop in stops.into_iter() {
        stop.write(true)
            .await;
    }
}
