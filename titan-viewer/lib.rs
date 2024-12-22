struct Viewer;

#[titan::async_trait]
impl titan::App for Viewer {
    async fn init(&self) -> titan::Result<()> {
        println!("Init!");
        Ok(())
    }

    async fn shutdown(&self) -> titan::Result<()> {
        println!("Shutdown!");
        Ok(())
    }
}

fn main() -> titan::Result<()> {
    titan::run(Viewer)?;
    Ok(())
}
