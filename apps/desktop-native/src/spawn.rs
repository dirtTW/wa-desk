//! Process spawning for the external tools (`ffmpeg`, `ffprobe`).
//!
//! A GUI process that starts a child without `CREATE_NO_WINDOW` gets a console
//! window flashed on screen for every clip; every spawn in the app goes through
//! [`quiet`] so playback stays invisible.

use std::process::Command;

/// A `Command` that never opens a console window on Windows.
pub fn quiet(program: impl AsRef<std::ffi::OsStr>) -> Command {
    // `mut` only exists on Windows: on Linux the command is returned untouched,
    // and a dangling `mut` is a `-D warnings` error there (CI caught it).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`: the child runs without a console.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

#[cfg(test)]
mod tests {
    /// The helper must still run a program and read its output.
    #[test]
    fn quiet_runs_a_process_and_keeps_stdout() {
        #[cfg(windows)]
        let output = super::quiet("cmd")
            .args(["/C", "echo", "hello"])
            .output()
            .expect("cmd");
        #[cfg(not(windows))]
        let output = super::quiet("echo").arg("hello").output().expect("echo");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
    }
}
