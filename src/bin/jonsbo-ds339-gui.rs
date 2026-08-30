#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = jonsbo_ds339_minimal::gui::run() {
        rfd::MessageDialog::new()
            .set_title("JONSBO DS339 Minimal")
            .set_description(format!("GUI failed: {error}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
