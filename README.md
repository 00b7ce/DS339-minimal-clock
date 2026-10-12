# DS339 Minimal Clock

An unofficial, lightweight desktop application that continuously displays a configurable clock on the JONSBO DS339. The GUI lets you combine a background image or color with a selected font, size, color, and placement, while previewing the exact logical frame sent to the display.

> [!IMPORTANT]
> This is not official JONSBO software. Communication with physical hardware has been verified on Windows. Linux builds are provided, but hardware communication on Linux has not yet been verified on a DS339.

## Features

- Updates an `HH:MM:SS` clock once per second
- Uses a PNG/JPEG background image or a solid background color
- Lists installed fonts in a drop-down menu
- Configures font size, text color, centered placement, or exact coordinates
- Previews the same `960x376` logical frame that is sent to the DS339
- Saves multiple profiles and restores the selected profile on launch
- Runs in the system tray with Start / Apply, Stop, Open settings, and Exit actions
- Can start automatically at sign-in, begin output immediately, and remain hidden in the tray
- Includes CLI commands for USB diagnostics and test-pattern output

## Windows quick start

1. Download and extract the Windows archive from [Releases](https://github.com/00b7ce/DS339-minimal-clock/releases).
2. Install the official JONSBO software/driver first, then close the official application.
3. Run `jonsbo-ds339-gui.exe`.
4. Choose a background and font settings, then select **Start / Apply**.

The Windows USB video transport dynamically loads `libusb0.dll`. If the DLL cannot be found, verify the official software installation and ensure that its directory is on the Windows search path or place the DLL beside the executable. The DLL itself is not distributed in this repository or its releases.

If Windows detects the display but output remains black or fails with libusb
timeout `-116`, see [USB troubleshooting](docs/usb-troubleshooting.md).

Closing the settings window keeps the application running in the system tray. Use **Exit** in the tray menu to stop output and terminate the application.

### Profiles

Enter a name under **Save as**, then select **Save** to store the current display settings. The selected profile is restored on the next launch.

Profile data is stored at:

- Windows: `%APPDATA%\jonsbo-ds339-minimal\profiles.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/jonsbo-ds339-minimal/profiles.toml`

Before overwriting the file, the application creates `profiles.toml.bak`. If the main file is damaged, it attempts to recover from that backup.

### Start automatically at sign-in

Enable **Start automatically at sign-in (hidden in the system tray)** to:

1. Launch the GUI with `--autostart` after the current user signs in.
2. Keep the settings window hidden in the system tray.
3. Start DS339 output immediately with the selected profile.

Windows uses `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; Linux uses an XDG Autostart desktop entry. Disabling the option removes the registration.

## CLI

`jonsbo-ds339-minimal` includes diagnostic commands:

```text
jonsbo-ds339-minimal                  # Check USB/HID connectivity without changing the display
jonsbo-ds339-minimal --test-pattern   # Send the RGB test pattern
jonsbo-ds339-minimal --clock-once     # Send one clock frame
jonsbo-ds339-minimal --clock          # Update the clock once per second; stop with Ctrl+C
jonsbo-ds339-minimal --gui            # Launch the GUI with a console window
```

The expected logical image for `--test-pattern` is shown below. Depending on the physical orientation of the display, it may appear rotated.

![Expected RGB test pattern](docs/expected-test-pattern.png)

## Linux

The release workflow builds Linux x86_64 binaries on Ubuntu. For a local Ubuntu/Debian build, install the native dependencies:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libudev-dev libusb-1.0-0-dev \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

Then build with stable Rust:

```bash
cargo build --release --bins
```

Access to the USB device may require an appropriate udev rule or group membership. Linux binaries should currently be considered experimental because physical DS339 communication has not yet been verified on Linux.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --bins
```

The currently targeted DS339 has USB VID `345F`, PID `9132`, image interface `3`, and bulk OUT endpoint `0x04`. Frames are rendered as a landscape `960x376` logical image and converted to the device's native orientation and BGR byte order before transmission.

Startup initializes the panel control registers and GPIO, fills both initial
frame buffers, and then enables the screen. Frames are sent in 64 KiB chunks
followed by a zero-length transfer. Transfer errors identify the failing offset
or end-of-frame stage; an incomplete frame is never automatically replayed.

## Known limitations

- Physical hardware operation has only been verified on Windows.
- There is no UI for choosing between multiple connected DS339 devices.
- Automatic reconnection after sleep is not implemented. Select **Start / Apply** again if the device disconnects.
- The project does not include a signed installer or automatic updater.

## License

[MIT License](LICENSE)
