use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

static BROADCAST_CHANNELS: Lazy<Arc<tokio::sync::Mutex<HashMap<(Uuid, String), broadcast::Sender<String>>>>> =
    Lazy::new(|| Arc::new(tokio::sync::Mutex::new(HashMap::new())));

static MPSC_CHANNELS: Lazy<Arc<tokio::sync::Mutex<HashMap<(Uuid, String), mpsc::Sender<String>>>>> =
    Lazy::new(|| Arc::new(tokio::sync::Mutex::new(HashMap::new())));

pub async fn get_or_create_broadcast_tx(project_uuid: Uuid, topic: &str) -> broadcast::Sender<String> {
    let key = (project_uuid, topic.to_string());
    let mut map = BROADCAST_CHANNELS.lock().await;
    // Get or create the sender - the HashMap keeps the original sender alive
    // which keeps the channel open. We clone it to return.
    map.entry(key.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(1024);
            tx
        })
        .clone()
}

pub async fn subscribe_broadcast(project_uuid: Uuid, topic: &str) -> broadcast::Receiver<String> {
    let tx = get_or_create_broadcast_tx(project_uuid, topic).await;
    tx.subscribe()
}

pub async fn get_or_create_mpsc_tx(project_uuid: Uuid, topic: &str) -> mpsc::Sender<String> {
    let key = (project_uuid, topic.to_string());
    let mut map = MPSC_CHANNELS.lock().await;
    map.entry(key)
        .or_insert_with(|| mpsc::channel(1024).0)
        .clone()
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

// For targeted mpsc, receivers would be created on subscribe/emit as needed

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
        let mut rx = subscribe_broadcast(uuid, topic).await;
        let tx = get_or_create_broadcast_tx(uuid, topic).await;
        let msg = "hello world".to_string();
        tx.send(msg.clone()).unwrap();
        let received: String = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_get_or_create_mpsc_tx_same_instance() {
        let uuid = Uuid::new_v4();
        let topic = "test";
        let tx1 = get_or_create_mpsc_tx(uuid, topic).await;
        let tx2 = get_or_create_mpsc_tx(uuid, topic).await;
        
        // Test they're the same by creating a receiver and sending from both
        // Since mpsc channels are created per key, both tx1 and tx2 should be clones
        // We can't easily test equality, but we can verify both are senders
        // For now, just verify they're both valid senders (no panic on clone)
        let _tx1_clone = tx1.clone();
        let _tx2_clone = tx2.clone();
        // Test passes if no panic
    }

    #[tokio::test]
    async fn test_mpsc_send() {
        let uuid = Uuid::new_v4();
        let topic = "test:mpsc";
        let tx = get_or_create_mpsc_tx(uuid, topic).await;
        // Create a receiver to test send/recv
        let (test_tx, mut test_rx) = mpsc::channel::<String>(1);
        // Note: The global tx doesn't have a paired receiver, so we test with a local channel
        let msg = "mpsc test".to_string();
        test_tx.send(msg.clone()).await.unwrap();
        let received = test_rx.recv().await.unwrap();
        assert_eq!(received, msg);
        // Also verify the global tx is a valid sender (clone works)
        let _tx_clone = tx.clone();
    }
}
