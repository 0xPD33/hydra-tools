use std::process::Command;
use std::env;
use std::fs;
use std::path::Path;
use anyhow::Result;

#[tokio::test]
async fn test_init_creates_hydra() -> Result<()> {
    let temp_dir = env::temp_dir().join("hydra_test_init");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    
    // Build first, then run the binary directly
    let build_output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .output()?;
    
    if !build_output.status.success() {
        eprintln!("Build failed: {}", String::from_utf8_lossy(&build_output.stderr));
        return Err(anyhow::anyhow!("Build failed"));
    }
    
    let project_root = env::current_dir()?;
    let binary_path = project_root.join("target/release/hydra-mail");
    if !binary_path.exists() {
        return Err(anyhow::anyhow!("Binary not found at {:?}", binary_path));
    }
    
    let output = Command::new(&binary_path)
        .arg("init")
        .current_dir(&temp_dir)
        .output()?;
    
    assert!(output.status.success(), "Init failed: stdout: {}, stderr: {}", 
            String::from_utf8_lossy(&output.stdout), 
            String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hydra initialized"));
    
    let hydra_dir = temp_dir.join(".hydra");
    assert!(hydra_dir.exists());
    let config_path = hydra_dir.join("config.toml");
    assert!(config_path.exists());
    
    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_init_daemon_spawns_pid() -> Result<()> {
    let temp_dir = env::temp_dir().join("hydra_test_daemon");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    
    let project_root = env::current_dir()?;
    let binary_path = project_root.join("target/release/hydra-mail");
    let output = Command::new(&binary_path)
        .arg("init")
        .arg("--daemon")
        .current_dir(&temp_dir)
        .output()?;
    
    assert!(output.status.success(), "Init --daemon failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Daemon spawned with PID"));
    
    let pid_path = temp_dir.join(".hydra").join("daemon.pid");
    assert!(pid_path.exists());
    let pid_str = fs::read_to_string(pid_path)?;
    let pid: u32 = pid_str.trim().parse()?;
    assert!(pid > 0);
    
    // Cleanup (kill if needed, but for test, just remove dir)
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
    
    // 1. Initialize project
    let init_output = Command::new(&binary_path)
        .arg("init")
        .arg("--daemon")
        .current_dir(&temp_dir)
        .output()?;
    
    assert!(init_output.status.success(), "Init failed: {}", String::from_utf8_lossy(&init_output.stderr));
    
    // Give daemon a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // 2. Emit a message
    let emit_output = Command::new(&binary_path)
        .arg("emit")
        .arg("--type")
        .arg("delta")
        .arg("--channel")
        .arg("test:channel")
        .arg("--data")
        .arg("{\"file\":\"test.py\"}")
        .current_dir(&temp_dir)
        .output()?;
    
    assert!(emit_output.status.success(), "Emit failed: stdout: {}, stderr: {}", 
            String::from_utf8_lossy(&emit_output.stdout),
            String::from_utf8_lossy(&emit_output.stderr));
    
    let emit_stdout = String::from_utf8_lossy(&emit_output.stdout);
    assert!(emit_stdout.contains("Emit successful"), "Expected 'Emit successful', got: {}", emit_stdout);
    
    // 3. Subscribe and get one message
    let subscribe_output = Command::new(&binary_path)
        .arg("subscribe")
        .arg("--channel")
        .arg("test:channel")
        .arg("--once")
        .current_dir(&temp_dir)
        .output()?;
    
    // Subscribe should succeed and receive the message
    assert!(subscribe_output.status.success(), "Subscribe failed: stdout: {}, stderr: {}", 
            String::from_utf8_lossy(&subscribe_output.stdout),
            String::from_utf8_lossy(&subscribe_output.stderr));
    
    let subscribe_stdout = String::from_utf8_lossy(&subscribe_output.stdout);
    assert!(subscribe_stdout.contains("test.py"), "Expected message content, got: {}", subscribe_stdout);
    
    // 4. Stop daemon
    let _ = Command::new(&binary_path)
        .arg("stop")
        .current_dir(&temp_dir)
        .output()?;
    
    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
