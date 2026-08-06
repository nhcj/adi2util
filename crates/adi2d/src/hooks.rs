use core::fmt;
use std::{collections::HashMap, io::Read, os::{fd::AsRawFd, unix::process::CommandExt}, process::{Child, Command, Stdio}, sync::mpsc, thread, time::{Duration, Instant}};

use adi2::midi::{ChannelParameterType, DeviceParameterType, Output};
use log::error;
use strfmt::strfmt;

pub type HookMap<T> = HashMap<T, Vec<String>>;

#[derive(Clone)]
pub struct Hooks {
    active_output_hooks: HookMap<Output>,
    channel_hooks: HookMap<ChannelParameterType>,
    device_hooks: HookMap<DeviceParameterType>,
    tx: mpsc::Sender<RunHook>,
}

struct RunHook {
    parameter: String,
    command: String,
}

impl Hooks {
    pub fn create(
        active_output_hooks: HookMap<Output>,
        channel_hooks: HookMap<ChannelParameterType>,
        device_hooks: HookMap<DeviceParameterType>,
    ) -> Hooks {
        let (tx, rx) = mpsc::channel();
        Self::spawn_run_thread(rx);

        Hooks {
            active_output_hooks,
            channel_hooks,
            device_hooks,
            tx,
        }
    }

    // run hooks in a separate thread so they can't block the main
    // thread from handling more commands
    fn spawn_run_thread(rx: mpsc::Receiver<RunHook>) {
        thread::spawn(move || {
            while let Ok(hook) = rx.recv() {
                run_hook(hook);
            }
        });
    }

    pub fn run_active_output_hooks(&self, old: Option<Output>, new: Output) {
        if let Some(hooks) = self.active_output_hooks.get(&new) {
            let old = old.map(|o| o.to_string()).unwrap_or_else(|| "-".to_string());
            self.run_hooks(hooks, "ActiveOutput", old, new);
        }
    }

    pub fn run_channel_hooks(&self, parameter: ChannelParameterType, old: i16, new: i16) {
        if let Some(hooks) = self.channel_hooks.get(&parameter) {
            self.run_hooks(hooks, parameter, old, new);
        }
    }

    pub fn run_device_hooks(&self, parameter: DeviceParameterType, old: u16, new: u16) {
        if let Some(hooks) = self.device_hooks.get(&parameter) {
            self.run_hooks(hooks, parameter, old, new);
        }
    }

    pub fn run_hooks(&self, hooks: &[String], parameter: impl fmt::Display, old: impl fmt::Display, new: impl fmt::Display) {
        let mut values = HashMap::new();
        values.insert("old".to_string(), old.to_string());
        values.insert("new".to_string(), new.to_string());
        
        for hook in hooks {
            let command = match strfmt(hook, &values) {
                Ok(h) => h,
                Err(e) => {
                    error!("failed to format hook ({}) with values: {:?}", hook, e);
                    continue;
                }
            };
            let hook = RunHook {
                parameter: parameter.to_string(),
                command,
            };
            
            if self.tx.send(hook).is_err() {
                error!("hook runner is dead; failed to send hook for {}", parameter);
            }
        }
    }
}

fn run_hook(hook: RunHook) {
    let child = Command::new("sh")
        .arg("-c")
        .arg(&hook.command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn();
    let mut child = match child  {
        Ok(c) => c,
        Err(e) => {
            error!("error running hook for {} (`{}`): {:?}", hook.parameter, hook.command, e);
            return;
        }
    };

    let timeout = Instant::now() + Duration::from_secs(2);
    let poll_interval = Duration::from_millis(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let stderr = read_stderr_to_string(&mut child);
                    error!("hook for {} (`{}`) exited with {}: {}", hook.parameter, hook.command, status, stderr.trim());
                    return
                }
            }
            Ok(None) if Instant::now() >= timeout => {
                kill_process_group(&mut child);
                error!("hook for {} (`{}`) was killed after timing out", hook.parameter, hook.command);
                return
            }
            Ok(None) => thread::sleep(poll_interval),
            Err(e) => {
                kill_process_group(&mut child);
                error!("error running hook for {} (`{}`): {:?}", hook.parameter, hook.command, e);
                return
            }
        }
    }
}

fn read_stderr_to_string(child: &mut Child) -> String {
    let Some(mut stderr) = child.stderr.take() else {
        return String::new();
    };
    let fd = stderr.as_raw_fd();
    // make it nonblocking so another child process can't block the read
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags == -1 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
            return String::new();
        }
    }
    let mut err = Vec::new();
    let mut read_buffer = [0u8; 4096];
    // arbitrarily chosen size limit
    while err.len() < (1<<16) {
        match stderr.read(&mut read_buffer) {
            Ok(0) => break,
            Ok(n) => err.extend_from_slice(&read_buffer[..n]),
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&err).into_owned()
}

// kill the group since we run sh
fn kill_process_group(child: &mut Child) {
    let pgid = child.id() as i32;
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
    _ = child.wait();
}
