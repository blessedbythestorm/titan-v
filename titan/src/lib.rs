mod engine;
mod graphics;
mod terminal;

use engine::EngineSubsystem;
use graphics::GraphicsSubsystem;
use std::path::PathBuf;
use tasks::TasksSubsystem;
use terminal::{TermView, TerminalSubsystem};
use titan_assets::ResourceSubsystem;
use titan_core::{
    runtime::runtime::Builder, tasks, tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter},
    ArcLock, Channels, IndexMap, Subsystem, SubsystemRef
};

pub use titan_core::{async_trait, Result, info, error, warn};

#[async_trait]
pub trait App: Send + Sync + 'static {
    async fn init(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

pub fn run(app: impl App) -> Result<()> {

    #[cfg(feature = "tracing")] {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));
        
        let subscriber = tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_file(false)
            .with_level(true)
            .with_line_number(false)
            .without_time()
            .with_thread_names(false)
            .with_target(false);

        tracing_subscriber::registry()
            .with(filter)
            .with(subscriber)
            .init();
    }
            
    let runtime = Builder::new_multi_thread()
        .thread_name("titan")
        .enable_all()
        .build()?;

    let run_result: Result<()> = runtime.block_on(async move {        
        let channels = start_subsystems(app)?;

        channels
            .get::<EngineSubsystem>()
            .send(engine::Init)
            .await??;

        // Note: Don't lock subsystem tasks in an
        // infinite loop as this can potentiallly
        // interfere with concurrency.
        // Better to have our main loop in the main thread
        // unbounded from any subsystem.

        let mut engine_quit = false;
        
        while !engine_quit {
            channels
                .get::<EngineSubsystem>()
                .send(engine::Run)
                .await??;

            engine_quit = channels
                .get::<EngineSubsystem>()
                .send(engine::ShouldQuit)
                .await?;
        }

        info!("Shutting down...");

        channels
            .get::<EngineSubsystem>()
            .send(engine::Shutdown)
            .await??;

        Ok(())
    });

    run_result
}

pub fn start_subsystems(app: impl App) -> Result<Channels> {
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

    TasksSubsystem::start_quiet(
        TasksSubsystem {
            channels: channels.clone(),
            tasks: ArcLock::new(IndexMap::new()),
            benchmarks: ArcLock::new(IndexMap::new()),
        },
        tasks_receiver,
    );

    TerminalSubsystem::start(
        TerminalSubsystem {
            channels: channels.clone(),
            terminal: None,
            view: TermView::Tasks,
            task_displays: Vec::new(),
        },
        terminal_receiver,
        channels.get::<TasksSubsystem>(),
    );

    ResourceSubsystem::start(
        ResourceSubsystem {
            channels: channels.clone(),
            assets_dir: PathBuf::from("/resources"),
            watcher: ArcLock::new(None),
        },
        resources_receiver,
        channels.get::<TasksSubsystem>(),
    );

    GraphicsSubsystem::start(
        GraphicsSubsystem {
            channels: channels.clone(),
            device: ArcLock::new(None),
            queue: ArcLock::new(None),
        },
        graphics_receiver,
        channels.get::<TasksSubsystem>(),
    );

    EngineSubsystem::start(
        EngineSubsystem {
            channels: channels.clone(),
            quit: false,
            app: Box::new(app),
            renders: 0,
        },
        engine_receiver,
        channels.get::<TasksSubsystem>(),
    );

    Ok(channels)
}

pub async fn stop_subsystems(stops: Vec<ArcLock<bool>>) {
    for stop in stops.into_iter() {
        stop.write(true)
            .await;
    }
}
