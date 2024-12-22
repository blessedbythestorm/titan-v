use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{future::Future, sync::Arc};

pub struct ArcLock<T>
where
    T: Send + 'static
{
    data: Arc<RwLock<T>>,
}

impl<T> ArcLock<T>
where
    T: Send + 'static
{    
    pub fn new(value: T) -> ArcLock<T> {
        let data = Arc::new(RwLock::new(value));
        
        ArcLock {
            data,
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        self.data.read().await
    }
        
    pub async fn lock(&self) -> RwLockWriteGuard<'_, T> {
        self.data.write().await
    }

    pub async fn write(&self, val: T) {
        let mut lock = self.data.write().await;
        *lock = val;
    }

    pub async fn write_with<F, Fut>(&self, f: F)
    where
        F: FnOnce(&mut T) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut lock = self.data.write().await;
        f(&mut lock);
    }    
}

impl<T> Clone for ArcLock<T>
where
    T: Send + 'static
{
    fn clone(&self) -> Self {
        ArcLock {
            data: Arc::clone(&self.data),
        }
    }
}
