use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Default PID file location
const PID_FILE_NAME: &str = "chico.pid";

/// Get the path to the PID file
pub fn get_pid_file_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(PID_FILE_NAME);
    path
}

/// Write the process ID to the PID file
#[cfg(any(windows, test))]
pub fn write_pid_file(pid: u32) -> io::Result<()> {
    let path = get_pid_file_path();
    fs::write(path, pid.to_string())
}

/// Read the process ID from the PID file
pub fn read_pid_file() -> io::Result<u32> {
    let path = get_pid_file_path();
    let content = fs::read_to_string(path)?;
    content.trim().parse::<u32>().map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid PID format: {}", e))
    })
}

/// Remove the PID file
pub fn remove_pid_file() -> io::Result<()> {
    let path = get_pid_file_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Check if a process with the given PID is running
#[cfg(unix)]
pub fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    
    Command::new("tasklist")
        .arg("/FI")
        .arg(format!("PID eq {}", pid))
        .arg("/FO")
        .arg("CSV")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|output| {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.lines().count() > 1  // More than just the header line
        })
        .unwrap_or(false)
}

/// Start the server as a daemon process
pub fn start_daemon(config_path: &str) -> io::Result<u32> {
    // Check if daemon is already running
    if let Ok(existing_pid) = read_pid_file() {
        if is_process_running(existing_pid) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Daemon already running with PID {}", existing_pid),
            ));
        } else {
            // Clean up stale PID file
            let _ = remove_pid_file();
        }
    }

    #[cfg(unix)]
    {
        start_daemon_unix(config_path)
    }

    #[cfg(windows)]
    {
        start_daemon_windows(config_path)
    }
}

#[cfg(unix)]
fn start_daemon_unix(config_path: &str) -> io::Result<u32> {
    // Get current executable path
    let current_exe = std::env::current_exe()?;
    
    // Spawn a child process that will become the daemon
    let child = Command::new(&current_exe)
        .arg("run")
        .arg("--daemon-mode")
        .arg("--config")
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let child_pid = child.id();
    
    // Give the daemon process a moment to start and daemonize itself
    std::thread::sleep(std::time::Duration::from_millis(2000));
    
    // Check if the PID file was created by the daemon
    if let Ok(daemon_pid) = read_pid_file() {
        println!("Daemon started with PID {}", daemon_pid);
        Ok(daemon_pid)
    } else {
        // Check if the original child process is still running
        if is_process_running(child_pid) {
            println!("Daemon started with PID {}", child_pid);
            Ok(child_pid)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Daemon process failed to start"))
        }
    }
}

#[cfg(windows)]
fn start_daemon_windows(config_path: &str) -> io::Result<u32> {
    // Get current executable path
    let current_exe = std::env::current_exe()?;

    // Spawn the daemon process
    let child = Command::new(current_exe)
        .arg("run")
        .arg("--config")
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let pid = child.id();
    write_pid_file(pid)?;

    println!("Daemon started with PID {}", pid);
    Ok(pid)
}

/// Stop the daemon process
pub fn stop_daemon() -> io::Result<()> {
    let pid = read_pid_file().map_err(|_| {
        io::Error::new(io::ErrorKind::NotFound, "No daemon PID file found")
    })?;

    if !is_process_running(pid) {
        remove_pid_file()?;
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No daemon process running with PID {}", pid),
        ));
    }

    // Send termination signal
    #[cfg(unix)]
    {
        use std::process::Command;
        let status = Command::new("kill")
            .arg(pid.to_string())
            .status()?;

        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to terminate process {}", pid),
            ));
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        let status = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/F")
            .status()?;

        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to terminate process {}", pid),
            ));
        }
    }

    // Wait a moment for the process to terminate
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Clean up PID file
    remove_pid_file()?;

    println!("Daemon with PID {} stopped", pid);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pid_file_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Override the PID file path for testing
        std::env::set_var("TMPDIR", temp_dir.path());

        // Test writing PID
        let test_pid = 12345u32;
        write_pid_file(test_pid).unwrap();

        // Test reading PID
        let read_pid = read_pid_file().unwrap();
        assert_eq!(test_pid, read_pid);

        // Test removing PID file
        remove_pid_file().unwrap();

        // Verify file is removed
        assert!(read_pid_file().is_err());
    }

    #[test]
    fn test_is_process_running() {
        // Test with current process (should be running)
        let current_pid = std::process::id();
        assert!(is_process_running(current_pid));

        // Test with a non-existent PID (very unlikely to exist)
        assert!(!is_process_running(999999));
    }
}