use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{subsystem::ErasedSubsystemRef, Subsystem, SubsystemRef};

#[derive(Clone, Default)]
pub struct Channels {
    channels: HashMap<TypeId, Arc<dyn ErasedSubsystemRef>>,
}

impl Channels {
    /// Add a subsystem reference of any type S that implements `Subsystem`.
    pub fn add<S: Subsystem>(&mut self, channel: SubsystemRef<S>) {
        self.channels.insert(TypeId::of::<S>(), Arc::new(channel));
    }

    /// Retrieve a subsystem reference by its type S.
    pub fn get<S: Subsystem>(&self) -> SubsystemRef<S> {
        let type_id = TypeId::of::<S>();

        let erased = self.channels.get(&type_id)
            .unwrap_or_else(|| panic!("No subsystem of that type was registered!"));

        // Downcast from `&dyn Any` to `&SubsystemRef<S>`.
        erased.as_any().downcast_ref::<SubsystemRef<S>>()
            .expect("TypeId matched but downcast failed.")
            .clone()
    }
}
