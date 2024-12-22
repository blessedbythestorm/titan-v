use crate::{
    graphics::{self, GraphicsSubsystem},
    terminal::{self, TerminalSubsystem},
    App, Channels,
};
use std::sync::atomic::{AtomicBool, Ordering};
use titan_assets::{assets, ResourceSubsystem};
use titan_core::{info, runtime::{self, time::Instant}, tasks::{self, TasksSubsystem}, Result};

pub struct EngineSubsystem {
    pub channels: Channels,
    pub quit: AtomicBool,
    pub app: Box<dyn App>,
    pub mut_test: bool,
}

#[titan_core::subsystem]
impl EngineSubsystem {
    
    #[titan_core::task]
    pub async fn init(&self) -> Result<()> {
        self.channels
            .get::<TerminalSubsystem>()
            .send(terminal::Init)
            .await?;

        let resources = self.channels
            .get::<ResourceSubsystem>()
            .send(assets::Init);
        
        let graphics = self.channels
            .get::<GraphicsSubsystem>()
            .send(graphics::Init);

        let (resources, graphics) = runtime::join!(resources, graphics);

        resources?;
        graphics?;
            
        Ok(())
    }

    #[titan_core::task]
    pub async fn run(&self) -> Result<()> {

        while !self.should_quit() {
        
            let benchmark_name = "engine::Fps";
            let frame_start = Instant::now();

            self.channels
                .get::<TasksSubsystem>()
                .send(tasks::StartBenchmark {
                    name: benchmark_name,
                }).await?;

            self.channels
                .get::<TerminalSubsystem>()
                .send(terminal::Render)
                .await?;
                        
            self.channels
                .get::<TerminalSubsystem>()
                .send(terminal::MutableTask)
                .await?;
            
            self.channels
                .get::<GraphicsSubsystem>()
                .send(graphics::Render)
                .await?;

            self.channels
                .get::<TasksSubsystem>()
                .send(tasks::EndBenchmark {
                    name: benchmark_name,
                    end: frame_start.elapsed().as_secs_f64() * 1000.0,
                    display: Box::new(|bench: tasks::BenchmarkLog| {
                        format!(
                            "{:0<4.0} [{:0<4.2} ms] ~ {:.0} [{:0<4.2} ms] <=> [{:.0} - {:.0}]",
                            1000.0 / bench.duration,
                            bench.duration,
                            (bench.runs as f64) * 1000.0 / bench.run_time,
                            bench.average,
                            1000.0 / bench.max,
                            1000.0 / bench.min,
                        )
                    }),
                }).await?;
        }

        Ok(())
    }

    #[titan_core::task]
    pub fn request_quit(&self) {
        info!("Quit requested...");
        
        self.quit
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn should_quit(&self) -> bool {
        self.quit
            .load(Ordering::Relaxed)
    }

    #[titan_core::task]
    pub async fn shutdown(&self) -> Result<()> {
        self.channels
            .get::<GraphicsSubsystem>()
            .send(graphics::Shutdown)
            .await?;

        self.channels
            .get::<TerminalSubsystem>()
            .send(terminal::Shutdown)
            .await?;

        Ok(())
    }
}
