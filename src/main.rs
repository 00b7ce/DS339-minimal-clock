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
    }

    if connected == 0 {
        eprintln!("\nThe DS339 was detected, but its HID interface could not be opened.");
        ExitCode::FAILURE
    } else {
        println!("\nHID connection check passed for {connected} device(s).");
        ExitCode::SUCCESS
    }
}
