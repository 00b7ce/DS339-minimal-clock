use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use eframe::egui;

use crate::clock::ClockDisplay;
use crate::renderer::ClockStyle;

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([520.0, 430.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "JONSBO DS339 Minimal",
        options,
        Box::new(|_| Ok(Box::new(SettingsApp::default()))),
    )
    .map_err(|error| error.to_string())
}

struct SettingsApp {
    background_path: String,
    background_color: [u8; 3],
    font_family: String,
    font_size: f32,
    text_color: [u8; 3],
    centered: bool,
    position_x: i32,
    position_y: i32,
    status: String,
    worker: Option<DisplayWorker>,
    events: Option<Receiver<WorkerEvent>>,
}

impl Default for SettingsApp {
    fn default() -> Self {
        let style = ClockStyle::default();
        Self {
            background_path: String::new(),
            background_color: style.background_color,
            font_family: String::new(),
            font_size: style.font_size,
            text_color: style.text_color,
            centered: true,
            position_x: 0,
            position_y: 0,
            status: "Stopped".to_owned(),
            worker: None,
            events: None,
        }
    }
}

impl SettingsApp {
    fn style(&self) -> ClockStyle {
        ClockStyle {
            background_path: non_empty(&self.background_path).map(PathBuf::from),
            background_color: self.background_color,
            font_family: non_empty(&self.font_family).map(str::to_owned),
            font_size: self.font_size,
            text_color: self.text_color,
            position: (!self.centered).then_some((self.position_x, self.position_y)),
        }
    }

    fn start(&mut self) {
        self.stop();
        let (worker, events) = DisplayWorker::start(self.style());
        self.worker = Some(worker);
        self.events = Some(events);
        self.status = "Connecting...".to_owned();
    }

    fn stop(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        self.events = None;
    }

    fn receive_events(&mut self, context: &egui::Context) {
        let Some(events) = &self.events else {
            return;
        };
        let mut finished = false;
        while let Ok(event) = events.try_recv() {
            self.status = match event {
                WorkerEvent::Displayed(time) => format!("Running - {time}"),
                WorkerEvent::Stopped => {
                    finished = true;
                    "Stopped".to_owned()
                }
                WorkerEvent::Failed(error) => {
                    finished = true;
                    format!("Error: {error}")
                }
            };
            context.request_repaint();
        }
        if finished {
            if let Some(mut worker) = self.worker.take() {
                worker.stop();
            }
            self.events = None;
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_events(root_ui.ctx());
        if self.worker.is_some() {
            root_ui
                .ctx()
                .request_repaint_after(Duration::from_millis(250));
        }

        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.heading("JONSBO DS339 Clock");
            ui.add_space(8.0);

            egui::Grid::new("settings")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Background image");
                    ui.text_edit_singleline(&mut self.background_path)
                        .on_hover_text("Leave blank to use the background color");
                    ui.end_row();

                    ui.label("Background color");
                    egui::color_picker::color_edit_button_srgb(ui, &mut self.background_color);
                    ui.end_row();

                    ui.label("Font family");
                    ui.text_edit_singleline(&mut self.font_family)
                        .on_hover_text("Leave blank to use the system sans-serif font");
                    ui.end_row();

                    ui.label("Font size");
                    ui.add(egui::DragValue::new(&mut self.font_size).range(8.0..=300.0));
                    ui.end_row();

                    ui.label("Text color");
                    egui::color_picker::color_edit_button_srgb(ui, &mut self.text_color);
                    ui.end_row();

                    ui.label("Placement");
                    ui.checkbox(&mut self.centered, "Center");
                    ui.end_row();

                    if !self.centered {
                        ui.label("Position (x, y)");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut self.position_x));
                            ui.add(egui::DragValue::new(&mut self.position_y));
                        });
                        ui.end_row();
                    }
                });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Start / Apply").clicked() {
                    self.start();
                }
                if ui
                    .add_enabled(self.worker.is_some(), egui::Button::new("Stop"))
                    .clicked()
                {
                    self.stop();
                    self.status = "Stopped".to_owned();
                }
            });
            ui.add_space(12.0);
            ui.label(&self.status);
        });
    }
}

impl Drop for SettingsApp {
    fn drop(&mut self) {
        self.stop();
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

enum WorkerEvent {
    Displayed(String),
    Stopped,
    Failed(String),
}

struct DisplayWorker {
    stop_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DisplayWorker {
    fn start(style: ClockStyle) -> (Self, Receiver<WorkerEvent>) {
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop_requested);
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut display = match ClockDisplay::open(style) {
                Ok(display) => display,
                Err(error) => {
                    let _ = sender.send(WorkerEvent::Failed(error));
                    return;
                }
            };

            while !worker_stop.load(Ordering::SeqCst) {
                match display.update() {
                    Ok(time) => {
                        if sender.send(WorkerEvent::Displayed(time)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(WorkerEvent::Failed(error));
                        return;
                    }
                }

                for _ in 0..10 {
                    if worker_stop.load(Ordering::SeqCst) {
                        let _ = sender.send(WorkerEvent::Stopped);
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            let _ = sender.send(WorkerEvent::Stopped);
        });

        (
            Self {
                stop_requested,
                thread: Some(thread),
            },
            receiver,
        )
    }

    fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_optional_text_settings() {
        assert_eq!(non_empty("  Segoe UI "), Some("Segoe UI"));
        assert_eq!(non_empty("   "), None);
    }
}
