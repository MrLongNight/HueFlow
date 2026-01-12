# HueFlow

**Philips Hue Entertainment Streaming Library for Rust**

A low-latency library for streaming real-time lighting effects to Philips Hue lights via the Entertainment API.

## Features

- ✅ Bridge Discovery (mDNS + Cloud)
- ✅ DTLS 1.2 PSK Streaming (Port 2100)
- ✅ v2 API Entertainment Configuration
- ✅ 50-60 FPS Frame Rate
- ✅ Multi-channel RGB Control
- ✅ Audio-reactive Effects

---

## 📚 Official Philips Hue Documentation

| Resource | Description |
|----------|-------------|
| [Hue Developer Portal](https://developers.meethue.com/) | Main developer hub |
| [Entertainment API Guide](https://developers.meethue.com/develop/hue-entertainment/) | Overview & concepts |
| [Streaming API Reference](https://developers.meethue.com/develop/hue-entertainment/hue-entertainment-api/) | DTLS protocol details |
| [API v2 Reference](https://developers.meethue.com/develop/hue-api-v2/api-reference/) | REST API endpoints |
| [EDK (C++ SDK)](https://developers.meethue.com/develop/hue-entertainment/philips-hue-entertainment-development-kit-edk/) | Official C++ SDK with effect engine |
| [Light Effects Guide](https://developers.meethue.com/develop/hue-entertainment/philips-hue-entertainment-development-kit-edk/light-effects-creation/) | Effect creation best practices |

---

## Quick Start

```bash
# Setup (requires Link Button press)
cargo run --package hue_flow_cli -- setup

# Run with multiband effect
cargo run --package hue_flow_cli -- run

# Test with static red color
cargo run --package hue_flow_cli -- static
```

---

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

// 5. Send frames (50-60 FPS recommended)
let mut light_map = HashMap::new();
light_map.insert(0u8, (255, 0, 0)); // Channel 0 = Red
let packet = create_message(&group.id, &light_map);
streamer.write_all(&packet)?;

// 6. Stop stream
set_stream_active(&config, &group.id, false).await?;
```

---

## DTLS Message Format

| Field | Bytes | Description |
|-------|-------|-------------|
| Protocol | 9 | `"HueStream"` |
| Version | 2 | `0x02, 0x00` (v2.0) |
| Sequence | 1 | Incrementing (ignored by bridge) |
| Reserved | 2 | `0x00, 0x00` |
| Color Space | 1 | `0x00` = RGB, `0x01` = XY+Brightness |
| Reserved | 1 | `0x00` |
| **UUID** | **36** | Entertainment Configuration ID (ASCII) |
| Channels | 7×N | `channel_id (u8) + R (u16) + G (u16) + B (u16)` |

**Max 20 channels per message. All 16-bit values are Big Endian.**

---

## 🎨 Creating Custom Effects

### Effect Trait

```rust
pub trait LightEffect: Send + Sync {
    fn update(&mut self, audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)>;
}
```

### Effect Types (from Hue EDK)

| Type | Description | Use Case |
|------|-------------|----------|
| **AreaEffect** | Colors all lights in a spatial region | Hit indicators, explosions |
| **MultiChannelEffect** | Distributes N virtual channels across lights | Music visualization, surround effects |
| **LightSourceEffect** | Virtual point light with falloff | Moving lights, spotlights |
| **LightIteratorEffect** | Sequential animation across lights | Chaser, running lights |

### Example: Strobe Effect

```rust
pub struct StrobeEffect {
    frequency_hz: f32,
    color: (u8, u8, u8),
    phase: f32,
}

impl StrobeEffect {
    pub fn new(frequency_hz: f32, color: (u8, u8, u8)) -> Self {
        Self { frequency_hz, color, phase: 0.0 }
    }
}

impl LightEffect for StrobeEffect {
    fn update(&mut self, audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)> {
        self.phase += self.frequency_hz / 50.0; // Assuming 50 FPS
        let on = (self.phase.sin() > 0.0);
        
        let color = if on { self.color } else { (0, 0, 0) };
        
        nodes.iter()
            .map(|n| (n.channel_id, color))
            .collect()
    }
}
```

### Example: Spatial Gradient

```rust
pub struct SpatialGradient {
    left_color: (u8, u8, u8),
    right_color: (u8, u8, u8),
}

impl LightEffect for SpatialGradient {
    fn update(&mut self, _audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)> {
        nodes.iter().map(|n| {
            // x ranges from -1.0 (left) to 1.0 (right)
            let t = ((n.x + 1.0) / 2.0).clamp(0.0, 1.0);
            let r = lerp(self.left_color.0, self.right_color.0, t);
            let g = lerp(self.left_color.1, self.right_color.1, t);
            let b = lerp(self.left_color.2, self.right_color.2, t);
            (n.channel_id, (r, g, b))
        }).collect()
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    ((a as f32) * (1.0 - t) + (b as f32) * t) as u8
}
```

---

## 🎛️ Granular Effect Parameters

### AudioSpectrum Fields

| Field | Range | Description |
|-------|-------|-------------|
| `bass` | 0.0 - 1.0 | Low frequencies (20-200 Hz) |
| `mids` | 0.0 - 1.0 | Mid frequencies (200-2000 Hz) |
| `highs` | 0.0 - 1.0 | High frequencies (2000-20000 Hz) |
| `energy` | 0.0 - 1.0 | Overall loudness/energy |

### LightNode Fields

| Field | Range | Description |
|-------|-------|-------------|
| `channel_id` | 0-19 | Streaming channel (use in DTLS messages) |
| `id` | String | REST API light ID (for state queries) |
| `x` | -1.0 to 1.0 | Left (-1) to Right (1) |
| `y` | -1.0 to 1.0 | Back (-1) to Front (1) |
| `z` | -1.0 to 1.0 | Below (-1) to Above (1) |

### Suggested Effect Parameters

```rust
pub struct AdvancedPulseEffect {
    // Color
    pub hue: f32,           // 0.0 - 360.0 degrees
    pub saturation: f32,    // 0.0 - 1.0
    pub min_brightness: f32, // 0.0 - 1.0
    pub max_brightness: f32, // 0.0 - 1.0
    
    // Dynamics
    pub attack_ms: u32,      // Rise time
    pub decay_ms: u32,       // Fall time
    pub threshold: f32,      // Min audio level to trigger
    pub sensitivity: f32,    // Audio multiplier
    
    // Spatial
    pub spread: f32,         // How much effect spreads across channels
    pub delay_per_channel_ms: u32, // Wave propagation delay
}
```

---

## ⚡ Performance Tips

| Tip | Reason |
|-----|--------|
| Stream at 50-60 FPS to app | Bridge sends 25 Hz to Zigbee; higher rate compensates UDP loss |
| Keep sending even if unchanged | Connection times out after 10s inactivity |
| Use RGB (0x00), not XY | RGB gives widest color range per bulb |
| Batch all channels in one message | Reduces network overhead |
| Avoid frequencies > 12.5 Hz | Fastest perceptible effect rate is ~12 Hz |

---

## ⚠️ Safety Guidelines

> **EPILEPSY WARNING:** Strobe effects can trigger seizures. Keep rapid brightness changes below **5 Hz**.

| Avoid | Do Instead |
|-------|------------|
| Sudden full brightness changes | Fade over 100ms minimum |
| Continuous strobing | Use sparingly for impact |
| Flashing peripheral lights | Peripheral vision is very sensitive |
| Mismatched colors with screen | Match lamp color to content |

---

## 🔧 Color Space Options

### RGB (0x00) - Default
- Simplest to use
- Widest color range per bulb
- Colors may differ between bulb types (gamuts)

### XY + Brightness (0x01)
- Hardware-independent color
- Consistent across different bulb gamuts
- Better for matching screen colors

```rust
// XY format in message:
// X: u16 (0x0000 = 0.0, 0xFFFF = 1.0)
// Y: u16 (0x0000 = 0.0, 0xFFFF = 1.0)
// Brightness: u16 (0x0000 = off, 0xFFFF = max)

// Conversion from RGB to XY (simplified):
fn rgb_to_xy(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let x = 0.4124*r + 0.3576*g + 0.1805*b;
    let y = 0.2126*r + 0.7152*g + 0.0722*b;
    let z = 0.0193*r + 0.1192*g + 0.9505*b;
    
    let sum = x + y + z;
    if sum == 0.0 { return (0.0, 0.0, 0.0); }
    
    (x / sum, y / sum, y) // (x_coord, y_coord, brightness)
}
```

---

## 📦 Requirements

- Philips Hue Bridge v2 (firmware ≥1948086000)
- Color-capable Hue lights
- Entertainment Area configured in Hue App
- OpenSSL (for DTLS)

## License

MIT
