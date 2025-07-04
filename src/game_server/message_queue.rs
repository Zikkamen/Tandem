use std::{
    sync::{Arc, RwLock},
    collections::VecDeque,
    thread,
    time::Duration,
};

pub struct MessageQueue<T> {
    message_queue: Arc<RwLock<VecDeque<T>>>,
}

impl<T> MessageQueue<T> {
    pub fn new() -> Self {
        MessageQueue { message_queue: Arc::new(RwLock::new(VecDeque::new())) }
    }

    pub fn produce(&self, message: T) {
        let mut message_queue = self.message_queue.write().unwrap();

        if message_queue.len() > 1_000 {
            let _ = message_queue.pop_front();
        }

        message_queue.push_back(message);
    }

    pub fn consume(&self) -> Option<T> {
        self.message_queue.write().unwrap().pop_front()
    }

    pub fn consume_blocking(&self) -> T {
        loop {
            let msg = self.consume();

            let message = match msg {
                Some(v) => v,
                None => {
                    thread::sleep(Duration::from_millis(20));

                    continue;
                }
            };

            return message;
        }
    }

    pub fn clone(&self) -> Self {
        MessageQueue { message_queue: self.message_queue.clone() }
    }
}
