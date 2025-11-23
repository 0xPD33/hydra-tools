use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Stores the last N messages per channel for late subscribers.
/// Uses a ring buffer (VecDeque) to maintain constant memory usage.
/// Default capacity: 100 messages per channel.
struct ReplayBuffer {
    messages: VecDeque<String>,
    capacity: usize,
}

impl ReplayBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, msg: String) {
        if self.messages.len() >= self.capacity {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    fn get_all(&self) -> Vec<String> {
        self.messages.iter().cloned().collect()
    }
}

static BROADCAST_CHANNELS: Lazy<Arc<tokio::sync::Mutex<HashMap<(Uuid, String), (broadcast::Sender<String>, ReplayBuffer)>>>> =
    Lazy::new(|| Arc::new(tokio::sync::Mutex::new(HashMap::new())));

pub async fn get_or_create_broadcast_tx(project_uuid: Uuid, topic: &str) -> broadcast::Sender<String> {
    let key = (project_uuid, topic.to_string());
    let mut map = BROADCAST_CHANNELS.lock().await;
    // Get or create the sender+buffer tuple - the HashMap keeps the original sender alive
    // which keeps the channel open. We clone the sender to return.
    let (tx, _buffer) = map.entry(key.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(1024);
            let buffer = ReplayBuffer::new(100); // Store last 100 messages
            (tx, buffer)
        });
    tx.clone()
}

/// Emit a message and store it in the replay buffer atomically
/// Returns the number of receivers that received the message (0 if no active receivers)
pub async fn emit_and_store(project_uuid: Uuid, topic: &str, message: String) -> usize {
    let key = (project_uuid, topic.to_string());
    let mut map = BROADCAST_CHANNELS.lock().await;

    let (tx, buffer) = map.entry(key.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(1024);
            let buffer = ReplayBuffer::new(100);
            (tx, buffer)
        });

    // Store in replay buffer first (always succeeds)
    buffer.push(message.clone());

    // Then broadcast - if there are no receivers, that's OK, we stored it
    tx.send(message).unwrap_or(0)
}

/// Subscribe to a broadcast channel and get message history
pub async fn subscribe_broadcast(project_uuid: Uuid, topic: &str) -> (broadcast::Receiver<String>, Vec<String>) {
    let key = (project_uuid, topic.to_string());
    let mut map = BROADCAST_CHANNELS.lock().await;

    // Use entry API to atomically get-or-create
    let (tx, buffer) = map.entry(key)
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(1024);
            let buffer = ReplayBuffer::new(100);
            (tx, buffer)
        });

    let rx = tx.subscribe();
    let history = buffer.get_all();
    (rx, history)
}

