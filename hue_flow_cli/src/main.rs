use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use hue_flow_core::api::client::HueClient;
use hue_flow_core::api::discovery::discover_bridges;
use hue_flow_core::api::groups::{flash_light, get_entertainment_groups, set_stream_active};
use hue_flow_core::effects::{LightEffect, MultiBandEffect, PulseEffect};
use hue_flow_core::models::HueConfig;
use hue_flow_core::stream::dtls::HueStreamer;
use hue_flow_core::stream::manager::{run_stream_loop, LightState};
use inquire::{Confirm, Select};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

const CONFIG_FILE: &str = "hue_config.json";

#[derive(Parser)]
#[command(name = "hueflow")]
#[command(about = "HueFlow - Philips Hue Entertainment Streaming", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Setup: Discover bridge and register
    Setup,
    /// Run the entertainment stream
    Run {
        /// Effect to use: pulse or multiband
        #[arg(short, long, default_value = "multiband")]
        effect: String,
    },
    /// Show current configuration
    Config,
    /// Test connection by flashing a light
    Test,
    /// Send a static DTLS packet for debugging
    Static,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Setup) => run_setup().await,
        Some(Commands::Run { effect }) => run_stream(&effect).await,
        Some(Commands::Config) => show_config(),
        Some(Commands::Test) => run_test().await,
        Some(Commands::Static) => run_static_test().await,
        None => {
            if config_path().exists() {
                println!("🎨 HueFlow - Starting entertainment stream...");
                println!("   Use 'hueflow setup' to reconfigure");
                println!("   Use 'hueflow run --effect pulse' for pulse effect");
                println!();
                run_stream("multiband").await
            } else {
                println!("👋 Welcome to HueFlow!");
                println!("   No configuration found. Starting setup...");
                println!();
                run_setup().await
            }
        }
    }
}

fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

fn load_config() -> Result<HueConfig> {
    let content = fs::read_to_string(config_path()).context("Failed to read config file")?;
    serde_json::from_str(&content).context("Failed to parse config file")
}

fn save_config(config: &HueConfig) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(config_path(), content)?;
    Ok(())
}

fn show_config() -> Result<()> {
    match load_config() {
        Ok(config) => {
            println!("📋 Current Configuration:");
            println!("   Bridge IP: {}", config.bridge_ip);
            println!("   Username (hue-application-key): {}", config.username);
            println!(
                "   Application ID (PSK Identity): {}",
                config.application_id
            );
            println!("   Entertainment Group: {}", config.entertainment_group_id);
        }
        Err(_) => {
            println!("❌ No configuration found. Run 'hueflow setup' first.");
        }
    }
    Ok(())
}

async fn run_setup() -> Result<()> {
    println!("🔍 Discovering Hue Bridges...");
    println!("   (Checking reachability of each bridge...)");
    println!();

    let bridges = match discover_bridges().await {
        Ok(b) if !b.is_empty() => b,
        Ok(_) | Err(_) => {
            println!("⚠️  No bridges found via cloud discovery.");
            let ip = inquire::Text::new("Enter your Hue Bridge IP address manually:").prompt()?;

            println!();
            println!("📡 Using bridge at: {}", ip);
            println!();
            println!("⚠️  Please press the LINK button on your Hue Bridge, then press Enter.");
            let _ = Confirm::new("Have you pressed the link button?")
                .with_default(true)
                .prompt()?;

            return continue_registration(&ip).await;
        }
    };

    println!("Found {} bridge(s):", bridges.len());
    for (i, bridge) in bridges.iter().enumerate() {
        let status = if i == 0 {
            "✅ reachable"
        } else {
            "⚠️  may be unreachable"
        };
        println!(
            "  {}. {} (ID: {}) - {}",
            i + 1,
            bridge.ip,
            &bridge.id[..8.min(bridge.id.len())],
            status
        );
    }
    println!();

    let mut options: Vec<String> = bridges
        .iter()
        .map(|b| format!("{} ({})", b.ip, &b.id[..8.min(b.id.len())]))
        .collect();
    options.push("Enter IP manually...".to_string());

    let selection = Select::new("Select your Hue Bridge:", options).prompt()?;

    let bridge_ip = if selection == "Enter IP manually..." {
        inquire::Text::new("Enter your Hue Bridge IP address:").prompt()?
    } else {
        selection
            .split(' ')
            .next()
            .unwrap_or(&selection)
            .to_string()
    };

    println!();
    println!("📡 Using bridge at: {}", bridge_ip);
    println!();
    println!("⚠️  Please press the LINK button on your Hue Bridge, then press Enter.");
    let _ = Confirm::new("Have you pressed the link button?")
        .with_default(true)
        .prompt()?;

    continue_registration(&bridge_ip).await
}

