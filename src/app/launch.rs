//! A macOS connection launch returns the terminal to its caller. The GUI
//! runs in its own OS session, so Ctrl-C and closing the caller's terminal
//! cannot take its connections down with it. Plain development launches
//! keep their foreground logs; an already-running app is contacted first.

use std::{
    ffi::OsString,
    io,
    os::unix::process::CommandExt as _,
    process::{Child, Command, Stdio},
};

const BACKGROUND_ARG: &str = "--shellrs-app-process";

/// Consume the private re-execution marker before interpreting user args.
/// It is an argument rather than an environment variable so local shells
/// do not inherit it and bypass the next background launch.
pub fn take_background_launch(args: &mut Vec<OsString>) -> bool {
    if args.first().is_some_and(|arg| arg == BACKGROUND_ARG) {
        args.remove(0);
        true
    } else {
        false
    }
}

/// Re-execute the app independently. The caller exits after this succeeds;
/// the child consumes its marker and continues normal GUI startup.
pub fn launch_in_background(args: &[OsString]) -> io::Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(BACKGROUND_ARG).args(args);
    // The launcher exits immediately; its child is adopted by the OS.
    spawn_independent(&mut command)?;
    Ok(())
}

fn spawn_independent(command: &mut Command) -> io::Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: after fork the hook only calls async-signal-safe setsid and
    // reads errno on failure. It does not allocate, lock or access env vars.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::{BACKGROUND_ARG, spawn_independent, take_background_launch};
    use std::{
        fs,
        io::IsTerminal as _,
        os::unix::process::{CommandExt as _, ExitStatusExt as _},
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    const PROBE: &str = "app::launch::tests::independent_launch_probe";
    const MODE: &str = "SHELLRS_TEST_LAUNCH_MODE";
    const DIRECTORY: &str = "SHELLRS_TEST_LAUNCH_DIRECTORY";

    fn wait_for(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !path.exists() {
            assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn probe(mode: &str, directory: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", PROBE, "--nocapture"])
            .env(MODE, mode)
            .env(DIRECTORY, directory);
        command
    }

    // Invoked in separate processes by the regression test below.
    #[test]
    fn independent_launch_probe() {
        let Ok(mode) = std::env::var(MODE) else {
            return;
        };
        let directory = std::env::var_os(DIRECTORY).unwrap();
        let directory = Path::new(&directory);
        if mode == "launcher" {
            let mut command = probe("app", directory);
            let mut app = spawn_independent(&mut command).unwrap();
            // Keep the caller alive until its process group is stopped.
            wait_for(&directory.join("caller-finished"));
            app.wait().unwrap();
        } else {
            // SAFETY: these only read this process's identifiers.
            unsafe {
                assert_eq!(libc::getsid(0), libc::getpid());
                assert_eq!(libc::getpgrp(), libc::getpid());
            }
            assert!(!std::io::stdin().is_terminal());
            assert!(!std::io::stdout().is_terminal());
            assert!(!std::io::stderr().is_terminal());
            let mut args = vec![BACKGROUND_ARG.into(), "生产 Web".into()];
            assert!(take_background_launch(&mut args));
            assert_eq!(args, vec![std::ffi::OsString::from("生产 Web")]);
            assert!(!take_background_launch(&mut args));
            fs::write(directory.join("app-ready"), b"").unwrap();
            wait_for(&directory.join("caller-stopped"));
            fs::write(directory.join("app-survived"), b"").unwrap();
        }
    }

    #[test]
    fn independent_launch_survives_the_callers_interrupt_and_hangup() {
        for signal in [libc::SIGINT, libc::SIGHUP] {
            let directory = tempfile::tempdir().unwrap();
            let mut command = probe("launcher", directory.path());
            let mut launcher = command
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            // Always reap this test's launcher, even if the probe fails.
            let ready = directory.path().join("app-ready");
            let deadline = Instant::now() + Duration::from_secs(10);
            while !ready.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if !ready.exists() {
                launcher.kill().ok();
                launcher.wait().ok();
                panic!("independent app failed to start");
            }
            // SAFETY: this is the separate process group created above,
            // never the test runner's or the user's terminal's group.
            assert_eq!(unsafe { libc::kill(-(launcher.id() as i32), signal) }, 0);
            let output = launcher.wait_with_output().unwrap();
            assert_eq!(output.status.signal(), Some(signal));
            // wait_with_output also proves the app released the caller's pipes.
            fs::write(directory.path().join("caller-stopped"), b"").unwrap();
            wait_for(&directory.path().join("app-survived"));
        }
    }
}
