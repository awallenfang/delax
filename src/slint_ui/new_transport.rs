use std::any::Any;
use std::sync::LazyLock;
use std::sync::Mutex;

use anymap3::Map;

pub static StorageSingleton: LazyLock<BufferStorage> = LazyLock::new(|| {
    BufferStorage {
        map: Mutex::new(Map::new()),
    }
});

pub struct BufferStorage {
    map: Mutex<Map<dyn Any + Send + Sync>>,
}

impl BufferStorage {
    /// Insert a new receiver into the registry, keyed by its data type.
    /// Each type can only have one receiver registered.
    pub fn insert<T: Send + Sync + 'static>(&self, buffer: triple_buffer::Output<T>) {
        self.map.lock().unwrap().insert(buffer);
    }

    /// Read the latest data from a receiver of type T, cloning the value.
    /// Returns `Some(data)` if a receiver of this type exists.
    pub fn read<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.map
            .lock()
            .unwrap()
            .get_mut::<triple_buffer::Output<T>>()
            .map(|output| output.read().clone())
    }
}
