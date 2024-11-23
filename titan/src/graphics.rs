use crate::Channels;
use orx_concurrent_option::ConcurrentOption;
use titan_core::{anyhow, runtime, subsystem, Result};

pub struct GraphicsSubsystem {
    pub channels: Channels,
    pub device: ConcurrentOption<wgpu::Device>,
    pub queue: ConcurrentOption<wgpu::Queue>,
}

#[subsystem]
impl GraphicsSubsystem {
    #[task]
    async fn init(&self) -> Result<()> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(anyhow!("Graphics: Failed to request adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Titan Device"),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await?;

        self.device.replace(device);
        self.queue.replace(queue);

        Ok(())
    }

    #[task(benchmark)]
    async fn render(&self) -> Result<()> {
        runtime::time::sleep(std::time::Duration::from_millis(8)).await;
        Ok(())
    }

    #[task]
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