async fn continue_registration(bridge_ip: &str) -> Result<()> {
    println!("🔐 Registering with bridge...");

    let mut config = None;
    for attempt in 1..=10 {
        match HueClient::register_user(&bridge_ip, "hueflow#device").await {
            Ok(cfg) => {
                config = Some(cfg);
                break;
            }
            Err(hue_flow_core::api::error::HueError::LinkButtonNotPressed) => {
                if attempt < 10 {
                    println!(
                        "   Link button not pressed. Retrying in 5 seconds... ({}/10)",
                        attempt
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            Err(e) => return Err(e.into()),
        }
    }

    let mut config = config.context("Failed to register after 10 attempts. Please try again.")?;
    println!("✅ Registered successfully!");
    println!("   Username: {}", config.username);

    // Fetch the application_id (required for DTLS PSK Identity)
    println!("🔑 Fetching application ID...");
    let app_id = HueClient::get_application_id(&config.bridge_ip, &config.username).await?;
    config.application_id = app_id.clone();
    println!("   Application ID: {}", app_id);

    println!();
    println!("🎭 Loading entertainment groups...");

    let groups = get_entertainment_groups(&config).await?;

    if groups.is_empty() {
        println!("❌ No entertainment groups found!");
        println!("   Please create an Entertainment Area in the Hue app first.");
        return Ok(());
    }

    let group_names: Vec<String> = groups
        .iter()
        .map(|g| format!("{} ({} channels)", g.name, g.lights.len()))
        .collect();
    let selection = Select::new("Select an entertainment group:", group_names).prompt()?;

    let selected_index = groups
        .iter()
        .position(|g| selection.starts_with(&g.name))
        .unwrap();
    let selected_group = &groups[selected_index];

    config.entertainment_group_id = selected_group.id.clone();
    save_config(&config)?;

    println!();
    println!("✅ Setup complete! Configuration saved to {}", CONFIG_FILE);
    println!(
        "   Selected group: {} with {} channels",
        selected_group.name,
        selected_group.lights.len()
    );
    println!();
    println!("🚀 Run 'hueflow' or 'hueflow run' to start the entertainment stream!");

    Ok(())
}

async fn run_stream(effect_name: &str) -> Result<()> {
    let config = load_config().context("No configuration found. Run 'hueflow setup' first.")?;

    // Validate that application_id is set
    if config.application_id.is_empty() {
        println!("⚠️  Application ID not set. Run 'hueflow setup' to reconfigure.");
        return Ok(());
    }

    println!("🎭 Loading entertainment group...");
    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id)
        .context("Configured entertainment group not found")?;

    println!(
        "   Group: {} with {} channels",
        group.name,
        group.lights.len()
    );
    println!("   Entertainment Config ID (UUID): {}", group.id);
    println!(
        "   Application ID (PSK Identity): {}",
        config.application_id
    );

    // Debug Channel Info
    println!("   Channels:");
    for light in &group.lights {
        println!(
            "     - Channel {}: at ({:.2}, {:.2}, {:.2})",
            light.channel_id, light.x, light.y, light.z
        );
    }

    println!("📡 Activating stream mode (v2 API)...");
    set_stream_active(&config, &group.id, true).await?;

    println!("🔒 Establishing DTLS connection...");
    // Use application_id as PSK Identity (NOT username!)
    let streamer = HueStreamer::connect(
        &config.bridge_ip,
        &config.application_id,
        &config.client_key,
    )
    .context("Failed to establish DTLS connection")?;

    println!("✅ Connected!");
    println!();
    println!("🎨 Starting {} effect...", effect_name);
    println!("   Press Ctrl+C to stop");
    println!();

    // Create channel for light states
    let (tx, rx) = mpsc::channel::<Vec<LightState>>(16);

    // Clone IDs for the streaming task
    let stream_area_id = group.id.clone();

    // Spawn streaming task
    let _stream_handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(run_stream_loop(streamer, rx, &stream_area_id));
    });

    // Create effect
    let mut effect: Box<dyn LightEffect> = match effect_name {
        "pulse" => Box::new(PulseEffect::new((255, 100, 50))),
        _ => Box::new(MultiBandEffect::new()),
    };

    // Convert LightNodes to our format (using channel_id!)
    let nodes = group.lights.clone();

    // Simulation loop with mock audio data
    let mut tick_interval = interval(Duration::from_millis(50)); // 20 FPS
    let mut phase: f32 = 0.0;

    loop {
        tick_interval.tick().await;

        // Generate mock audio spectrum
        phase += 0.1;
        let mock_audio = hue_flow_core::audio_interface::AudioSpectrum {
            bass: (phase.sin() * 0.5 + 0.5).abs(),
            mids: ((phase * 1.5).sin() * 0.5 + 0.5).abs(),
            highs: ((phase * 2.0).sin() * 0.5 + 0.5).abs(),
            energy: 1.0,
        };

        // Update effect
        let colors = effect.update(&mock_audio, &nodes);

        // Convert to LightState - NOTE: id is now channel_id!
        let states: Vec<LightState> = colors
            .into_iter()
            .map(|(channel_id, (r, g, b))| LightState {
                id: channel_id,
                r,
                g,
                b,
            })
            .collect();

        // Debug output
        if phase.fract() < 0.1 && !states.is_empty() {
            let first = &states[0];
            println!(
                "Values: Bass={:.2} -> Channel {}: RGB({},{},{})",
                mock_audio.bass, first.id, first.r, first.g, first.b
            );
        }

        if tx.send(states).await.is_err() {
            break;
        }
    }

    set_stream_active(&config, &group.id, false).await.ok();

    Ok(())
}

