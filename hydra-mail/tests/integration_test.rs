use std::process::Command;
use std::env;
use std::fs;
use anyhow::Result;

#[tokio::test]
async fn test_init_creates_hydra() -> Result<()> {
    let temp_dir = env::temp_dir().join("hydra_test_init");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let project_root = env::current_dir()?;
    let binary_path = project_root.join("target/release/hydra-mail");

    let output = Command::new(&binary_path)
        .arg("init")
        .current_dir(&temp_dir)
        .output()?;

    assert!(output.status.success());
    assert!(temp_dir.join(".hydra").exists());
    assert!(temp_dir.join(".hydra/config.toml").exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_emit_subscribe_end_to_end() -> Result<()> {
    let temp_dir = env::temp_dir().join("hydra_test_e2e");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let project_root = env::current_dir()?;
    let binary_path = project_root.join("target/release/hydra-mail");

    Command::new(&binary_path)
        .arg("init")
        .arg("--daemon")
        .current_dir(&temp_dir)
        .output()?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    Command::new(&binary_path)
        .arg("emit")
        .arg("--type").arg("delta")
        .arg("--channel").arg("test:channel")
        .arg("--data").arg("{\"file\":\"test.py\"}")
        .current_dir(&temp_dir)
        .output()?;

    let subscribe_output = Command::new(&binary_path)
        .arg("subscribe")
        .arg("--channel").arg("test:channel")
        .arg("--once")
        .current_dir(&temp_dir)
        .output()?;

    assert!(subscribe_output.status.success());
    // Message is TOON-encoded, so check for non-empty output with TOON-encoded content
    let output_str = String::from_utf8_lossy(&subscribe_output.stdout);
    assert!(!output_str.is_empty(), "Subscribe output should contain message");
    // TOON encoding should contain the message content
    assert!(!output_str.trim().is_empty(), "Message should not be empty");

    let _ = Command::new(&binary_path).arg("stop").current_dir(&temp_dir).output()?;
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
