use titan_core::{Result, runtime, anyhow, Channels, ArcLock};

pub struct GraphicsSubsystem {
    pub channels: Channels,
    pub device: ArcLock<Option<wgpu::Device>>,
    pub queue: ArcLock<Option<wgpu::Queue>>,
}

#[titan_core::subsystem]
impl GraphicsSubsystem {
    
    #[titan_core::task]
    async fn init(&self) -> Result<()> {
        let instance = wgpu::Instance::default();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(anyhow!("Graphics: Failed to request adapter"))?;

        let (device, queue) = adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Titan Device"),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await?;

        self.device.write(Some(device))
            .await;
        
        self.queue.write(Some(queue))
            .await;

        Ok(())
    }

    #[titan_core::task(benchmark)]
    async fn render(&self) {
        
    }

    #[titan_core::task]
    async fn shutdown(&self) {
    }
}
