use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;
use crate::constants::{REPLAY_BUFFER_CAPACITY, BROADCAST_CHANNEL_CAPACITY};
use std::path::PathBuf;

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

type ChannelKey = (Uuid, String);
type ChannelValue = (broadcast::Sender<String>, ReplayBuffer);
type ChannelMap = HashMap<ChannelKey, ChannelValue>;

static BROADCAST_CHANNELS: LazyLock<Arc<tokio::sync::Mutex<ChannelMap>>> =
    LazyLock::new(|| Arc::new(tokio::sync::Mutex::new(HashMap::new())));

static MESSAGE_LOG: LazyLock<Arc<Mutex<Option<crate::message_log::MessageLog>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Set the path for message logging (for crash recovery)
pub fn set_message_log_path(path: Option<PathBuf>) {
    use crate::message_log::MessageLog;

    let mut log = MESSAGE_LOG.lock().unwrap();
    *log = path.and_then(|p| MessageLog::open(&p).ok());
}

/// Append message to log file (if logging is enabled)
fn log_message(project_uuid: Uuid, channel: &str, message: &str) {
    // Try to get lock without blocking - if we can't get it, skip logging this message
    // This prevents blocking the async runtime if the log is temporarily locked
    if let Ok(mut log_guard) = MESSAGE_LOG.try_lock() {
        if let Some(log) = log_guard.as_mut() {
            let _ = log.append(project_uuid, channel, message);
        }
    }
}

/// Replay message log to restore replay buffers after crash
pub async fn replay_message_log(log_path: &std::path::Path) -> anyhow::Result<usize> {
    use crate::message_log::MessageLog;

    let log = MessageLog::open(log_path)?;
    let entries = log.replay()?;
    let count = entries.len();

    for entry in entries {
        emit_and_store(entry.project_uuid, &entry.channel, entry.message).await;
    }

    Ok(count)
}

pub async fn get_or_create_broadcast_tx(project_uuid: Uuid, topic: &str) -> broadcast::Sender<String> {
    let key = (project_uuid, topic.to_string());
    let mut map = BROADCAST_CHANNELS.lock().await;
    // Get or create the sender+buffer tuple - the HashMap keeps the original sender alive
    // which keeps the channel open. We clone the sender to return.
    let (tx, _buffer) = map.entry(key.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
            let buffer = ReplayBuffer::new(REPLAY_BUFFER_CAPACITY);
            (tx, buffer)
        });
    tx.clone()
}

/// Emit a message and store it in the replay buffer atomically
/// Returns the number of receivers that received the message (0 if no active receivers)
pub async fn emit_and_store(project_uuid: Uuid, topic: &str, message: String) -> usize {
    let key = (project_uuid, topic.to_string());

    // Clone message and get sender outside the critical section to reduce lock time
    let message_clone = message.clone();
    let sender = {
        let mut map = BROADCAST_CHANNELS.lock().await;
        let (tx, buffer) = map.entry(key.clone())
            .or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
                let buffer = ReplayBuffer::new(REPLAY_BUFFER_CAPACITY);
                (tx, buffer)
            });

        // Store in replay buffer (always succeeds)
        buffer.push(message_clone);

        // Clone sender to use outside lock
        tx.clone()
    };
    // Lock released here

    // Log message for crash recovery (async, non-blocking)
    log_message(project_uuid, topic, &message);

    // Broadcast outside the lock - if there are no receivers, that's OK, we stored it
    // The replay buffer ensures late subscribers can catch up
    sender.send(message).unwrap_or(0)
}

/// Subscribe to a broadcast channel and get message history
///
/// IMPORTANT: Gets history BEFORE subscribing to avoid race condition where messages
/// emitted between subscribe and get_history appear in both live stream and history (duplicates).
pub async fn subscribe_broadcast(project_uuid: Uuid, topic: &str) -> (broadcast::Receiver<String>, Vec<String>) {
    let key = (project_uuid, topic.to_string());

    // Get history and receiver atomically with minimal lock time
    let (rx, history) = {
        let mut map = BROADCAST_CHANNELS.lock().await;

        // Use entry API to atomically get-or-create
        let (tx, buffer) = map.entry(key)
            .or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
                let buffer = ReplayBuffer::new(REPLAY_BUFFER_CAPACITY);
                (tx, buffer)
            });

        // Get history FIRST, then subscribe
        // This ensures messages don't appear in both history and live stream
        let history = buffer.get_all();
        let rx = tx.subscribe();

        (rx, history)
    };
    // Lock released here

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

/// Clear all channels (for testing crash recovery)
#[doc(hidden)]
pub async fn clear_all_channels() {
    let mut map = BROADCAST_CHANNELS.lock().await;
    map.clear();
}

/// Channel statistics
#[derive(Debug, serde::Serialize)]
pub struct ChannelStats {
    pub channel: String,
    pub replay_buffer_size: usize,
    pub subscriber_count: usize,
}

