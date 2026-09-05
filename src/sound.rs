use std::{
    io::{self, Write},
    process::{Child, Command, Stdio},
};

use crate::config::{SOUNDS, Settings};

/// Child processes are polled, never awaited on the UI thread while playing.
#[derive(Default)]
pub struct Alerts {
    player: Option<Child>,
    notification: Option<Child>,
    ready: bool,
    muted: bool,
    next_ms: u64,
}

impl Alerts {
    pub fn update(&mut self, ready: bool, now_ms: u64, settings: &Settings) {
        self.reap();
        if !ready {
            if self.ready {
                self.stop();
            }
            self.ready = false;
            self.muted = false;
            return;
        }
        if !self.ready {
            self.ready = true;
            self.next_ms = now_ms;
            if settings.alert != "Off" {
                self.notify();
            }
        }
        if !self.muted && settings.alert != "Off" && now_ms >= self.next_ms {
            self.preview(&settings.sound);
            self.next_ms = now_ms.saturating_add(1_500);
            if settings.alert != "Repeat" {
                self.muted = true;
            }
        }
    }

    pub fn silence(&mut self) {
        self.muted = true;
        self.stop();
    }

    pub fn preview(&mut self, sound: &str) {
        stop_child(&mut self.player);
        if cfg!(target_os = "macos") && SOUNDS.contains(&sound) && sound != "Bell" {
            let path = format!("/System/Library/Sounds/{sound}.aiff");
            if let Ok(child) = Command::new("/usr/bin/afplay")
                .args(["--volume", "0.7", &path])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                self.player = Some(child);
                return;
            }
        }
        bell();
    }

    fn notify(&mut self) {
        if cfg!(target_os = "macos") {
            stop_child(&mut self.notification);
            self.notification = Command::new("/usr/bin/osascript")
                .args(["-e", "display notification \"Break complete. Press Space in pomo when ready.\" with title \"pomo\""])
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
        }
    }

    fn reap(&mut self) {
        if let Some(child) = &mut self.player {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        bell();
                    }
                    self.player = None;
                }
                Err(_) => {
                    stop_child(&mut self.player);
                    bell();
                }
                _ => {}
            }
        }
        if let Some(child) = &mut self.notification
            && matches!(child.try_wait(), Ok(Some(_)) | Err(_))
        {
            stop_child(&mut self.notification);
        }
    }

    fn stop(&mut self) {
        stop_child(&mut self.player);
        stop_child(&mut self.notification);
    }
}

fn bell() {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(b"\x07");
    let _ = stdout.flush();
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(mut child) = child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for Alerts {
    fn drop(&mut self) {
        self.stop();
    }
}
