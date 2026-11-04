//! Audio and video recording through the system's `ffmpeg` (PulseAudio / PipeWire for the
//! microphone, Video4Linux for a camera): NoteCraft has no audio stack of its own. The
//! recording goes to a private temporary file; stopping asks ffmpeg to finish it (`q` on its
//! input), reads the file and removes it.

use std::io::Write;
use std::process::{Child, Command, Stdio};

pub struct Recording {
    child: Child,
    file: std::path::PathBuf,
    pub video: bool,
}

impl Recording {
    pub fn start(video: bool) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!("notecraft-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join(if video { "video.mkv" } else { "audio.ogg" });
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-y", "-loglevel", "error"]);
        if video {
            cmd.args(["-f", "v4l2", "-i", "/dev/video0"]);
        }
        cmd.args(["-f", "pulse", "-i", "default"]);
        if video {
            cmd.args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "libopus"]);
        } else {
            cmd.args(["-c:a", "libopus"]);
        }
        let child = cmd
            .arg(&file)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("couldn't start ffmpeg ({e}): install it to record {}", if video { "video" } else { "audio" }))?;
        Ok(Recording { child, file, video })
    }

    /// Finish the recording and return the file's bytes.
    pub fn stop(mut self) -> Result<Vec<u8>, String> {
        if let Some(mut input) = self.child.stdin.take() {
            let _ = input.write_all(b"q");
        }
        let status = self.child.wait().map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&self.file);
        let _ = std::fs::remove_file(&self.file);
        let bytes = bytes.map_err(|_| "Nothing was recorded (no microphone or camera?).".to_string())?;
        if bytes.is_empty() || !status.success() && bytes.len() < 64 {
            return Err("Nothing was recorded (no microphone or camera?).".into());
        }
        Ok(bytes)
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        // Quitting mid-recording: don't leave ffmpeg running.
        let _ = self.child.kill();
        let _ = std::fs::remove_file(&self.file);
    }
}
