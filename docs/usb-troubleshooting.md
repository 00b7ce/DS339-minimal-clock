# USB output troubleshooting

Device Manager recognition alone does not prove that the panel is ready to
receive pixels. Close the official application and other DS339 output programs
before starting this application.

Run `jonsbo-ds339-minimal` without arguments to check HID access, panel status,
and bulk interface access without changing the display mode or sending pixels.
The current Windows probe uses the configured endpoint 0x04; it does not parse
the endpoint descriptors. A successful probe is not a successful frame transfer.

## Timeout -116 with a black or corrupted screen

`libusb0-dll:err [_usb_reap_async] timeout error (code -116)` means a submitted
USB transfer did not complete before its deadline. It does not mean the DLL is
missing. Errors now distinguish frame chunks (including byte offset) from the
zero-length end-of-frame transfer.

The corrected startup sequence powers the panel, disables video, initializes
panel control and GPIO, configures native mode, enables pixel transfer, sends
two complete frames, then enables the screen. Sending video-enable before
filling the buffers, or omitting panel/GPIO setup, can leave a detected device
unable to accept frames after reconnecting.

Wire-sequence evidence: the independently published
[OEM communication analysis](https://github.com/akisakurap/ds339-simhub/blob/main/docs/ds339-reverse-engineering.md).
The implementation uses the recorded command values; it does not import that
project's source or require its WinUSB driver replacement.

### Windows verification on 2026-10-12

- Device Manager reported healthy HID and OEM libusb0 interfaces.
- HID status reported the panel connected; bulk interface 3 could be claimed.
- The original whole-frame write timed out with -116.
- Chunking alone still timed out on the first 65,536-byte transfer.
- Adding panel/GPIO initialization and delaying video-enable until after two
  initial frames made both frame and end-of-frame transfers succeed.
- The user confirmed that the physical color bars displayed correctly.
- A clock run completed 16 consecutive updates without an error; two subsequent
  reopen/initialize/single-clock tests also succeeded.
- After installing the corrected GUI with a verified executable backup, the
  user confirmed that the saved clock appearance updated normally and changing
  a setting followed by **Start / Apply** updated the physical display.
- Installed executable and backup hashes were checked; the stored profile and
  login-startup registration were unchanged immediately after launch.
- No driver replacement or stored profile changes were required.

This establishes that the corrected sequence works on the tested DS339 with
the existing driver. Individual register writes were not isolated separately;
Linux hardware, sleep/resume and long-duration output remain unverified.
The previous application had worked for about two months with normal PC use,
including shutdowns. The trigger for this failure remains unknown; the tests
do not establish that device initialization state persisted during that period
or that the previous startup sequence always fails from a fresh power-on.

If startup fails after disabling video, the screen can remain black. Exit the
output program, reconnect the USB device, then restart the application and use
**Start / Apply**. Keep the previous executable when trying a replacement.
Do not repeatedly replay timed-out frames, because partial transfer progress is
unknown. Driver changes should follow a separate, device-specific diagnosis.
