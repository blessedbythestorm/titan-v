use crate::{engine::EngineSubsystem, graphics::GraphicsSubsystem, terminal::TerminalSubsystem};
use std::sync::{Arc, OnceLock};
use titan_core::{tasks::TasksSubsystem, SubsystemRef};

#[derive(Clone)]
pub struct Channels {
    pub engine: SubsystemRef<EngineSubsystem>,
    pub graphics: SubsystemRef<GraphicsSubsystem>,
    pub terminal: SubsystemRef<TerminalSubsystem>,
    pub tasks: SubsystemRef<TasksSubsystem>,
}

pub static CHANNELS: OnceLock<Arc<Channels>> = OnceLock::new();

pub fn channels() -> Arc<Channels> {
    CHANNELS
        .get()
        .cloned()
        .expect("Failed to get global CHANNELS")
}
