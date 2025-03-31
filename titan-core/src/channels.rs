use std::{
    any::{Any, TypeId}, collections::HashMap, future::Future, pin::Pin, sync::Arc
};
use crate::{subsystem::ErasedSubsystemRef, ArcLock, Event, ImmutableTask, MutableTask, Subsystem, SubsystemRef, Task};


type SubscriberFn = Box<
    dyn Fn(Box<dyn Any + Send + Sync + 'static>, Channels) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct Channels {
    channels: ArcLock<HashMap<TypeId, Arc<dyn ErasedSubsystemRef>>>,
    subscriptions: ArcLock<HashMap<TypeId, Vec<SubscriberFn>>>,
}

impl Channels {
    /// Add a subsystem reference of any type `S` that implements `Subsystem`.
    pub fn add<S: Subsystem>(&mut self, channel: SubsystemRef<S>) {
        if let Ok(mut channels_lock) = self.channels.lock_sync() {
            channels_lock.insert(TypeId::of::<S>(), Arc::new(channel));
        }
    }

    /// Retrieve a subsystem reference by its type `S`.
    pub fn get<S: Subsystem>(&self) -> SubsystemRef<S> {
        let type_id = TypeId::of::<S>();

        if let Ok(channels_lock) = self.channels.read_sync() {
            let erased = channels_lock
                .get(&type_id)
                .unwrap_or_else(|| panic!("Get: No subsystem of type `{}` was registered!", std::any::type_name::<S>()));

            erased
                .as_any()
                .downcast_ref::<SubsystemRef<S>>()
                .expect("TypeId matched but downcast failed.")
                .clone()

        } else {
            panic!("Failed to acquire read lock!");
        }
    }

    pub async fn subscribe<T1, T2>(&self)
    where
        T1: Task + 'static,
        T2: ImmutableTask + From<T1::Inputs> + 'static,
        T1::Inputs: Clone + Send + Sync + 'static,
    {
        // Create a subscriber function
        let subscriber: SubscriberFn = Box::new(move |inputs: Box<dyn Any + Send + Sync + 'static>, channels: Channels| {
            let cloned_inputs = match inputs.downcast::<T1::Inputs>() {
                Ok(boxed) => (*boxed).clone(),
                Err(_) => {
                    panic!("Failed to downcast subscription inputs!");
                }
            };
            // Create the Future
            Box::pin(async move {
                
                let t2_instance: T2 = T2::from(cloned_inputs);

                channels.get::<T2::Subsystem>()
                    .send(t2_instance);

            }) as Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        });

        self.subscriptions
            .lock()
            .await
            .entry(TypeId::of::<T1>())
            .or_insert_with(Vec::new)
            .push(subscriber);
    }

    
    pub async fn subscribe_mut<T1, T2>(&self)
    where
        T1: Task + 'static,
        T2: MutableTask + From<T1::Inputs> + 'static,
        T1::Inputs: Clone + Send + Sync + 'static,
    {
        let subscriber: SubscriberFn = Box::new(
            move |inputs: Box<dyn Any + Send + Sync + 'static>, channels: Channels| {

                let cloned_inputs = match inputs.downcast::<T1::Inputs>() {
                    Ok(boxed) => (*boxed).clone(),
                    Err(_) => {
                        panic!("Failed to downcast subscription inputs!");
                    }
                };

                Box::pin(async move {

                    let t2_instance: T2 = T2::from(cloned_inputs);

                    channels.get::<T2::Subsystem>()
                        .send_mut(t2_instance);
                    
                }) as Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            }
        );

        self.subscriptions
            .lock()
            .await
            .entry(TypeId::of::<T1>())
            .or_insert_with(Vec::new)
            .push(subscriber);
    }

    pub async fn publish<T>(&self, task: T)
    where
        T: ImmutableTask,
        T::Inputs: Clone + Sync + 'static,
    {    
        let type_id = TypeId::of::<T>();
        let sub_lock = self.subscriptions.read().await;
        if let Some(subscriptions) = sub_lock.get(&type_id) {
            for subscription in subscriptions {
                let inputs: Box<dyn Any + Send + Sync + 'static> = Box::new(task.inputs().clone());
                subscription(inputs, self.clone())
                    .await;
            }
        }
    }

    
    pub async fn publish_mut<T>(&self, task: T)
    where
        T: MutableTask,
        T::Inputs: Clone + Sync + 'static,
    {    
        let type_id = TypeId::of::<T>();
        let sub_lock = self.subscriptions.read().await;
        if let Some(subscriptions) = sub_lock.get(&type_id) {
            for subscription in subscriptions {
                let inputs: Box<dyn Any + Send + Sync + 'static> = Box::new(task.inputs().clone());
                subscription(inputs, self.clone())
                    .await;
            }
        }
    }
}

impl Default for Channels {
    fn default() -> Self {
        Self {
            channels: ArcLock::new(HashMap::new()),
            subscriptions: ArcLock::new(HashMap::new()),
        }
    }
}
