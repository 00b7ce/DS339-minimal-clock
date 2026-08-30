use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Local;
use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::autostart::{AUTOSTART_ARGUMENT, LoginLauncher};
use crate::clock::{ClockDisplay, FrameTicker};
use crate::profile::{ProfileRepository, ProfileStore};
use crate::renderer::{
    ClockRenderer, ClockStyle, LOGICAL_HEIGHT, LOGICAL_WIDTH, system_font_families,
};

const TRAY_SHOW: &str = "show";
const TRAY_START: &str = "start";
const TRAY_STOP: &str = "stop";
const TRAY_EXIT: &str = "exit";

pub fn run() -> Result<(), String> {
    let launched_by_autostart = std::env::args_os().any(|argument| argument == AUTOSTART_ARGUMENT);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 650.0])
            .with_visible(!launched_by_autostart),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "JONSBO DS339 Minimal",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(SettingsApp::new(
                creation_context,
                launched_by_autostart,
            )))
        }),
    )
    .map_err(|error| error.to_string())
}

struct SettingsApp {
    login_launcher: Option<LoginLauncher>,
    tray_icon: Option<TrayIcon>,
    tray_status: String,
    exit_requested: Arc<AtomicBool>,
    tray_events: Option<Receiver<TrayAction>>,
    autostart_hide_frames: u8,
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
    events: Option<Arc<WorkerMailbox>>,
    file_dialog: Option<Receiver<Option<PathBuf>>>,
    preview_renderer: Option<ClockRenderer>,
    preview_style: Option<ClockStyle>,
    preview_texture: Option<egui::TextureHandle>,
    preview_time: String,
    preview_error: Option<String>,
}

impl SettingsApp {
    fn new(creation_context: &eframe::CreationContext<'_>, launched_by_autostart: bool) -> Self {
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
        let (tray_icon, tray_status) = match create_tray_icon() {
            Ok(icon) => (Some(icon), String::new()),
            Err(error) => (None, format!("Tray unavailable: {error}")),
        };
        let exit_requested = Arc::new(AtomicBool::new(false));
        let tray_events = tray_icon.as_ref().map(|_| {
            install_tray_event_handlers(&creation_context.egui_ctx, Arc::clone(&exit_requested))
        });
        let start_on_launch = profile_store.start_on_launch;
        let (login_launcher, login_status) = match LoginLauncher::new() {
            Ok(launcher) => {
                let status = if start_on_launch {
                    launcher.enable().err()
                } else {
                    match launcher.is_enabled() {
                        Ok(true) => Some(
                            "Login startup is registered but disabled in this profile; toggle the setting to resync"
                                .to_owned(),
                        ),
                        Ok(false) => None,
                        Err(error) => Some(error),
                    }
                };
                (Some(launcher), status.unwrap_or_default())
            }
            Err(error) => (None, error),
        };
        let mut app = Self {
            login_launcher,
            tray_icon,
            tray_status,
            exit_requested,
            tray_events,
            // eframe shows the root window after its first rendered frame. Reapply hidden state
            // for that frame and the next one so login startup remains tray-only.
            autostart_hide_frames: if launched_by_autostart { 2 } else { 0 },
            profile_repository,
            profile_store,
            profile_name,
            profile_status: if login_status.is_empty() {
                profile_status
            } else {
                login_status
            },
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
        };
        if start_on_launch || launched_by_autostart {
            app.start();
        }
        if launched_by_autostart && app.tray_icon.is_none() {
            show_window(&creation_context.egui_ctx);
        }
        app
    }
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

    fn set_login_startup(&mut self, enabled: bool) {
        let Some(launcher) = &self.login_launcher else {
            self.profile_status = "Login startup is unavailable".to_owned();
            return;
        };
        let result = if enabled {
            launcher.enable()
        } else {
            launcher.disable()
        };
        match result {
            Ok(()) => {
                self.profile_store.start_on_launch = enabled;
                self.save_profile_store(if enabled {
                    "Login startup enabled"
                } else {
                    "Login startup disabled"
                });
            }
            Err(error) => self.profile_status = error,
        }
    }

    fn stop(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        self.events = None;
    }

    fn receive_events(&mut self, context: &egui::Context) {
        let Some(event) = self.events.as_ref().and_then(|events| events.take()) else {
            return;
        };
        let mut finished = false;
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

    fn receive_tray_events(&mut self, context: &egui::Context) {
        let received: Vec<_> = self
            .tray_events
            .as_ref()
            .map(|events| events.try_iter().collect())
            .unwrap_or_default();
        for event in received {
            match event {
                TrayAction::Start => self.start(),
                TrayAction::Stop => {
                    self.stop();
                    self.status = "Stopped".to_owned();
                }
            }
        }
        context.request_repaint();
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
        if self.autostart_hide_frames > 0 {
            root_ui
                .ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            root_ui
                .ctx()
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.autostart_hide_frames -= 1;
            root_ui.ctx().request_repaint();
        }
        self.receive_tray_events(root_ui.ctx());
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

            let mut start_on_launch = self.profile_store.start_on_launch;
            if ui
                .add_enabled(
                    self.login_launcher.is_some(),
                    egui::Checkbox::new(
                        &mut start_on_launch,
                        "Start automatically at sign-in (hidden in the system tray)",
                    ),
                )
                .changed()
            {
                self.set_login_startup(start_on_launch);
            }
            if self.tray_icon.is_some() {
                ui.small("Closing this window keeps the app in the system tray.");
            } else if !self.tray_status.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.tray_status);
            }

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