async fn run_test() -> Result<()> {
    let config = load_config().context("No configuration found. Run 'hueflow setup' first.")?;
    println!("🧪 Testing connection to Bridge at {}...", config.bridge_ip);
    println!("   Using Username: {}", config.username);
    println!("   Application ID: {}", config.application_id);

    println!("📂 Fetching entertainment groups...");
    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id);

    if let Some(group) = group {
        println!("✅ Found Entertainment Group: {}", group.name);
        println!("   Contains {} channels", group.lights.len());

        if let Some(light) = group.lights.first() {
            println!(
                "🔦 Flashing Light (Channel {} at {:.2}, {:.2}, {:.2})...",
                light.channel_id, light.x, light.y, light.z
            );
            // Note: flash_light still uses REST API light ID, not channel_id
            // This may not work correctly if the light ID isn't available
            flash_light(&config, &light.id).await?;
            println!("✅ Light flashed successfully!");
        } else {
            println!("❌ Group has no channels!");
        }
    } else {
        println!("❌ Configured entertainment group not found on bridge.");
    }
    Ok(())
}

async fn run_static_test() -> Result<()> {
    use std::collections::HashMap;
    use std::sync::Arc;
    let config = load_config()?;
    let config_arc = Arc::new(config.clone());

    if config.application_id.is_empty() {
        println!("⚠️  Application ID not set. Run 'hueflow setup' to reconfigure.");
        return Ok(());
    }

    println!("🧪 Static DTLS Test (Correct Protocol)...");
    println!(
        "   Application ID (PSK Identity): {}",
        config.application_id
    );

    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id)
        .context("Group not found")?;

    println!("   Entertainment Config UUID: {}", group.id);
    println!(
        "   Channels: {:?}",
        group
            .lights
            .iter()
            .map(|l| l.channel_id)
            .collect::<Vec<_>>()
    );

    println!("📡 Activating stream (v2 API)...");
    set_stream_active(&config, &group.id, false).await.ok();
    tokio::time::sleep(Duration::from_millis(500)).await;
    set_stream_active(&config, &group.id, true).await?;

    // Spawn Monitor Task
    let group_id = group.id.clone();
    let config_monitor = config_arc.clone();

    let monitor_handle = tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        loop {
            let url = format!(
                "https://{}/clip/v2/resource/entertainment_configuration/{}",
                config_monitor.bridge_ip, group_id
            );
            if let Ok(resp) = client
                .get(&url)
                .header("hue-application-key", &config_monitor.username)
                .send()
                .await
            {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(data) = json.get("data").and_then(|d| d.get(0)) {
                        if let Some(status) = data.get("status") {
                            println!("   [Monitor] Status: {}", status);
                        }
                        if let Some(streamer) = data.get("active_streamer") {
                            println!("   [Monitor] Active Streamer: {}", streamer);
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    println!("🔒 Connecting DTLS (with correct PSK Identity)...");
    let mut streamer = HueStreamer::connect(
        &config.bridge_ip,
        &config.application_id,
        &config.client_key,
    )?;

    // Build channel map with correct channel_ids
    let mut light_map = HashMap::new();
    for light in &group.lights {
        // Use channel_id (0, 1, 2...) and set to bright RED
        light_map.insert(light.channel_id, (255, 0, 0));
    }

    println!(
        "🎨 Sending RED frames to channels {:?} for 10 seconds...",
        group
            .lights
            .iter()
            .map(|l| l.channel_id)
            .collect::<Vec<_>>()
    );

    // Print the first packet for debugging
    let packet = hue_flow_core::stream::protocol::create_message(&group.id, &light_map);
    println!("📦 Packet Size: {} bytes", packet.len());
    println!(
        "📦 Header (first 52 bytes): {:02X?}",
        &packet[..52.min(packet.len())]
    );

    let mut tick_interval = interval(Duration::from_millis(100));
    for _ in 0..100 {
        tick_interval.tick().await;
        let packet = hue_flow_core::stream::protocol::create_message(&group.id, &light_map);
        streamer.write_all(&packet)?;
    }

    monitor_handle.abort();
    set_stream_active(&config, &group.id, false).await.ok();
    println!("✅ Test finished.");
    Ok(())
}
