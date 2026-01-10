use crate::stream::dtls::HueStreamer;
use crate::stream::protocol;
use crate::models::LightState;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

pub async fn run_stream_loop(
    mut streamer: HueStreamer,
    mut receiver: mpsc::Receiver<Vec<LightState>>,
) {
    let target_frame_time = Duration::from_millis(20); // 50 FPS
    let mut last_frame_time = Instant::now();
    let area_id = "hue_stream_area"; // Placeholder, not used in protocol.rs

    // We keep the current state of lights to resend if no new data comes (keep-alive)?
    // Or just stream what we get?
    // "Sende Frame. Warte min. 20ms (max 50fps)."
    // "Implementiere Keep-Alive Logik".
    // Keep-Alive in Hue usually means sending frames continuously even if nothing changes,
    // because the bridge will stop streaming mode if it receives nothing for a few seconds.

    let mut current_lights: HashMap<u8, (u8, u8, u8)> = HashMap::new();

    loop {
        let deadline = last_frame_time + target_frame_time;

        // Wait for new data or timeout
        // If we have data, update state.
        // If timeout, just send current state (Keep-Alive).

        let timeout = tokio::time::sleep_until(deadline);
        tokio::select! {
            res = receiver.recv() => {
                match res {
                    Some(updates) => {
                        // Update current state
                        for light in updates {
                            current_lights.insert(light.id, (light.r, light.g, light.b));
                        }
                    }
                    None => {
                        // Channel closed
                        break;
                    }
                }
            }
            _ = timeout => {
                // Time to send a frame (or keep-alive)
            }
        }

        // Check if we need to send
        let now = Instant::now();
        if now >= last_frame_time + target_frame_time {
             // Create message
             if !current_lights.is_empty() {
                 let msg = protocol::create_message(area_id, &current_lights);

                 // Sending is blocking IO on the streamer, so we should spawn_blocking or accept blocking?
                 // Since it's UDP send, it's very fast. I'll accept blocking for now as it simplifies things
                 // and avoids moving streamer into a closure constantly.
                 // However, calling blocking function in async context is discouraged.
                 // But since HueStreamer is not Clone, I can't easily move it in and out of spawn_blocking unless I wrap it in Arc<Mutex> or similar.
                 // Given the constraints and likely usage, direct call is probably intended for this "MVP".

                 match streamer.write_all(&msg) {
                     Ok(_) => {},
                     Err(e) => {
                         // Error log is important
                         eprintln!("Error sending Hue stream frame: {}", e);
                         // Reconnect logic is optional for MVP.
                     }
                 }
             }
             last_frame_time = now;
        }
    }
}
