use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult, Debouncer, RecommendedCache};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use titan_core::{error, info, ArcLock, Channels, Result};

pub struct DiskResourceDef {
    extensions: &'static [&'static str],
}

pub enum DiskResourceType {
    Model(DiskResourceDef),
    Texture(DiskResourceDef),
    Shader(DiskResourceDef),
    Script(DiskResourceDef),
    Data(DiskResourceDef),
}

const DISK_RESOURCE_TYPES: &[&DiskResourceType] = &[
    &DiskResourceType::Model(DiskResourceDef {
        extensions: &["fbx", "obj", "gltf", "glb"],
    }),
    &DiskResourceType::Texture(DiskResourceDef {
        extensions: &["png", "jpg", "jpeg", "hdr"],
    }),
    &DiskResourceType::Shader(DiskResourceDef {
        extensions: &["vert", "frag", "comp"],
    }),
    &DiskResourceType::Script(DiskResourceDef {
        extensions: &["lua"],
    }),
    &DiskResourceType::Data(DiskResourceDef {
        extensions: &["json", "yaml", "toml"],
    }),
];

#[derive(Serialize, Deserialize)]
pub struct AssetsConfig {
    pub assets_dir: String,
}

pub struct ResourceSubsystem {
    pub channels: Channels,
    pub assets_dir: PathBuf,
    pub watcher: ArcLock<Option<Debouncer<RecommendedWatcher, RecommendedCache>>>
    // pub resources: DashMap<String, Resource>,
}

#[titan_core::subsystem]
impl ResourceSubsystem {
    
    #[titan_core::task]
    pub async fn init(&self) -> Result<()> {
    
        let watcher = new_debouncer(
            Duration::from_secs(2),
            None,
            |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        events.into_iter()
                            .for_each(|event| {
                                Self::watcher_event(&event);
                            });
                    },
                    Err(errors) => {
                        errors.into_iter()
                            .for_each(|error| {
                                error!("Error: {:?}", error);                                    
                            });
                    }
                }
            }
        )
        .expect("Failed to create file watcher!");

        self.watcher.write(Some(watcher))
            .await;

        {
            let watch_dir = std::env::current_dir()?
                .join(&self.assets_dir);
            
            let mut watcher_lock = self.watcher.lock()
                .await;

            watcher_lock
                .as_mut()
                .expect("Failed to get watcher!")
                .watch(&watch_dir, RecursiveMode::Recursive)
                .unwrap_or_else(|err| error!("Failed to start watching: {:?}", err));
        }
        
        Ok(())
    }

    fn watcher_event(event: &Event) {
        match event.kind {
            EventKind::Create(_) => {
                info!("Created files: {:?}", event.paths);
            },
            EventKind::Modify(_) => {
                info!("Modified files: {:?}", event.paths);
            },
            EventKind::Remove(_) => {
                info!("Removed files: {:?}", event.paths);
            },
            _ => {}
        }
    }

    #[titan_core::task]
    pub async fn scan(&self) -> Result<()> {
        
        Ok(())
    }
}
