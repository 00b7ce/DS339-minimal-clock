// Full packets between chunks keep the device's frame stream continuous.
// Only the final short transfer and explicit zero-length transfer end a frame.
const CHUNK_SIZE: usize = 65_536;

pub(crate) fn write_frame(
    data: &[u8],
    mut transfer: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    for (index, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
        let offset = index * CHUNK_SIZE;
        transfer(chunk).map_err(|error| {
            format!(
                "frame data transfer (offset {offset}, {} bytes, endpoint 0x04): {error}",
                chunk.len()
            )
        })?;
    }
    transfer(&[])
        .map_err(|error| format!("end-of-frame transfer (0 bytes, endpoint 0x04): {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_native_frame_in_order_then_one_end_marker() {
        let data: Vec<_> = (0..1_082_896).map(|index| index as u8).collect();
        let mut lengths = Vec::new();
        let mut received = Vec::new();
        write_frame(&data, |chunk| {
            lengths.push(chunk.len());
            received.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
        assert_eq!(received, data);
        assert_eq!(&lengths[..16], &[CHUNK_SIZE; 16]);
        assert_eq!(&lengths[16..], &[34_320, 0]);
    }

    #[test]
    fn stops_after_failed_chunk_without_replaying_or_ending_frame() {
        let mut calls = 0;
        let error = write_frame(&vec![0; CHUNK_SIZE * 3], |_| {
            calls += 1;
            if calls == 2 {
                Err("timeout (code -116)".to_owned())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(calls, 2);
        assert!(error.contains("offset 65536, 65536 bytes"));
        assert!(error.contains("code -116"));
    }

    #[test]
    fn reports_end_marker_failure_separately() {
        let error = write_frame(&[1], |chunk| {
            if chunk.is_empty() {
                Err("timeout".to_owned())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(error.contains("end-of-frame transfer (0 bytes"));
    }
}
