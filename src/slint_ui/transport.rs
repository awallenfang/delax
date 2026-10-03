use std::any::Any;
use std::sync::LazyLock;
use std::sync::Mutex;

use anymap3::Map;

use crate::slint_ui::frames::UiFrame;
pub use crate::slint_ui::frames::BufferChannel;

pub static FRAME_STORE: LazyLock<BufferStorage> =
    LazyLock::new(|| BufferStorage {
        map: Mutex::new(Map::new()),
    });

pub struct BufferStorage {
    map: Mutex<Map<dyn Any + Send + Sync>>,
}

impl BufferStorage {
    pub fn channel<T: Clone + Default + Send + Sync + 'static>(
        &self,
        initial: T,
    ) -> triple_buffer::Input<T> {
        let (input, output) = triple_buffer::triple_buffer(&initial);
        self.map.lock().unwrap().insert(output);
        input
    }

    pub fn read<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.map
            .lock()
            .unwrap()
            .get_mut::<triple_buffer::Output<T>>()
            .map(|output| output.read().clone())
    }

    pub fn with_store<R>(&self, f: impl FnOnce(&mut Map<dyn Any + Send + Sync>) -> R) -> R {
        let mut map = self.map.lock().unwrap();
        f(&mut map)
    }
}

pub fn active_len_for(ch: BufferChannel) -> usize {
    FRAME_STORE
        .read::<UiFrame>()
        .map(|f| *f.active_len.get(ch))
        .unwrap_or(0)
}
