use hidapi::{HidApi, HidError};

pub const VENDOR_ID: u16 = 0x345F;
pub const PRODUCT_ID: u16 = 0x9132;

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
}

pub fn detect() -> Result<Vec<DetectedDevice>, HidError> {
    let api = HidApi::new()?;
    let devices = api
        .device_list()
        .filter(|device| is_ds339(device.vendor_id(), device.product_id()))
        .map(|device| DetectedDevice {
            path: device.path().to_string_lossy().into_owned(),
            product: device.product_string().map(ToOwned::to_owned),
            manufacturer: device.manufacturer_string().map(ToOwned::to_owned),
            serial_number: device.serial_number().map(ToOwned::to_owned),
            interface_number: device.interface_number(),
            usage_page: device.usage_page(),
            usage: device.usage(),
            open_error: device
                .open_device(&api)
                .err()
                .map(|error| error.to_string()),
        })
        .collect();

    Ok(devices)
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
}
