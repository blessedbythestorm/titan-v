use ad_astra::export;

#[export(package)]
#[derive(Default)]
struct {app_name}Package;

struct {app_name};

#[titan::async_trait]
impl titan::App for {app_name} {
    async fn init(&self) -> titan::Result<()> {
        println!("Init!");
        Ok(())
    }

    async fn shutdown(&self) -> titan::Result<()> {
        println!("Shutdown!");
        Ok(())
    }
}

pub fn entry() -> titan::Result<()> {
    titan::run({app_name})?;
    Ok(())
}
