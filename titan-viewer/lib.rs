use ad_astra::{export, runtime::PackageMeta, runtime::ScriptPackage};

#[export(package)]
#[derive(Default)]
pub struct ViewerPackage;

#[export]
pub fn hello_app() -> String {
    "42".to_string()
}

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

    fn reflection(&self) -> &'static PackageMeta {
        ViewerPackage::meta()
    }
}

fn main() -> titan::Result<()> {
    titan::run(Viewer)?;
    Ok(())
}
