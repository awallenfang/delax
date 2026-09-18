use crate::slint_ui::data_transport::BufferChannel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Channels<T> {
    pub left: T,
    pub right: T,
}

impl<T> Channels<T> {
    pub const fn new(left: T, right: T) -> Self {
        Self { left, right }
    }

    pub fn get(&self, ch: BufferChannel) -> &T {
        match ch {
            BufferChannel::Left => &self.left,
            BufferChannel::Right => &self.right,
        }
    }

    pub fn get_mut(&mut self, ch: BufferChannel) -> &mut T {
        match ch {
            BufferChannel::Left => &mut self.left,
            BufferChannel::Right => &mut self.right,
        }
    }

    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> Channels<U>
    where
        T: Copy,
    {
        Channels {
            left: f(self.left),
            right: f(self.right),
        }
    }
}

impl<T: Default> Default for Channels<T> {
    fn default() -> Self {
        Self {
            left: T::default(),
            right: T::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Heads {
    pub read: f32,
    pub write: f32,
}
