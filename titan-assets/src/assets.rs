use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use titan_core::{subsystem, Subsystem};

#[allow(dead_code)]
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

pub struct AssetsSubsystem {
    assets_dir: PathBuf,
    // pub resources: DashMap<String, Resource>,
}

#[subsystem]
impl AssetsSubsystem {
    #[task]
    pub async fn init(&self) {}

    #[task]
    pub async fn scan(&self) {}
}