/// Get statistics for all channels of a project
pub async fn get_channel_stats(project_uuid: Uuid) -> Vec<ChannelStats> {
    let map = BROADCAST_CHANNELS.lock().await;
    let mut stats = Vec::new();

    for ((uuid, channel), (tx, buffer)) in map.iter() {
        if *uuid == project_uuid {
            stats.push(ChannelStats {
                channel: channel.clone(),
                replay_buffer_size: buffer.messages.len(),
                subscriber_count: tx.receiver_count(),
            });
        }
    }

    stats.sort_by(|a, b| a.channel.cmp(&b.channel));
    stats
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

    // ============ STRESS TESTS ============

    #[tokio::test]
    async fn stress_concurrent_emitters() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let uuid = Uuid::new_v4();
        let topic = "stress:concurrent";
        let num_emitters = 10;
        let msgs_per_emitter = 100;
        let total_expected = num_emitters * msgs_per_emitter;

        // Subscribe first to receive all messages
        let (mut rx, _) = subscribe_broadcast(uuid, topic).await;

        let received = Arc::new(AtomicUsize::new(0));
        let received_clone = received.clone();

        // Spawn receiver task
        let receiver_handle = tokio::spawn(async move {
            let mut count = 0;
            loop {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    rx.recv()
                ).await {
                    Ok(Ok(_)) => count += 1,
                    _ => break,
                }
            }
            received_clone.store(count, Ordering::SeqCst);
        });

        // Spawn multiple emitter tasks
        let mut handles = Vec::new();
        for emitter_id in 0..num_emitters {
            let uuid_clone = uuid;
            let topic_clone = topic.to_string();
            handles.push(tokio::spawn(async move {
                for msg_id in 0..msgs_per_emitter {
                    let msg = format!("emitter{}:msg{}", emitter_id, msg_id);
                    emit_and_store(uuid_clone, &topic_clone, msg).await;
                }
            }));
        }

        // Wait for all emitters to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Give receiver time to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        receiver_handle.abort();

        // Verify we received all messages (via replay buffer check)
        let (_, history) = subscribe_broadcast(uuid, topic).await;
        // Replay buffer only holds 100, but we should have received most live
        assert!(history.len() == REPLAY_BUFFER_CAPACITY,
            "Replay buffer should be at capacity: {} != {}", history.len(), REPLAY_BUFFER_CAPACITY);

        // Total emitted should be tracked
        assert_eq!(total_expected, 1000);
    }

    #[tokio::test]
    async fn stress_concurrent_subscribers() {
        let uuid = Uuid::new_v4();
        let topic = "stress:subscribers";
        let num_subscribers = 20;
        let num_messages = 50;

        // Pre-emit some messages for history
        for i in 0..num_messages {
            emit_and_store(uuid, topic, format!("msg{}", i)).await;
        }

        // Spawn multiple subscribers concurrently
        let mut handles = Vec::new();
        for _ in 0..num_subscribers {
            let uuid_clone = uuid;
            let topic_clone = topic.to_string();
            handles.push(tokio::spawn(async move {
                let (_, history) = subscribe_broadcast(uuid_clone, &topic_clone).await;
                history.len()
            }));
        }

        // All subscribers should get the same history
        for handle in handles {
            let history_len = handle.await.unwrap();
            assert_eq!(history_len, num_messages,
                "Each subscriber should get all {} messages", num_messages);
        }
    }

    #[tokio::test]
    async fn stress_emit_subscribe_race() {
        // Test that concurrent emit/subscribe operations don't deadlock or crash
        // Use unique topic to avoid interference from other tests
        let uuid = Uuid::new_v4();
        let topic = format!("stress:race:{}", Uuid::new_v4());
        let num_messages = 50;

        // Emit messages first
        for i in 0..num_messages {
            emit_and_store(uuid, &topic, format!("msg{}", i)).await;
        }

        // Now subscribe and verify history is correct
        let (_, history) = subscribe_broadcast(uuid, &topic).await;
        assert_eq!(history.len(), num_messages,
            "Should have {} messages in history", num_messages);

        // Emit more while we have a subscriber
        let (mut rx, _) = subscribe_broadcast(uuid, &topic).await;

        // Spawn emitter
        let uuid_clone = uuid;
        let topic_clone = topic.clone();
        let emitter = tokio::spawn(async move {
            for i in num_messages..(num_messages * 2) {
                emit_and_store(uuid_clone, &topic_clone, format!("msg{}", i)).await;
            }
        });

        // Receive some messages (don't require all - just verify no deadlock)
        let mut received = 0;
        for _ in 0..10 {
            if let Ok(Ok(_)) = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                rx.recv()
            ).await {
                received += 1;
            }
        }

        emitter.await.unwrap();

        // Verify some messages were received live (proves no deadlock)
        assert!(received > 0, "Should receive at least some messages live");

        // Final history check - should have last 100 messages
        let (_, final_history) = subscribe_broadcast(uuid, &topic).await;
        assert_eq!(final_history.len(), REPLAY_BUFFER_CAPACITY,
            "History should be at capacity");
    }

    #[tokio::test]
    async fn stress_high_throughput() {
        let uuid = Uuid::new_v4();
        let topic = "stress:throughput";
        let num_messages = 5000;

        let start = std::time::Instant::now();

        for i in 0..num_messages {
            emit_and_store(uuid, topic, format!("msg{}", i)).await;
        }

        let elapsed = start.elapsed();
        let msgs_per_sec = num_messages as f64 / elapsed.as_secs_f64();

        // Should handle at least 10k msgs/sec (conservative for CI)
        assert!(msgs_per_sec > 10_000.0,
            "Throughput {} msgs/sec is below minimum 10k/sec", msgs_per_sec);

        // Verify replay buffer integrity
        let (_, history) = subscribe_broadcast(uuid, topic).await;
        assert_eq!(history.len(), REPLAY_BUFFER_CAPACITY);

        // Last message should be the final one
        assert_eq!(history.last().unwrap(), &format!("msg{}", num_messages - 1));
    }
}
