pub const WIDTH: usize = 376;
pub const HEIGHT: usize = 960;
const BYTES_PER_PIXEL: usize = 3;
const HEADER_SIZE: usize = 8;
const TRAILER_SIZE: usize = 8;

pub fn color_bars() -> Vec<u8> {
    let mut pixels = vec![0_u8; WIDTH * HEIGHT * BYTES_PER_PIXEL];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let color = if !(8..WIDTH - 8).contains(&x) || !(8..HEIGHT - 8).contains(&y) {
                [0, 0, 0]
            } else if x < WIDTH / 3 {
                [0, 0, 255]
            } else if x < WIDTH * 2 / 3 {
                [0, 255, 0]
            } else {
                [255, 0, 0]
            };
            let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
            pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&color);
        }
    }

    pixels
}

pub fn encode(pixels: &[u8]) -> Result<Vec<u8>, String> {
    let expected = WIDTH * HEIGHT * BYTES_PER_PIXEL;
    if pixels.len() != expected {
        return Err(format!(
            "invalid pixel buffer length: {} (expected {expected})",
            pixels.len()
        ));
    }

    let mut frame = Vec::with_capacity(HEADER_SIZE + expected + TRAILER_SIZE);
    let flagged_height = (HEIGHT as u16) | 0x8000;
    frame.extend_from_slice(&[
        0xFF,
        0x00,
        0x00,
        0x00,
        0x00,
        (WIDTH / 16) as u8,
        (flagged_height >> 8) as u8,
        flagged_height as u8,
    ]);
    frame.extend_from_slice(pixels);
    frame.extend_from_slice(&[0xFF, 0xC0, 0, 0, 0, 0, 0, 0]);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_native_frame_envelope() {
        let pixels = vec![0_u8; WIDTH * HEIGHT * BYTES_PER_PIXEL];
        let frame = encode(&pixels).unwrap();

        assert_eq!(frame.len(), HEADER_SIZE + pixels.len() + TRAILER_SIZE);
        assert_eq!(&frame[..8], &[0xFF, 0x00, 0, 0, 0, 23, 0x83, 0xC0]);
        assert_eq!(&frame[frame.len() - 8..], &[0xFF, 0xC0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn rejects_wrong_pixel_buffer_size() {
        assert!(encode(&[]).is_err());
    }
}
