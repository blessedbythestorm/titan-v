use crate::{
    graphics::{self},
    tasks::{self},
    terminal::{self},
    App, Channels,
};
use ad_astra::{
    analysis::{ModuleRead, ScriptModule},
    lady_deirdre::analysis::TriggerHandle,
};
use std::sync::atomic::AtomicBool;
use titan_core::{error, info, runtime::time::Instant, subsystem, Result};

pub struct EngineSubsystem {
    pub channels: Channels,
    pub quit: AtomicBool,
    pub app: Box<dyn App>,
}

#[subsystem]
impl EngineSubsystem {
    #[task]
    pub async fn init(&self) -> Result<()> {
        self.channels
            .terminal
            .send(terminal::Init)
            .await?;

        self.channels
            .graphics
            .send(graphics::Init)
            .await?;

        let package = self.app.reflection();

        info!("Package: {:?}", package);

        let module: ScriptModule = ScriptModule::new(
            package,
            std::fs::read_to_string("./titan-viewer/content/test_script.adastra")?,
        );

        let handle = TriggerHandle::new();

        let read_guard = module.read(&handle, 1)?;
        let script_fn = read_guard.compile()?;

        match script_fn.run() {
            Ok(result) => {
                info!("Script returned: {}", result.stringify(false));
            }
            Err(error) => {
                let module_text = read_guard.text();
                error!("{}", error.display(&module_text));
            }
        }

        Ok(())
    }

    #[task]
    pub async fn run(&self) -> Result<()> {
        while !self
            .quit
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let fps_benchmark = "engine::fps";
            let frame_start = Instant::now();

            self.channels
                .tasks
                .send(tasks::StartBenchmark {
                    name: fps_benchmark.to_string(),
                })
                .await?;

            self.channels
                .terminal
                .send(terminal::Render);

            self.channels
                .graphics
                .send(graphics::Render)
                .await?;

            self.channels
                .tasks
                .send(tasks::EndBenchmark {
                    name: fps_benchmark.to_string(),
                    end: frame_start.elapsed().as_secs_f64(),
                    display: Box::new(|bench: tasks::BenchmarkLog| {
                        format!(
                            "{:.0} [{:.4}s] Avg {:.0}[{:.4}s] Rng [{:.0} - {:.0}]",
                            1.0 / bench.duration,
                            bench.duration,
                            bench.runs as f64 / bench.run_time,
                            bench.run_time / bench.runs as f64,
                            1.0 / bench.min,
                            1.0 / bench.max,
                        )
                    }),
                });
        }

        Ok(())
    }

    #[task]
    pub fn quit(&self) {
        self.quit
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[task]
    pub async fn shutdown(&self) -> Result<()> {
        self.channels
            .graphics
            .send(graphics::Shutdown)
            .await?;

        self.channels
            .terminal
            .send(terminal::Shutdown)
            .await?;

        Ok(())
    }
}
