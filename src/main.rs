use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use jonsbo_ds339_minimal::{bulk, clock, ds339, frame, gui, renderer};

enum Command {
    Probe,
    TestPattern,
    ClockOnce,
    Clock,
    Gui,
}

fn main() -> ExitCode {
    let command = match parse_arguments() {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    if matches!(command, Command::Gui) {
        return match gui::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("GUI failed: {error}");
                ExitCode::FAILURE
            }
        };
    }

    let devices = match ds339::detect() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("Failed to enumerate HID devices: {error}");
            return ExitCode::FAILURE;
        }
    };

    if devices.is_empty() {
        eprintln!(
            "JONSBO DS339 ({:04X}:{:04X}) was not found.",
            ds339::VENDOR_ID,
            ds339::PRODUCT_ID
        );
        return ExitCode::FAILURE;
    }

    println!("Detected {} JONSBO DS339 HID interface(s).", devices.len());

    let mut connected = 0;
    for (index, device) in devices.iter().enumerate() {
        println!("\nDevice {}", index + 1);
        println!("  Path: {}", device.path);
        println!(
            "  Product: {}",
            device.product.as_deref().unwrap_or("(not reported)")
        );
        println!(
            "  Manufacturer: {}",
            device.manufacturer.as_deref().unwrap_or("(not reported)")
        );
        println!(
            "  Serial: {}",
            device.serial_number.as_deref().unwrap_or("(not reported)")
        );
        println!(
            "  Interface: {}, usage page: 0x{:04X}, usage: 0x{:04X}",
            device.interface_number, device.usage_page, device.usage
        );

        match &device.open_error {
            None => {
                connected += 1;
                println!("  Connection: OK");
            }
            Some(error) => println!("  Connection: failed ({error})"),
        }

        match (device.panel_connected, &device.status_error) {
            (Some(true), _) => println!("  Panel status: connected"),
            (Some(false), _) => println!("  Panel status: disconnected"),
            (None, Some(error)) => println!("  Panel status: unavailable ({error})"),
            (None, None) => println!("  Panel status: unavailable"),
        }
    }

    if connected == 0 {
        eprintln!("\nThe DS339 was detected, but its HID interface could not be opened.");
        return ExitCode::FAILURE;
    }

    println!("\nHID connection check passed for {connected} device(s).");

    let bulk_devices = match bulk::detect() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("Failed to enumerate USB bulk devices: {error}");
            return ExitCode::FAILURE;
        }
    };

    if bulk_devices.is_empty() {
        eprintln!("The DS339 USB bulk device was not found.");
        return ExitCode::FAILURE;
    }

    let mut bulk_connected = 0;
    for device in &bulk_devices {
        println!(
            "\nBulk device at bus {}, address {}",
            device.bus, device.address
        );
        println!(
            "  Interface {}: {}",
            bulk::VIDEO_INTERFACE,
            if device.interface_found {
                "found"
            } else {
                "not found"
            }
        );
        println!(
            "  Endpoint 0x{:02X}: {}",
            bulk::VIDEO_ENDPOINT,
            if device.endpoint_found {
                "bulk OUT"
            } else {
                "not found"
            }
        );

        match &device.open_error {
            None => {
                bulk_connected += 1;
                println!("  Connection: OK");
            }
            Some(error) => println!("  Connection: failed ({error})"),
        }
    }

    if bulk_connected == 0 {
        eprintln!("\nThe DS339 was detected, but its bulk interface could not be claimed.");
        return ExitCode::FAILURE;
    }

    println!("\nBulk connection check passed for {bulk_connected} device(s).");

    let result = match command {
        Command::Probe => Ok(()),
        Command::TestPattern => send_test_frame(),
        Command::ClockOnce => send_clock(false),
        Command::Clock => send_clock(true),
        Command::Gui => unreachable!(),
    };
    if let Err(error) = result {
        eprintln!("Display update failed: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn parse_arguments() -> Result<Command, String> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => Ok(Command::Probe),
        [argument] if argument == "--test-pattern" => Ok(Command::TestPattern),
        [argument] if argument == "--clock-once" => Ok(Command::ClockOnce),
        [argument] if argument == "--clock" => Ok(Command::Clock),
        [argument] if argument == "--gui" => Ok(Command::Gui),
        _ => Err(
            "Usage: jonsbo-ds339-minimal [--test-pattern|--clock-once|--clock|--gui]".to_owned(),
        ),
    }
}

fn send_clock(continuous: bool) -> Result<(), String> {
    let stop_requested = Arc::new(AtomicBool::new(false));
    if continuous {
        let stop_requested = Arc::clone(&stop_requested);
        ctrlc::set_handler(move || stop_requested.store(true, Ordering::SeqCst))
            .map_err(|error| format!("failed to install shutdown handler: {error}"))?;
    }

    let mut display = clock::ClockDisplay::open(renderer::ClockStyle::default())?;
    let mut ticker = clock::FrameTicker::new(Duration::from_secs(1));

    loop {
        let update = display.update()?;
        println!("Displayed {}", update.time);

        if !continuous {
            return Ok(());
        }
        if !ticker.wait(&stop_requested) {
            println!("Clock stopped.");
            return Ok(());
        }
    }
}

fn send_test_frame() -> Result<(), String> {
    let control = ds339::ControlConnection::open().map_err(|error| error.to_string())?;
    let mut video = bulk::VideoConnection::open()?;
    let pixels = frame::color_bars();
    let encoded = frame::encode(&pixels)?;

    control
        .set_native_mode()
        .map_err(|error| error.to_string())?;
    video.write_all(&encoded)?;
    video.write_all(&encoded)?;
    control.show_screen().map_err(|error| error.to_string())?;
    println!(
        "\nSent a {}x{} RGB test pattern ({} bytes).",
        frame::WIDTH,
        frame::HEIGHT,
        encoded.len()
    );
    Ok(())
}