        if root_ui
            .ctx()
            .input(|input| input.viewport().close_requested())
            && !self.exit_requested.load(Ordering::SeqCst)
            && self.tray_icon.is_some()
        {
            root_ui
                .ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            root_ui
                .ctx()
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
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

fn show_window(context: &egui::Context) {
    context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    context.send_viewport_cmd(egui::ViewportCommand::Focus);
}

#[derive(Clone, Copy)]
enum TrayAction {
    Start,
    Stop,
}

fn install_tray_event_handlers(
    context: &egui::Context,
    exit_requested: Arc<AtomicBool>,
) -> Receiver<TrayAction> {
    let (sender, receiver) = mpsc::channel();
    let menu_context = context.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        handle_tray_menu(event.id.as_ref(), &menu_context, &exit_requested, &sender);
    }));

    let tray_context = context.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            show_window(&tray_context);
            tray_context.request_repaint();
        }
    }));
    receiver
}

fn handle_tray_menu(
    id: &str,
    context: &egui::Context,
    exit_requested: &AtomicBool,
    sender: &mpsc::Sender<TrayAction>,
) {
    match id {
        TRAY_SHOW => show_window(context),
        TRAY_START => {
            let _ = sender.send(TrayAction::Start);
        }
        TRAY_STOP => {
            let _ = sender.send(TrayAction::Stop);
        }
        TRAY_EXIT => {
            exit_requested.store(true, Ordering::SeqCst);
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        _ => return,
    }
    context.request_repaint();
}

fn create_tray_icon() -> Result<TrayIcon, String> {
    let menu = Menu::new();
    let show = MenuItem::with_id(TRAY_SHOW, "Open settings", true, None);
    let start = MenuItem::with_id(TRAY_START, "Start / Apply", true, None);
    let stop = MenuItem::with_id(TRAY_STOP, "Stop", true, None);
    let separator = PredefinedMenuItem::separator();
    let exit = MenuItem::with_id(TRAY_EXIT, "Exit", true, None);
    menu.append_items(&[&show, &start, &stop, &separator, &exit])
        .map_err(|error| error.to_string())?;

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("JONSBO DS339 Minimal")
        .with_icon(tray_icon_image()?)
        .build()
        .map_err(|error| error.to_string())
}

fn tray_icon_image() -> Result<Icon, String> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let radius_squared = dx * dx + dy * dy;
            let color = if radius_squared <= 14 * 14 {
                if radius_squared >= 11 * 11 {
                    [78, 174, 255, 255]
                } else if (x == 16 && (8..=17).contains(&y)) || (y == 16 && (16..=23).contains(&x))
                {
                    [255, 255, 255, 255]
                } else {
                    [18, 27, 40, 255]
                }
            } else {
                [0, 0, 0, 0]
            };
            let offset = ((y * SIZE + x) * 4) as usize;
            rgba[offset..offset + 4].copy_from_slice(&color);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).map_err(|error| error.to_string())
}

enum WorkerEvent {
    Displayed { time: String, logical_rgb: Vec<u8> },
    Stopped,
    Failed(String),
}

#[derive(Default)]
struct WorkerMailbox {
    latest: Mutex<Option<WorkerEvent>>,
}

impl WorkerMailbox {
    fn publish(&self, event: WorkerEvent) {
        let mut latest = self
            .latest
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *latest = Some(event);
    }

    fn take(&self) -> Option<WorkerEvent> {
        self.latest
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

struct DisplayWorker {
    stop_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DisplayWorker {
    fn start(style: ClockStyle) -> (Self, Arc<WorkerMailbox>) {
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop_requested);
        let events = Arc::new(WorkerMailbox::default());
        let worker_events = Arc::clone(&events);
        let thread = thread::spawn(move || {
            let mut display = match ClockDisplay::open(style) {
                Ok(display) => display,
                Err(error) => {
                    worker_events.publish(WorkerEvent::Failed(error));
                    return;
                }
            };
            let mut ticker = FrameTicker::new(Duration::from_secs(1));

            while !worker_stop.load(Ordering::SeqCst) {
                match display.update() {
                    Ok(update) => {
                        worker_events.publish(WorkerEvent::Displayed {
                            time: update.time,
                            logical_rgb: update.logical_rgb,
                        });
                    }
                    Err(error) => {
                        worker_events.publish(WorkerEvent::Failed(error));
                        return;
                    }
                }

                if !ticker.wait(&worker_stop) {
                    worker_events.publish(WorkerEvent::Stopped);
                    return;
                }
            }
            worker_events.publish(WorkerEvent::Stopped);
        });

        (
            Self {
                stop_requested,
                thread: Some(thread),
            },
            events,
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

    #[test]
    fn worker_mailbox_keeps_only_the_latest_frame() {
        let mailbox = WorkerMailbox::default();
        mailbox.publish(WorkerEvent::Displayed {
            time: "12:00:00".to_owned(),
            logical_rgb: vec![1],
        });
        mailbox.publish(WorkerEvent::Displayed {
            time: "12:00:01".to_owned(),
            logical_rgb: vec![2],
        });

        let Some(WorkerEvent::Displayed { time, logical_rgb }) = mailbox.take() else {
            panic!("expected the latest displayed frame");
        };
        assert_eq!(time, "12:00:01");
        assert_eq!(logical_rgb, vec![2]);
        assert!(mailbox.take().is_none());
    }

    #[test]
    fn tray_exit_marks_the_close_as_intentional() {
        let context = egui::Context::default();
        let exit_requested = AtomicBool::new(false);
        let (sender, _receiver) = mpsc::channel();

        handle_tray_menu(TRAY_EXIT, &context, &exit_requested, &sender);

        assert!(exit_requested.load(Ordering::SeqCst));
    }
}
