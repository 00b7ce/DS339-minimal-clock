use chrono::Local;

use crate::bulk;
use crate::ds339;
use crate::frame;
use crate::renderer::{ClockRenderer, ClockStyle};

pub struct ClockDisplay {
    _control: ds339::ControlConnection,
    video: bulk::VideoConnection,
    renderer: ClockRenderer,
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

    pub fn update(&mut self) -> Result<String, String> {
        let text = Local::now().format("%H:%M:%S").to_string();
        let pixels = self.renderer.render(&text);
        let encoded = frame::encode(&pixels)?;
        self.video.write_all(&encoded)?;
        Ok(text)
    }
}
