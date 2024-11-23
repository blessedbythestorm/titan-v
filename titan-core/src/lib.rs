mod subsystem;
pub mod tasks;

pub use ad_astra as scripting;
use ad_astra::export;
pub use anyhow::{anyhow, Result};
pub use async_trait::async_trait;
pub use dashmap::DashMap;
pub use futures;
pub use indexmap::IndexMap;
pub use log;
pub use subsystem::{Subsystem, SubsystemRef, Task};
pub use titan_macro::{subsystem, task};
pub use tokio as runtime;
pub use tracing::{debug, error, info, trace, warn};
pub use tracing_subscriber;

#[export(package)]
#[derive(Default)]
pub struct TitanCorePackage;
