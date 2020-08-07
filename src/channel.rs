use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}
impl<T> Sender<T> {
    pub fn send(&self, t: T) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.queue.push_back(t);
        // Drop before wakeup so the other thread can immediately pick up the lock
        drop(inner);
        self.shared.available.notify_one();
    }
}
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.senders += 1;
        // Drop before wakeup so the other thread can immediately pick up the lock
        drop(inner);
        Sender {
            shared: Arc::clone(&self.shared),
        }
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.senders -= 1;
        let was_last = inner.senders == 0;
        // Drop before wakeup so the other thread can immediately pick up the lock
        drop(inner);
        if was_last {
            self.shared.available.notify_one();
        }
    }
}
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}
impl<T> Receiver<T> {
    /// The &mut self isn't required here, but since this implementation is written for single receiver (like mpsc) we make this a mut (exclusive access)
    pub fn recv(&mut self) -> Option<T> {
        // Async / Await is more for IO bounds and not CPU bounds. If you are CPU bound, a Condvar is better. ~ 29 min in https://youtu.be/b4mS5UPHh20
        let mut inner = self.shared.inner.lock().unwrap();
        // Note that this does not actually loop and throttle CPU, our Condvar puts the CPU to sleep until woken up again
        loop {
            match inner.queue.pop_front() {
                Some(t) => return Some(t),
                None if inner.senders == 0 => return None,
                None => {
                    // Wait hands over the guard saying "fine you can have it; wake me up when I can take it back"
                    inner = self.shared.available.wait(inner).unwrap();
                }
            }
        }
    }
}
struct Inner<T> {
    queue: VecDeque<T>,
    senders: usize,
}
impl<T> Default for Inner<T> {
    fn default() -> Self {
        Inner {
            queue: VecDeque::default(),
            senders: 1,
        }
    }
}
struct Shared<T> {
    inner: Mutex<Inner<T>>,
    available: Condvar,
}
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Shared {
        inner: Default::default(),
        available: Default::default(),
    };
    let shared = Arc::new(shared);

    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}
