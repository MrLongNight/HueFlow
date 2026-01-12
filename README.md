# HueFlow

**Philips Hue Entertainment Streaming Library for Rust**

A low-latency library for streaming real-time lighting effects to Philips Hue lights via the Entertainment API.

## Features

- ✅ Bridge Discovery (mDNS + Cloud)
- ✅ DTLS 1.2 PSK Streaming (Port 2100)
- ✅ v2 API Entertainment Configuration
- ✅ 50-60 FPS Frame Rate
- ✅ Multi-channel RGB Control
- ✅ Audio-reactive Effects (MultiBand, Pulse)

## Quick Start

```bash
# Setup (requires Link Button press)
cargo run --package hue_flow_cli -- setup

# Run with multiband effect
cargo run --package hue_flow_cli -- run

# Test with static red color
cargo run --package hue_flow_cli -- static
```

## Library Usage

```rust
use hue_flow_core::api::client::HueClient;
use hue_flow_core::api::groups::{get_entertainment_groups, set_stream_active};
use hue_flow_core::stream::dtls::HueStreamer;
use hue_flow_core::stream::protocol::create_message;

// 1. Get application ID (PSK Identity)
let app_id = HueClient::get_application_id(&ip, &username).await?;

// 2. Get entertainment configuration
let groups = get_entertainment_groups(&config).await?;

// 3. Start stream (v2 API)
set_stream_active(&config, &group.id, true).await?;

// 4. Connect DTLS
let mut streamer = HueStreamer::connect(&ip, &app_id, &client_key)?;

// 5. Send frames
let mut light_map = HashMap::new();
light_map.insert(0u8, (255, 0, 0)); // Channel 0 = Red
let packet = create_message(&group.id, &light_map);
streamer.write_all(&packet)?;

// 6. Stop stream
set_stream_active(&config, &group.id, false).await?;
```

## Message Format

| Field | Size | Description |
|-------|------|-------------|
| Protocol | 9 | "HueStream" |
| Version | 2 | 0x02, 0x00 |
| Sequence | 1 | Incrementing |
| Reserved | 2 | 0x00, 0x00 |
| Color Space | 1 | 0x00 = RGB |
| Reserved | 1 | 0x00 |
| **UUID** | **36** | **Entertainment Config ID** |
| Channels | 7×N | channel_id + RGB16 |

## Requirements

- Philips Hue Bridge v2 (firmware ≥1948086000)
- Color-capable Hue lights
- Entertainment Area configured in Hue App
- OpenSSL (for DTLS)

## License

MIT
