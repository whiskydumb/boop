#[cfg(windows)]
mod job;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::features::apps::AppEntry;

struct DeployedStub {
    exe: PathBuf,
    created_dir: Option<PathBuf>,
}

struct RunningProcess {
    id: String,
    since: Instant,
    child: Child,
    stub: Option<DeployedStub>,
}

pub struct Launcher {
    running: Option<RunningProcess>,
    #[cfg(windows)]
    job: Option<job::JobObject>,
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Launcher {
    pub fn new() -> Self {
        Self {
            running: None,
            #[cfg(windows)]
            job: job::JobObject::new().ok(),
        }
    }

    pub fn running_id(&self) -> Option<&str> {
        self.running.as_ref().map(|process| process.id.as_str())
    }

    pub fn running_since(&self) -> Option<Instant> {
        self.running.as_ref().map(|process| process.since)
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running_id() == Some(id)
    }

    pub fn launch(&mut self, entry: &AppEntry, exe: &Path, cwd: Option<&Path>) -> Result<()> {
        self.kill();

        let stub = deploy_stub(exe)?;

        let mut command = Command::new(exe);
        command.args(&entry.args);
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                cleanup_stub(stub);
                return Err(error).with_context(|| format!("failed to launch {}", exe.display()));
            }
        };

        #[cfg(windows)]
        if let Some(job) = &self.job {
            let _ = job.assign(&child);
        }

        self.running = Some(RunningProcess {
            id: entry.id.clone(),
            since: Instant::now(),
            child,
            stub,
        });
        Ok(())
    }

    pub fn kill(&mut self) {
        if let Some(mut process) = self.running.take() {
            #[cfg(windows)]
            if let Some(job) = &self.job {
                let _ = job.terminate();
            }
            let _ = process.child.kill();
            let _ = process.child.wait();
            cleanup_stub(process.stub);
        }
    }

    pub fn poll(&mut self) -> bool {
        let exited = match &mut self.running {
            Some(process) => !matches!(process.child.try_wait(), Ok(None)),
            None => return false,
        };
        if exited && let Some(process) = self.running.take() {
            cleanup_stub(process.stub);
        }
        exited
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        self.kill();
    }
}

fn deploy_stub(exe: &Path) -> Result<Option<DeployedStub>> {
    if exe.exists() {
        return Ok(None);
    }

    let stub = stub_path().context("cannot locate the stub executable")?;
    if !stub.exists() {
        bail!(
            "stub executable not found at {} -- run `cargo build` first",
            stub.display()
        );
    }

    let created_dir = match exe.parent() {
        Some(parent) => {
            let highest = highest_missing_dir(parent);
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
            highest
        }
        None => None,
    };

    fs::copy(&stub, exe).with_context(|| format!("failed to write stub to {}", exe.display()))?;

    Ok(Some(DeployedStub {
        exe: exe.to_path_buf(),
        created_dir,
    }))
}

fn cleanup_stub(stub: Option<DeployedStub>) {
    let Some(stub) = stub else {
        return;
    };
    match stub.created_dir {
        Some(dir) => {
            let _ = fs::remove_dir_all(&dir);
        }
        None => {
            let _ = fs::remove_file(&stub.exe);
        }
    }
}

fn highest_missing_dir(dir: &Path) -> Option<PathBuf> {
    if dir.exists() {
        return None;
    }
    let mut highest = dir.to_path_buf();
    while let Some(parent) = highest.parent() {
        if parent.as_os_str().is_empty() || parent.exists() {
            break;
        }
        highest = parent.to_path_buf();
    }
    Some(highest)
}

fn stub_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot determine current executable path")?;
    let dir = exe.parent().context("executable has no parent directory")?;
    Ok(dir.join(format!("stub{}", std::env::consts::EXE_SUFFIX)))
}
