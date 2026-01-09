use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};

static SEQUENCE_ID: AtomicU8 = AtomicU8::new(0);

pub fn create_message(_area_id: &str, lights: &HashMap<u8, (u8, u8, u8)>) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(16 + lights.len() * 7);

    // Header "HueStream"
    buffer.extend_from_slice(b"HueStream");

    // Version 2.0 (0x02, 0x00)
    buffer.extend_from_slice(&[0x02, 0x00]);

    // Sequence ID
    let seq = SEQUENCE_ID.fetch_add(1, Ordering::SeqCst);
    buffer.push(seq);

    // Reserved (0x00, 0x00)
    buffer.extend_from_slice(&[0x00, 0x00]);

    // Color Space (0x00 = RGB)
    buffer.push(0x00);

    // Reserved (0x00)
    buffer.push(0x00);

    // Sort lights by ID to have deterministic output
    let mut sorted_lights: Vec<_> = lights.iter().collect();
    sorted_lights.sort_by_key(|(id, _)| *id);

    for (id, (r, g, b)) in sorted_lights {
        buffer.push(*id);
        // Scale 8-bit (0-255) to 16-bit (0-65535)
        // Formula: val * 257 (since 255 * 257 = 65535)
        let r16 = (*r as u16) * 257;
        let g16 = (*g as u16) * 257;
        let b16 = (*b as u16) * 257;

        buffer.extend_from_slice(&r16.to_be_bytes());
        buffer.extend_from_slice(&g16.to_be_bytes());
        buffer.extend_from_slice(&b16.to_be_bytes());
    }

    buffer
}
