use chrono::Local;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::bulk;
use crate::ds339;
use crate::frame;
use crate::renderer::{ClockRenderer, ClockStyle};

pub struct ClockDisplay {
    _control: ds339::ControlConnection,
    video: bulk::VideoConnection,
    renderer: ClockRenderer,
}

pub struct ClockUpdate {
    pub time: String,
    pub logical_rgb: Vec<u8>,
}

pub struct FrameTicker {
    interval: Duration,
    next_deadline: Instant,
}

impl FrameTicker {
    pub fn new(interval: Duration) -> Self {
        Self::from_start(Instant::now(), interval)
    }

    fn from_start(start: Instant, interval: Duration) -> Self {
        Self {
            interval,
            next_deadline: start + interval,
        }
    }

    pub fn wait(&mut self, stop_requested: &AtomicBool) -> bool {
        let deadline = self.take_deadline();
        while !stop_requested.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }
            thread::sleep((deadline - now).min(Duration::from_millis(50)));
        }
        false
    }

    fn take_deadline(&mut self) -> Instant {
        let deadline = self.next_deadline;
        self.next_deadline += self.interval;
        deadline
    }
}

impl ClockDisplay {
    pub fn open(style: ClockStyle) -> Result<Self, String> {
        let control = ds339::ControlConnection::open().map_err(|error| error.to_string())?;
        let video = bulk::VideoConnection::open()?;
        let renderer = ClockRenderer::new(style)?;

        control
            .set_native_mode()
            .map_err(|error| error.to_string())?;

        Ok(Self {
            _control: control,
            video,
            renderer,
        })
    }

    pub fn update(&mut self) -> Result<ClockUpdate, String> {
        let text = Local::now().format("%H:%M:%S").to_string();
        let logical_rgb = self.renderer.render_logical(&text);
        let native_bgr = ClockRenderer::native_bgr(&logical_rgb)?;
        let encoded = frame::encode(&native_bgr)?;
        self.video.write_all(&encoded)?;
        Ok(ClockUpdate {
            time: text,
            logical_rgb,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_deadlines_are_anchored_to_the_start_time() {
        let start = Instant::now();
        let interval = Duration::from_secs(1);
        let mut ticker = FrameTicker::from_start(start, interval);

        assert_eq!(ticker.take_deadline(), start + interval);
        assert_eq!(ticker.take_deadline(), start + interval * 2);
        assert_eq!(ticker.take_deadline(), start + interval * 3);
    }
}
