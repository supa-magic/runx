use std::collections::HashMap;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Stores the PID of the currently running child process for signal forwarding.
/// Updated before each execution, read by the signal handler.
static CHILD_PID: AtomicU32 = AtomicU32::new(0);

/// Tracks whether the signal handler was successfully installed.
static SIGNAL_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Execute a command in the given environment.
///
/// Spawns the command as a child process with:
/// - A fully isolated (or inherited) environment from `env_vars`
/// - Inherited stdin/stdout/stderr for transparent I/O
/// - On Ctrl+C (SIGINT), sends SIGTERM to the child process on Unix
/// - On Windows, Ctrl+C is automatically forwarded to the child process group
///
/// Returns the child's exit status. Temp directories and other RAII guards
/// in the caller are cleaned up normally when this function returns.
///
/// **Note:** There is a narrow race window between `spawn()` and the PID being
/// stored. A signal arriving in that window is not forwarded to the child.
/// This is acceptable because the child inherits the process group and will
/// receive SIGINT directly from the terminal.
pub fn execute(
    program: &str,
    args: &[String],
    env_vars: &HashMap<String, String>,
) -> Result<ExitStatus, ExecutorError> {
    // Register the signal handler (retries if a previous attempt failed)
    install_signal_handler()?;

    let mut child = std::process::Command::new(program)
        .args(args)
        .env_clear()
        .envs(env_vars)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| ExecutorError::Spawn {
            program: program.to_string(),
            source: e,
        })?;

    // Store the child PID so the signal handler can forward signals.
    // There is a narrow window between spawn() and this store where a signal
    // would not be forwarded, but the child still receives SIGINT directly
    // from the terminal via its process group.
    CHILD_PID.store(child.id(), Ordering::Release);

    let status = child.wait().map_err(|e| ExecutorError::Wait {
        program: program.to_string(),
        source: e,
    })?;

    // Clear the PID after the child exits to prevent signaling a stale/reused PID
    CHILD_PID.store(0, Ordering::Release);

    Ok(status)
}

/// Install the global signal handler.
///
/// Uses an `AtomicBool` instead of `Once` so that if installation fails,
/// subsequent calls can retry rather than silently succeeding.
///
/// On Ctrl+C (SIGINT): sends SIGTERM to the child process on Unix.
/// On Windows: prevents the parent from exiting before the child
/// (Ctrl+C is forwarded to the child process group by the OS).
fn install_signal_handler() -> Result<(), ExecutorError> {
    if SIGNAL_HANDLER_INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }

    ctrlc::set_handler(|| {
        let pid = CHILD_PID.load(Ordering::Acquire);
        if pid != 0 {
            #[cfg(unix)]
            {
                // SAFETY: `kill` is async-signal-safe per POSIX. We send SIGTERM
                // to a PID we spawned. If the child has already exited, the signal
                // is harmlessly ignored (or hits an unrelated process in the
                // extremely unlikely case of PID reuse within the wait() window).
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
            }
        }
    })
    .map_err(|e| ExecutorError::SignalHandler {
        reason: e.to_string(),
    })?;

    SIGNAL_HANDLER_INSTALLED.store(true, Ordering::Release);
    Ok(())
}

/// Extract the numeric exit code from an `ExitStatus`.
///
/// Returns 0 for success, the exit code if available, or 1 as fallback.
/// On Unix, if the process was terminated by a signal, returns 128 + signal number.
pub fn exit_code(status: &ExitStatus) -> i32 {
    if status.success() {
        return 0;
    }

    // On Unix, check for signal termination
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }

    status.code().unwrap_or(1)
}

/// Errors that occur during command execution.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// Failed to spawn the child process.
    #[error("failed to execute `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    /// Failed to wait for the child process.
    #[error("failed to wait for `{program}`: {source}")]
    Wait {
        program: String,
        source: std::io::Error,
    },

    /// Failed to install signal handler.
    #[error("failed to install signal handler: {reason}")]
    SignalHandler { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_true_succeeds() {
        let env: HashMap<String, String> = HashMap::new();
        let status = execute("true", &[], &env).unwrap();
        assert!(status.success());
    }

    #[test]
    fn test_execute_false_fails() {
        let env: HashMap<String, String> = HashMap::new();
        let status = execute("false", &[], &env).unwrap();
        assert!(!status.success());
    }

    #[test]
    fn test_exit_code_success() {
        let env: HashMap<String, String> = HashMap::new();
        let status = execute("true", &[], &env).unwrap();
        assert_eq!(exit_code(&status), 0);
    }

    #[test]
    fn test_exit_code_failure() {
        let env: HashMap<String, String> = HashMap::new();
        let status = execute("false", &[], &env).unwrap();
        assert_eq!(exit_code(&status), 1);
    }

    #[test]
    fn test_execute_with_args() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        let status = execute("echo", &["hello".to_string()], &env).unwrap();
        assert!(status.success());
    }

    #[test]
    fn test_execute_with_custom_env() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env.insert("RUNX_TEST_VAR".to_string(), "hello".to_string());

        // Use sh to verify our custom var is set
        let status = execute(
            "sh",
            &[
                "-c".to_string(),
                "test \"$RUNX_TEST_VAR\" = \"hello\"".to_string(),
            ],
            &env,
        )
        .unwrap();
        assert!(status.success());
    }

    #[test]
    fn test_execute_env_clear_isolates() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

        // Verify HOME is NOT inherited from parent (env_clear)
        let status = execute(
            "sh",
            &["-c".to_string(), "test -z \"$HOME\"".to_string()],
            &env,
        )
        .unwrap();
        assert!(
            status.success(),
            "HOME should not be inherited when env_clear is used"
        );
    }

    #[test]
    fn test_execute_nonexistent_program() {
        let env: HashMap<String, String> = HashMap::new();
        let result = execute("nonexistent-program-that-does-not-exist", &[], &env);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExecutorError::Spawn { .. }));
    }

    #[test]
    fn test_execute_specific_exit_code() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

        let status = execute("sh", &["-c".to_string(), "exit 42".to_string()], &env).unwrap();
        assert_eq!(exit_code(&status), 42);
    }

    #[test]
    fn test_executor_error_display() {
        let err = ExecutorError::Spawn {
            program: "node".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        assert_eq!(err.to_string(), "failed to execute `node`: not found");

        let err = ExecutorError::Wait {
            program: "node".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::Other, "interrupted"),
        };
        assert_eq!(err.to_string(), "failed to wait for `node`: interrupted");

        let err = ExecutorError::SignalHandler {
            reason: "already set".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to install signal handler: already set"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_exit_code_from_signal() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

        // Kill self with SIGTERM (signal 15) → exit code should be 128 + 15 = 143
        let status = execute("sh", &["-c".to_string(), "kill -TERM $$".to_string()], &env).unwrap();
        assert_eq!(exit_code(&status), 128 + 15);
    }
}
