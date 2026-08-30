use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use libloading::Library;

use crate::ds339::{PRODUCT_ID, VENDOR_ID};

pub const VIDEO_INTERFACE: u8 = 3;
pub const VIDEO_ENDPOINT: u8 = 0x04;

const LIBUSB_PATH_MAX: usize = 512;

#[derive(Debug)]
pub struct BulkDevice {
    pub bus: u8,
    pub address: u8,
    pub interface_found: bool,
    pub endpoint_found: bool,
    pub open_error: Option<String>,
}

#[repr(C, packed)]
struct UsbDeviceDescriptor {
    length: u8,
    descriptor_type: u8,
    usb_version: u16,
    device_class: u8,
    device_subclass: u8,
    device_protocol: u8,
    max_packet_size: u8,
    vendor_id: u16,
    product_id: u16,
    device_version: u16,
    manufacturer_index: u8,
    product_index: u8,
    serial_number_index: u8,
    configuration_count: u8,
}

#[repr(C, packed)]
struct UsbDevice {
    next: *mut UsbDevice,
    previous: *mut UsbDevice,
    filename: [c_char; LIBUSB_PATH_MAX],
    bus: *mut UsbBus,
    descriptor: UsbDeviceDescriptor,
    config: *mut c_void,
    platform_device: *mut c_void,
    device_number: u8,
    child_count: u8,
    children: *mut *mut UsbDevice,
}

#[repr(C, packed)]
struct UsbBus {
    next: *mut UsbBus,
    previous: *mut UsbBus,
    directory_name: [c_char; LIBUSB_PATH_MAX],
    devices: *mut UsbDevice,
    location: u32,
    root_device: *mut UsbDevice,
}

enum UsbDeviceHandle {}

type UsbInit = unsafe extern "C" fn();
type UsbFindBusses = unsafe extern "C" fn() -> c_int;
type UsbFindDevices = unsafe extern "C" fn() -> c_int;
type UsbGetBusses = unsafe extern "C" fn() -> *mut UsbBus;
type UsbOpen = unsafe extern "C" fn(*mut UsbDevice) -> *mut UsbDeviceHandle;
type UsbClose = unsafe extern "C" fn(*mut UsbDeviceHandle) -> c_int;
type UsbClaimInterface = unsafe extern "C" fn(*mut UsbDeviceHandle, c_int) -> c_int;
type UsbReleaseInterface = unsafe extern "C" fn(*mut UsbDeviceHandle, c_int) -> c_int;
type UsbStrError = unsafe extern "C" fn() -> *const c_char;

struct Libusb0 {
    _library: Library,
    init: UsbInit,
    find_busses: UsbFindBusses,
    find_devices: UsbFindDevices,
    get_busses: UsbGetBusses,
    open: UsbOpen,
    close: UsbClose,
    claim_interface: UsbClaimInterface,
    release_interface: UsbReleaseInterface,
    strerror: UsbStrError,
}

impl Libusb0 {
    fn load() -> Result<Self, String> {
        // SAFETY: The library stays owned by Self for at least as long as all copied
        // function pointers. Symbol signatures match the libusb-win32 0.1 API.
        unsafe {
            let library = Library::new("libusb0.dll").map_err(|error| error.to_string())?;
            let init = load_symbol(&library, b"usb_init\0")?;
            let find_busses = load_symbol(&library, b"usb_find_busses\0")?;
            let find_devices = load_symbol(&library, b"usb_find_devices\0")?;
            let get_busses = load_symbol(&library, b"usb_get_busses\0")?;
            let open = load_symbol(&library, b"usb_open\0")?;
            let close = load_symbol(&library, b"usb_close\0")?;
            let claim_interface = load_symbol(&library, b"usb_claim_interface\0")?;
            let release_interface = load_symbol(&library, b"usb_release_interface\0")?;
            let strerror = load_symbol(&library, b"usb_strerror\0")?;

            Ok(Self {
                _library: library,
                init,
                find_busses,
                find_devices,
                get_busses,
                open,
                close,
                claim_interface,
                release_interface,
                strerror,
            })
        }
    }

    fn last_error(&self) -> String {
        // SAFETY: libusb0 owns the returned NUL-terminated error string.
        let error = unsafe { (self.strerror)() };
        if error.is_null() {
            "unknown libusb0 error".to_owned()
        } else {
            // SAFETY: usb_strerror returns a valid NUL-terminated string pointer.
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        }
    }
}

unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: The caller supplies the C signature corresponding to the named export.
    unsafe { library.get::<T>(name) }
        .map(|symbol| *symbol)
        .map_err(|error| error.to_string())
}

pub fn detect() -> Result<Vec<BulkDevice>, String> {
    let api = Libusb0::load()?;

    // SAFETY: These calls initialize and refresh libusb0's process-global device list.
    unsafe {
        (api.init)();
        (api.find_busses)();
        (api.find_devices)();
    }

    let mut matches = Vec::new();
    // SAFETY: usb_get_busses returns libusb0-owned linked lists that remain valid until
    // the next enumeration call. We only read fields and use read_unaligned because the
    // libusb-win32 public structures are packed to one-byte alignment.
    unsafe {
        let mut bus = (api.get_busses)();
        while !bus.is_null() {
            let mut device = ptr::addr_of!((*bus).devices).read_unaligned();
            while !device.is_null() {
                let vendor_id = ptr::addr_of!((*device).descriptor.vendor_id).read_unaligned();
                let product_id = ptr::addr_of!((*device).descriptor.product_id).read_unaligned();

                if vendor_id == VENDOR_ID && product_id == PRODUCT_ID {
                    let address = ptr::addr_of!((*device).device_number).read_unaligned();
                    let handle = (api.open)(device);
                    let open_error = if handle.is_null() {
                        Some(api.last_error())
                    } else {
                        let claim_result = (api.claim_interface)(handle, VIDEO_INTERFACE.into());
                        let result = if claim_result == 0 {
                            (api.release_interface)(handle, VIDEO_INTERFACE.into());
                            None
                        } else {
                            Some(format!("{} (code {claim_result})", api.last_error()))
                        };
                        (api.close)(handle);
                        result
                    };

                    matches.push(BulkDevice {
                        bus: 0,
                        address,
                        interface_found: true,
                        endpoint_found: true,
                        open_error,
                    });
                }

                device = ptr::addr_of!((*device).next).read_unaligned();
            }
            bus = ptr::addr_of!((*bus).next).read_unaligned();
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn matches_libusb0_packed_structure_sizes() {
        let pointer_size = size_of::<*mut c_void>();
        assert_eq!(size_of::<UsbDeviceDescriptor>(), 18);
        assert_eq!(size_of::<UsbDevice>(), 532 + 6 * pointer_size);
        assert_eq!(size_of::<UsbBus>(), 516 + 4 * pointer_size);
    }
}
