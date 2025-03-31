use crate::{
    graphics::{self, GraphicsSubsystem},
    terminal::{self, TerminalSubsystem},
    App, Channels,
};
use titan_assets::{assets, ResourceSubsystem};
use titan_core::{chrono, runtime::time::Instant, tasks::{self, TasksSubsystem}, Result};
use titan_core::info;

pub struct EngineSubsystem {
    pub channels: Channels,
    pub quit: bool,
    pub app: Box<dyn App>,
    pub renders: u32,
}

#[titan_core::subsystem]
impl EngineSubsystem {
    
    #[titan_core::task]
    pub async fn init(&self) -> Result<()> {
        
        #[cfg(not(feature = "tracing"))] {
            self.channels
                .get::<TerminalSubsystem>()
                .send_mut(terminal::Init)
                .await??;
        }

        self.channels
            .get::<ResourceSubsystem>()
            .send(assets::Init)
            .await??;
        
        self.channels
            .get::<GraphicsSubsystem>()
            .send(graphics::Init)
            .await??;
             
        Ok(())
    }

    #[titan_core::task]
    pub async fn run(&self) -> Result<()> {
                      
        let benchmark_name = "engine::Fps";
        let frame_start = Instant::now();

        self.channels
            .get::<TasksSubsystem>()
            .send(tasks::StartBenchmark {
                name: benchmark_name,
            })
            .await?;
        
        #[cfg(not(feature = "tracing"))] {
            self.channels
                .get::<TerminalSubsystem>()
                .send_mut(terminal::Render)
                .await??;
        }
                                
        self.channels
            .get::<GraphicsSubsystem>()
            .send(graphics::Render)
            .await??;
        
        // future::try_join_all(vec![frame_render])
        //     .await?;
        
        self.channels
            .get::<TasksSubsystem>()
            .send(tasks::EndBenchmark {
                name: benchmark_name,
                end: frame_start.elapsed().as_secs_f64(),
                display: |bench| {
                    format!(
                        "{:>4.0} [{}] ~ {:>4.0} [{}] <=> [{:.0} - {:.0}]",
                        1.0 / bench.duration,
                        &chrono::format_duration(&bench.duration),
                        (bench.runs as f64) * 1.0 / bench.run_time,
                        &chrono::format_duration(&bench.average),
                        1.0 / bench.max,
                        1.0 / bench.min,
                    )
                },
            })
            .await?;

        Ok(())
    }

    #[titan_core::task]
    pub fn request_quit(&mut self) {
        info!("Quit requested...");
        self.quit = true;
    }

    #[titan_core::task]
    pub fn should_quit(&self) -> bool {
        self.quit
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
            .await??;

        Ok(())
    }
}
