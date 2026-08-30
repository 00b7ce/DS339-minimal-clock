use auto_launcher::{AutoLaunch, AutoLaunchBuilder, LinuxLaunchMode, WindowsEnableMode};

const APP_NAME: &str = "JONSBO DS339 Minimal";
pub const AUTOSTART_ARGUMENT: &str = "--autostart";

pub struct LoginLauncher {
    launcher: AutoLaunch,
}

impl LoginLauncher {
    pub fn new() -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("failed to locate the GUI executable: {error}"))?;
        let executable = executable
            .to_str()
            .ok_or_else(|| "the GUI executable path is not valid Unicode".to_owned())?;
        let quoted_executable = quote_argument(executable);

        let launcher = AutoLaunchBuilder::new()
            .set_app_name(APP_NAME)
            .set_app_path(&quoted_executable)
            .set_args(&[AUTOSTART_ARGUMENT])
            .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
            .set_linux_launch_mode(LinuxLaunchMode::XdgAutostart)
            .build()
            .map_err(|error| format!("failed to prepare login startup: {error}"))?;
        Ok(Self { launcher })
    }

    pub fn enable(&self) -> Result<(), String> {
        self.launcher
            .enable()
            .map_err(|error| format!("failed to enable login startup: {error}"))
    }

    pub fn disable(&self) -> Result<(), String> {
        self.launcher
            .disable()
            .map_err(|error| format!("failed to disable login startup: {error}"))
    }

    pub fn is_enabled(&self) -> Result<bool, String> {
        self.launcher
            .is_enabled()
            .map_err(|error| format!("failed to read login startup: {error}"))
    }
}

fn quote_argument(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_executable_paths_with_spaces() {
        assert_eq!(
            quote_argument(r"C:\Program Files\DS339\clock.exe"),
            r#""C:\Program Files\DS339\clock.exe""#
        );
    }
}
