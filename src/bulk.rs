use rusb::{Direction, TransferType};

use crate::ds339::{PRODUCT_ID, VENDOR_ID};

pub const VIDEO_INTERFACE: u8 = 3;
pub const VIDEO_ENDPOINT: u8 = 0x04;

#[derive(Debug)]
pub struct BulkDevice {
    pub bus: u8,
    pub address: u8,
    pub interface_found: bool,
    pub endpoint_found: bool,
    pub open_error: Option<String>,
}

pub struct VideoConnection {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
}

impl VideoConnection {
    pub fn open() -> Result<Self, String> {
        let handle = rusb::open_device_with_vid_pid(VENDOR_ID, PRODUCT_ID)
            .ok_or_else(|| "DS339 bulk device not found".to_owned())?;
        handle
            .claim_interface(VIDEO_INTERFACE)
            .map_err(|error| error.to_string())?;
        Ok(Self { handle })
    }

    pub fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        let timeout = std::time::Duration::from_secs(3);
        crate::transport::write_frame(data, |chunk| {
            let written = self
                .handle
                .write_bulk(VIDEO_ENDPOINT, chunk, timeout)
                .map_err(|error| error.to_string())?;
            if written != chunk.len() {
                return Err(format!("short bulk write: {written}/{} bytes", chunk.len()));
            }
            Ok(())
        })
    }
}

impl Drop for VideoConnection {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(VIDEO_INTERFACE);
    }
}

pub fn detect() -> Result<Vec<BulkDevice>, rusb::Error> {
    let devices = rusb::devices()?;
    let mut matches = Vec::new();

    for device in devices.iter() {
        let descriptor = device.device_descriptor()?;
        if descriptor.vendor_id() != VENDOR_ID || descriptor.product_id() != PRODUCT_ID {
            continue;
        }

        let mut interface_found = false;
        let mut endpoint_found = false;

        for config_index in 0..descriptor.num_configurations() {
            let config = device.config_descriptor(config_index)?;
            for interface in config.interfaces() {
                if interface.number() != VIDEO_INTERFACE {
                    continue;
                }

                interface_found = true;
                endpoint_found = interface.descriptors().any(|interface_descriptor| {
                    interface_descriptor.endpoint_descriptors().any(|endpoint| {
                        endpoint.address() == VIDEO_ENDPOINT
                            && endpoint.direction() == Direction::Out
                            && endpoint.transfer_type() == TransferType::Bulk
                    })
                });
            }
        }

        let open_error = match device.open() {
            Ok(handle) => handle
                .claim_interface(VIDEO_INTERFACE)
                .err()
                .map(|error| error.to_string()),
            Err(error) => Some(error.to_string()),
        };

        matches.push(BulkDevice {
            bus: device.bus_number(),
            address: device.address(),
            interface_found,
            endpoint_found,
            open_error,
        });
    }

    Ok(matches)
}