// List active channels for a project
pub async fn list_channels(project_uuid: Uuid) -> Vec<String> {
    let broadcast_map = BROADCAST_CHANNELS.lock().await;
    let mut channels: Vec<String> = broadcast_map
        .keys()
        .filter_map(|(uuid, topic)| {
            if *uuid == project_uuid {
                Some(topic.clone())
            } else {
                None
            }
        })
        .collect();
    channels.sort();
    channels.dedup();
    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_or_create_broadcast_tx_same_instance() {
        let uuid = Uuid::new_v4();
        let topic = "test";
        let tx1 = get_or_create_broadcast_tx(uuid, topic).await;
        let tx2 = get_or_create_broadcast_tx(uuid, topic).await;
        
        // Test they're the same by sending to one and receiving from both
        let mut rx1 = tx1.subscribe();
        let mut rx2 = tx2.subscribe();
        
        let msg = "test message".to_string();
        tx1.send(msg.clone()).unwrap();
        
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1, msg);
        assert_eq!(received2, msg);
    }

    #[tokio::test]
    async fn test_subscribe_broadcast_send_recv() {
        let uuid = Uuid::new_v4();
        let topic = "test";
        let (mut rx, history) = subscribe_broadcast(uuid, topic).await;
        // New subscribers should have no history since channel is empty
        assert_eq!(history.len(), 0);

        let tx = get_or_create_broadcast_tx(uuid, topic).await;
        let msg = "hello world".to_string();
        tx.send(msg.clone()).unwrap();
        let received: String = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_replay_buffer_stores_messages() {
        let uuid = Uuid::new_v4();
        let topic = "replay:test";

        // Emit 3 messages using emit_and_store
        emit_and_store(uuid, topic, "msg1".to_string()).await;
        emit_and_store(uuid, topic, "msg2".to_string()).await;
        emit_and_store(uuid, topic, "msg3".to_string()).await;

        // Late subscriber gets history
        let (_rx, history) = subscribe_broadcast(uuid, topic).await;
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], "msg1");
        assert_eq!(history[1], "msg2");
        assert_eq!(history[2], "msg3");
    }

    #[tokio::test]
    async fn test_replay_buffer_capacity_limit() {
        let uuid = Uuid::new_v4();
        let topic = "capacity:test";

        // Emit 150 messages (buffer capacity is 100)
        for i in 0..150 {
            emit_and_store(uuid, topic, format!("msg{}", i)).await;
        }

        // Should only have last 100 messages
        let (_rx, history) = subscribe_broadcast(uuid, topic).await;
        assert_eq!(history.len(), 100);
        assert_eq!(history[0], "msg50"); // First 50 were dropped
        assert_eq!(history[99], "msg149");
    }

    #[tokio::test]
    async fn test_replay_buffer_late_subscriber() {
        let uuid = Uuid::new_v4();
        let topic = "late:test";

        // Emit before any subscriber exists
        emit_and_store(uuid, topic, "early1".to_string()).await;
        emit_and_store(uuid, topic, "early2".to_string()).await;

        // First subscriber gets history
        let (mut rx1, history1) = subscribe_broadcast(uuid, topic).await;
        assert_eq!(history1.len(), 2);
        assert_eq!(history1[0], "early1");
        assert_eq!(history1[1], "early2");

        // Emit new message
        emit_and_store(uuid, topic, "new1".to_string()).await;

        // First subscriber gets live message
        let live1 = rx1.recv().await.unwrap();
        assert_eq!(live1, "new1");

        // Second late subscriber gets all 3 from history
        let (_rx2, history2) = subscribe_broadcast(uuid, topic).await;
        assert_eq!(history2.len(), 3);
        assert_eq!(history2[0], "early1");
        assert_eq!(history2[1], "early2");
        assert_eq!(history2[2], "new1");
    }

    #[tokio::test]
    async fn test_multiple_channels_isolated() {
        let uuid = Uuid::new_v4();

        // Emit to channel A
        emit_and_store(uuid, "channel_a", "msg_a1".to_string()).await;
        emit_and_store(uuid, "channel_a", "msg_a2".to_string()).await;

        // Emit to channel B
        emit_and_store(uuid, "channel_b", "msg_b1".to_string()).await;

        // Verify channel A has only its messages
        let (_rx_a, history_a) = subscribe_broadcast(uuid, "channel_a").await;
        assert_eq!(history_a.len(), 2);
        assert_eq!(history_a[0], "msg_a1");
        assert_eq!(history_a[1], "msg_a2");

        // Verify channel B has only its messages
        let (_rx_b, history_b) = subscribe_broadcast(uuid, "channel_b").await;
        assert_eq!(history_b.len(), 1);
        assert_eq!(history_b[0], "msg_b1");
    }

    #[tokio::test]
    async fn test_different_projects_isolated() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        let topic = "shared:name";

        // Emit to project 1
        emit_and_store(uuid1, topic, "project1_msg".to_string()).await;

        // Emit to project 2
        emit_and_store(uuid2, topic, "project2_msg".to_string()).await;

        // Verify project 1 only sees its message
        let (_rx1, history1) = subscribe_broadcast(uuid1, topic).await;
        assert_eq!(history1.len(), 1);
        assert_eq!(history1[0], "project1_msg");

        // Verify project 2 only sees its message
        let (_rx2, history2) = subscribe_broadcast(uuid2, topic).await;
        assert_eq!(history2.len(), 1);
        assert_eq!(history2[0], "project2_msg");
    }
}
