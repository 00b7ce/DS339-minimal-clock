#[cfg(target_os = "linux")]
mod bulk;
#[cfg(windows)]
#[path = "bulk_windows.rs"]
mod bulk;
mod ds339;

use std::process::ExitCode;

fn main() -> ExitCode {
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
        ExitCode::FAILURE
    } else {
        println!("\nBulk connection check passed for {bulk_connected} device(s).");
        ExitCode::SUCCESS
    }
}
