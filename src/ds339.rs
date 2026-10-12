use hidapi::{HidApi, HidDevice, HidError};
use std::thread;
use std::time::Duration;

pub const VENDOR_ID: u16 = 0x345F;
pub const PRODUCT_ID: u16 = 0x9132;
const NATIVE_WIDTH: u16 = 376;
const NATIVE_HEIGHT: u16 = 960;
const NATIVE_VIC: u8 = 171;

#[derive(Debug)]
pub struct DetectedDevice {
    pub path: String,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub open_error: Option<String>,
    pub panel_connected: Option<bool>,
    pub status_error: Option<String>,
}

pub struct ControlConnection {
    device: HidDevice,
}

impl ControlConnection {
    pub fn open() -> Result<Self, HidError> {
        let api = HidApi::new()?;
        let device = api.open(VENDOR_ID, PRODUCT_ID)?;
        Ok(Self { device })
    }

    pub fn set_native_mode(&self) -> Result<(), HidError> {
        let [width_high, width_low] = NATIVE_WIDTH.to_be_bytes();
        let [height_high, height_low] = NATIVE_HEIGHT.to_be_bytes();

        self.write_control([0xA6, 0x07, 0x01, 0x02, 0, 0, 0, 0])?;
        self.write_control([0xA6, 0x05, 0x00, 0, 0, 0, 0, 0])?;
        // Cold-start panel/GPIO setup observed in the OEM initialization sequence.
        // Mode commands alone can leave the bulk receiver unready after reconnect.
        self.write_control([0xB6, 0xF0, 0x05, 0x40, 0, 0, 0, 0])?;
        self.write_control([0xC6, 0xB0, 0xDF, 0, 0, 0, 0, 0])?;
        self.write_control([0xC6, 0xA0, 0xE4, 0, 0, 0, 0, 0])?;
        self.write_control([0xC6, 0xA0, 0xF0, 0, 0, 0, 0, 0])?;
        self.write_control([0xB6, 0xF0, 0x16, 0x08, 0, 0, 0, 0])?;
        self.write_control([0xA6, 0x03, 0x03, 0, 0, 0, 0, 0])?;
        self.write_control([
            0xA6,
            0x01,
            width_high,
            width_low,
            height_high,
            height_low,
            0x11,
            0x00,
        ])?;
        self.write_control([
            0xA6,
            0x02,
            NATIVE_VIC,
            0x00,
            width_high,
            width_low,
            height_high,
            height_low,
        ])?;
        self.write_control([0xA6, 0x04, 0x01, 0, 0, 0, 0, 0])?;
        thread::sleep(Duration::from_millis(100));
        Ok(())
    }

    /// Enable the panel only after both initial frame buffers have been filled.
    pub fn show_screen(&self) -> Result<(), HidError> {
        self.write_control([0xB6, 0xF0, 0x05, 0x50, 0, 0, 0, 0])?;
        self.write_control([0xA6, 0x05, 0x01, 0, 0, 0, 0, 0])
    }

    fn write_control(&self, payload: [u8; 8]) -> Result<(), HidError> {
        let mut report = [0_u8; 9];
        report[1..].copy_from_slice(&payload);
        self.device
            .send_feature_report(&report)
            .map_err(|error| HidError::HidApiError {
                message: format!("HID command {payload:02X?}: {error}"),
            })?;
        Ok(())
    }
}

pub fn detect() -> Result<Vec<DetectedDevice>, HidError> {
    let api = HidApi::new()?;
    let devices = api
        .device_list()
        .filter(|device| is_ds339(device.vendor_id(), device.product_id()))
        .map(|device_info| {
            let (open_error, panel_connected, status_error) = match device_info.open_device(&api) {
                Ok(device) => match read_register(&device, 0x0032) {
                    Ok(status) => (None, Some(status == 1), None),
                    Err(error) => (None, None, Some(error.to_string())),
                },
                Err(error) => (Some(error.to_string()), None, None),
            };

            DetectedDevice {
                path: device_info.path().to_string_lossy().into_owned(),
                product: device_info.product_string().map(ToOwned::to_owned),
                manufacturer: device_info.manufacturer_string().map(ToOwned::to_owned),
                serial_number: device_info.serial_number().map(ToOwned::to_owned),
                interface_number: device_info.interface_number(),
                usage_page: device_info.usage_page(),
                usage: device_info.usage(),
                open_error,
                panel_connected,
                status_error,
            }
        })
        .collect();

    Ok(devices)
}

fn read_register(device: &HidDevice, address: u16) -> Result<u8, HidError> {
    let request = register_read_request(address);
    device.send_feature_report(&request)?;

    let mut response = [0_u8; 9];
    let response_length = device.get_feature_report(&mut response)?;
    if response_length < 5 {
        return Err(HidError::HidApiError {
            message: format!("short register response: {response_length} bytes"),
        });
    }

    Ok(response[4])
}

fn register_read_request(address: u16) -> [u8; 9] {
    [0, 0xB5, (address >> 8) as u8, address as u8, 0, 0, 0, 0, 0]
}

fn is_ds339(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == VENDOR_ID && product_id == PRODUCT_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ds339_usb_id() {
        assert!(is_ds339(VENDOR_ID, PRODUCT_ID));
    }

    #[test]
    fn rejects_other_usb_ids() {
        assert!(!is_ds339(0x345F, 0x9131));
        assert!(!is_ds339(0x1234, PRODUCT_ID));
    }

    #[test]
    fn builds_register_read_feature_report() {
        assert_eq!(
            register_read_request(0x1234),
            [0x00, 0xB5, 0x12, 0x34, 0, 0, 0, 0, 0]
        );
    }
}
