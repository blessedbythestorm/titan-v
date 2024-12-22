use titan::info;

struct Viewer;

#[titan::async_trait]
impl titan::App for Viewer {
    async fn init(&self) -> titan::Result<()> {
        info!("Init!");
        Ok(())
    }

    async fn shutdown(&self) -> titan::Result<()> {
        info!("Shutdown!");
        Ok(())
    }
}

pub fn run() -> titan::Result<()> {
    titan::run(Viewer)?;
    Ok(())
}
