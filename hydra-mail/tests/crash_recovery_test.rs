use anyhow::Result;
use hydra_mail::channels;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_crash_recovery_restores_messages() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let log_path = temp_dir.path().join("messages.log");

    // Simulate daemon session 1: emit messages with logging enabled
    let project_uuid = Uuid::new_v4();
    channels::set_message_log_path(Some(log_path.clone()));

    channels::emit_and_store(project_uuid, "test:channel", "msg1".to_string()).await;
    channels::emit_and_store(project_uuid, "test:channel", "msg2".to_string()).await;
    channels::emit_and_store(project_uuid, "test:channel", "msg3".to_string()).await;

    // Give async logging time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify log file exists
    assert!(log_path.exists(), "Log file should exist");

    // Simulate daemon crash and restart: clear in-memory state
    channels::set_message_log_path(None);
    channels::clear_all_channels().await;

    // Replay log to restore state
    let restored_count = channels::replay_message_log(&log_path).await?;
    assert_eq!(restored_count, 3, "Should restore 3 messages");

    // Verify replay buffer has messages
    let (_rx, history) = channels::subscribe_broadcast(project_uuid, "test:channel").await;
    assert_eq!(history.len(), 3, "Replay buffer should have 3 messages");
    assert_eq!(history[0], "msg1");
    assert_eq!(history[1], "msg2");
    assert_eq!(history[2], "msg3");

    Ok(())
}

#[tokio::test]
async fn test_log_compaction() -> Result<()> {
    use hydra_mail::message_log::MessageLog;

    let temp_dir = TempDir::new()?;
    let log_path = temp_dir.path().join("messages.log");

    let project_uuid = Uuid::new_v4();

    // Write 150 messages to log
    {
        let mut log = MessageLog::open(&log_path)?;
        for i in 0..150 {
            log.append(project_uuid, "test:channel", &format!("msg{}", i))?;
        }
    }

    // Compact to keep only last 100
    {
        let log = MessageLog::open(&log_path)?;
        log.compact(100)?;
    }

    // Verify only 100 remain
    {
        let log = MessageLog::open(&log_path)?;
        let entries = log.replay()?;
        assert_eq!(entries.len(), 100, "Should keep only 100 messages after compaction");
        assert_eq!(entries[0].message, "msg50", "First message should be msg50 (50-149)");
        assert_eq!(entries[99].message, "msg149", "Last message should be msg149");
    }

    Ok(())
}
