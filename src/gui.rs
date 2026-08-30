use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Local;
use eframe::egui;

use crate::clock::{ClockDisplay, FrameTicker};
use crate::profile::{ProfileRepository, ProfileStore};
use crate::renderer::{
    ClockRenderer, ClockStyle, LOGICAL_HEIGHT, LOGICAL_WIDTH, system_font_families,
};

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 650.0]),
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
    profile_repository: Option<ProfileRepository>,
    profile_store: ProfileStore,
    profile_name: String,
    profile_status: String,
    background_path: String,
    background_color: [u8; 3],
    font_family: String,
    font_families: Vec<String>,
    font_size: f32,
    text_color: [u8; 3],
    centered: bool,
    position_x: i32,
    position_y: i32,
    status: String,
    worker: Option<DisplayWorker>,
    events: Option<Receiver<WorkerEvent>>,
    file_dialog: Option<Receiver<Option<PathBuf>>>,
    preview_renderer: Option<ClockRenderer>,
    preview_style: Option<ClockStyle>,
    preview_texture: Option<egui::TextureHandle>,
    preview_time: String,
    preview_error: Option<String>,
}

impl Default for SettingsApp {
    fn default() -> Self {
        let (profile_repository, profile_store, profile_status) = match ProfileRepository::new() {
            Ok(repository) => match repository.load() {
                Ok(loaded) => (
                    Some(repository),
                    loaded.store,
                    loaded.warning.unwrap_or_default(),
                ),
                Err(error) => (Some(repository), ProfileStore::default(), error),
            },
            Err(error) => (None, ProfileStore::default(), error),
        };
        let profile_name = profile_store.selected_profile.clone();
        let style = profile_store
            .profiles
            .get(&profile_name)
            .cloned()
            .unwrap_or_default();
        let (position_x, position_y) = style.position.unwrap_or_default();
        Self {
            profile_repository,
            profile_store,
            profile_name,
            profile_status,
            background_path: style
                .background_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            background_color: style.background_color,
            font_family: style.font_family.unwrap_or_default(),
            font_families: system_font_families(),
            font_size: style.font_size,
            text_color: style.text_color,
            centered: style.position.is_none(),
            position_x,
            position_y,
            status: "Stopped".to_owned(),
            worker: None,
            events: None,
            file_dialog: None,
            preview_renderer: None,
            preview_style: None,
            preview_texture: None,
            preview_time: String::new(),
            preview_error: None,
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

    fn apply_style(&mut self, style: ClockStyle) {
        self.background_path = style
            .background_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        self.background_color = style.background_color;
        self.font_family = style.font_family.unwrap_or_default();
        self.font_size = style.font_size;
        self.text_color = style.text_color;
        self.centered = style.position.is_none();
        (self.position_x, self.position_y) = style.position.unwrap_or_default();
    }

    fn load_profile(&mut self, name: &str) {
        let Some(style) = self.profile_store.profiles.get(name).cloned() else {
            self.profile_status = format!("Profile not found: {name}");
            return;
        };
        self.profile_store.selected_profile = name.to_owned();
        self.profile_name = name.to_owned();
        self.apply_style(style);
        self.save_profile_store("Selected profile saved");
    }

    fn save_profile(&mut self) {
        let name = self.profile_name.trim().to_owned();
        if name.is_empty() {
            self.profile_status = "Profile name is required".to_owned();
            return;
        }
        self.profile_store
            .profiles
            .insert(name.clone(), self.style());
        self.profile_store.selected_profile = name.clone();
        self.profile_name = name;
        self.save_profile_store("Profile saved");
    }

    fn save_profile_store(&mut self, success: &str) {
        let Some(repository) = &self.profile_repository else {
            self.profile_status = "Profile directory is unavailable".to_owned();
            return;
        };
        self.profile_status = match repository.save(&self.profile_store) {
            Ok(()) => format!("{success}: {}", repository.path().display()),
            Err(error) => error,
        };
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
        let received: Vec<_> = events.try_iter().collect();
        let mut finished = false;
        for event in received {
            self.status = match event {
                WorkerEvent::Displayed { time, logical_rgb } => {
                    self.set_preview_texture(context, &logical_rgb);
                    self.preview_time = time.clone();
                    format!("Running - {time}")
                }
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

    fn receive_file_dialog(&mut self) {
        let Some(receiver) = &self.file_dialog else {
            return;
        };
        match receiver.try_recv() {
            Ok(Some(path)) => {
                self.background_path = path.display().to_string();
                self.file_dialog = None;
            }
            Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                self.file_dialog = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn open_background_dialog(&mut self) {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let selection = rfd::FileDialog::new()
                .add_filter("Image files", &["png", "jpg", "jpeg"])
                .pick_file();
            let _ = sender.send(selection);
        });
        self.file_dialog = Some(receiver);
    }

    fn update_preview(&mut self, context: &egui::Context) {
        let style = self.style();
        if self.preview_style.as_ref() != Some(&style) {
            match ClockRenderer::new(style.clone()) {
                Ok(renderer) => {
                    self.preview_renderer = Some(renderer);
                    self.preview_error = None;
                }
                Err(error) => {
                    self.preview_renderer = None;
                    self.preview_error = Some(error);
                }
            }
            self.preview_style = Some(style);
            self.preview_time.clear();
        }

        let time = Local::now().format("%H:%M:%S").to_string();
        if self.preview_time == time || self.worker.is_some() {
            return;
        }
        self.preview_time = time.clone();

        let Some(renderer) = &self.preview_renderer else {
            return;
        };
        let pixels = renderer.render_logical(&time);
        self.set_preview_texture(context, &pixels);
    }

    fn set_preview_texture(&mut self, context: &egui::Context, pixels: &[u8]) {
        let image = egui::ColorImage::from_rgb([LOGICAL_WIDTH, LOGICAL_HEIGHT], pixels);
        match &mut self.preview_texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.preview_texture = Some(context.load_texture(
                    "clock-preview",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_events(root_ui.ctx());
        self.receive_file_dialog();
        self.update_preview(root_ui.ctx());
        root_ui
            .ctx()
            .request_repaint_after(Duration::from_millis(250));

        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.heading("JONSBO DS339 Clock");
            ui.add_space(8.0);

            let profile_names: Vec<_> = self.profile_store.profiles.keys().cloned().collect();
            let mut selected_profile = None;
            egui::Grid::new("profiles")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Profile");
                    egui::ComboBox::from_id_salt("profile")
                        .selected_text(&self.profile_store.selected_profile)
                        .width(280.0)
                        .show_ui(ui, |ui| {
                            for name in &profile_names {
                                if ui
                                    .selectable_label(
                                        self.profile_store.selected_profile == *name,
                                        name,
                                    )
                                    .clicked()
                                {
                                    selected_profile = Some(name.clone());
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("Save as");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.profile_name).desired_width(220.0),
                        );
                        if ui.button("Save").clicked() {
                            self.save_profile();
                        }
                    });
                    ui.end_row();
                });
            if let Some(name) = selected_profile {
                self.load_profile(&name);
            }
            if !self.profile_status.is_empty() {
                ui.small(&self.profile_status);
            }
            ui.separator();

            egui::Grid::new("settings")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Background image");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.background_path)
                                .desired_width(220.0),
                        )
                        .on_hover_text("Leave blank to use the background color");
                        if ui
                            .add_enabled(self.file_dialog.is_none(), egui::Button::new("Browse..."))
                            .clicked()
                        {
                            self.open_background_dialog();
                        }
                    });
                    ui.end_row();

                    ui.label("Background color");
                    egui::color_picker::color_edit_button_srgb(ui, &mut self.background_color);
                    ui.end_row();

                    ui.label("Font family");
                    let selected_font = if self.font_family.is_empty() {
                        "System default"
                    } else {
                        &self.font_family
                    };
                    egui::ComboBox::from_id_salt("font-family")
                        .selected_text(selected_font)
                        .width(280.0)
                        .height(300.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.font_family,
                                String::new(),
                                "System default",
                            );
                            for family in &self.font_families {
                                ui.selectable_value(&mut self.font_family, family.clone(), family);
                            }
                        });
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

            ui.add_space(12.0);
            ui.separator();
            ui.label("Output preview (press Start / Apply to send changes)");
            if let Some(error) = &self.preview_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            } else if let Some(texture) = &self.preview_texture {
                let width = ui.available_width().min(720.0);
                let height = width * LOGICAL_HEIGHT as f32 / LOGICAL_WIDTH as f32;
                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(width, height)));
            }
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
    Displayed { time: String, logical_rgb: Vec<u8> },
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
            let mut ticker = FrameTicker::new(Duration::from_secs(1));

            while !worker_stop.load(Ordering::SeqCst) {
                match display.update() {
                    Ok(update) => {
                        if sender
                            .send(WorkerEvent::Displayed {
                                time: update.time,
                                logical_rgb: update.logical_rgb,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(WorkerEvent::Failed(error));
                        return;
                    }
                }

                if !ticker.wait(&worker_stop) {
                    let _ = sender.send(WorkerEvent::Stopped);
                    return;
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
