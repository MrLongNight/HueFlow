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
    /// Send a static DTLS packet (Green, Index-based)
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
            // Default: check if config exists, run setup or stream
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
            println!("   Username: {}", config.username);
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

    // Show all discovered bridges with reachability status
    println!("Found {} bridge(s):", bridges.len());
    for (i, bridge) in bridges.iter().enumerate() {
        // First bridge in list is reachable (sorted by reachability)
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

    // Let user select or enter manually
    let mut options: Vec<String> = bridges
        .iter()
        .map(|b| format!("{} ({})", b.ip, &b.id[..8.min(b.id.len())]))
        .collect();
    options.push("Enter IP manually...".to_string());

    let selection = Select::new("Select your Hue Bridge:", options).prompt()?;

    let bridge_ip = if selection == "Enter IP manually..." {
        inquire::Text::new("Enter your Hue Bridge IP address:").prompt()?
    } else {
        // Extract IP from selection
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
        .map(|g| format!("{} ({} lights)", g.name, g.lights.len()))
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
        "   Selected group: {} with {} lights",
        selected_group.name,
        selected_group.lights.len()
    );
    println!();
    println!("🚀 Run 'hueflow' or 'hueflow run' to start the entertainment stream!");

    Ok(())
}

async fn run_stream(effect_name: &str) -> Result<()> {
    let config = load_config().context("No configuration found. Run 'hueflow setup' first.")?;

    println!("🎭 Loading entertainment group...");
    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id)
        .context("Configured entertainment group not found")?;

    println!(
        "   Group: {} with {} lights",
        group.name,
        group.lights.len()
    );

    // Debug Light IDs
    println!("   Light IDs in group:");
    for light in &group.lights {
        // print!("{}", light.id); if ...
        println!(
            "     - ID: '{}' at ({:.2}, {:.2}, {:.2})",
            light.id, light.x, light.y, light.z
        );
    }

    // Check if IDs are parseable as u8
    let unparseable = group
        .lights
        .iter()
        .filter(|l| l.id.parse::<u8>().is_err())
        .count();
    if unparseable > 0 {
        println!("⚠️  WARNING: {} lights have IDs that represent non-u8 values! These will be ignored by the current effect implementation.", unparseable);
    }

    println!("📡 Activating stream mode...");

    set_stream_active(&config, &group.id, true).await?;

    println!("🔒 Establishing DTLS connection...");
    let streamer = HueStreamer::connect(&config.bridge_ip, &config.username, &config.client_key)
        .context("Failed to establish DTLS connection")?;

    println!("✅ Connected!");
    println!();
    println!("🎨 Starting {} effect...", effect_name);
    println!("   Press Ctrl+C to stop");
    println!();

    // Create channel for light states
    let (tx, rx) = mpsc::channel::<Vec<LightState>>(16);

    // Spawn streaming task
    let _stream_handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(run_stream_loop(streamer, rx));
    });

    // Create effect
    let mut effect: Box<dyn LightEffect> = match effect_name {
        "pulse" => Box::new(PulseEffect::new((255, 100, 50))),
        _ => Box::new(MultiBandEffect::new()),
    };

    // Convert LightNodes to our format
    let nodes = group.lights.clone();

    // Simulation loop with mock audio data
    let mut tick_interval = interval(Duration::from_millis(50)); // 20 FPS
    let mut phase: f32 = 0.0;

    loop {
        tick_interval.tick().await;

        // Generate mock audio spectrum (simulated bass/mids/highs)
        phase += 0.1;
        let mock_audio = hue_flow_core::audio_interface::AudioSpectrum {
            bass: (phase.sin() * 0.5 + 0.5).abs(),
            mids: ((phase * 1.5).sin() * 0.5 + 0.5).abs(),
            highs: ((phase * 2.0).sin() * 0.5 + 0.5).abs(),
            energy: 1.0, // Full brightness for testing
        };

        // Update effect
        let colors = effect.update(&mock_audio, &nodes);

        // Convert to LightState
        let states: Vec<LightState> = colors
            .into_iter()
            .map(|(id, (r, g, b))| LightState { id, r, g, b })
            .collect();

        // Debug output (1% chance or every X frames) - simple log
        if phase.fract() < 0.1 && !states.is_empty() {
            let first = &states[0];
            println!(
                "Values: Bass={:.2} -> Light {}: RGB({},{},{})",
                mock_audio.bass, first.id, first.r, first.g, first.b
            );
        }

        // Send to streamer
        if tx.send(states).await.is_err() {
            break; // Channel closed
        }
    }

    // Cleanup
    set_stream_active(&config, &group.id, false).await.ok();

    Ok(())
}

async fn run_test() -> Result<()> {
    let config = load_config().context("No configuration found. Run 'hueflow setup' first.")?;
    println!("🧪 Testing connection to Bridge at {}...", config.bridge_ip);
    println!("   Using Username: {}", config.username);

    println!("📂 Fetching entertainment groups...");
    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id);

    if let Some(group) = group {
        println!("✅ Found Entertainment Group: {}", group.name);
        println!("   Contains {} lights", group.lights.len());

        if let Some(light) = group.lights.first() {
            println!(
                "🔦 Flashing Light ID {} (at {:.2}, {:.2}, {:.2})...",
                light.id, light.x, light.y, light.z
            );
            flash_light(&config, &light.id).await?;
            println!("✅ Light flashed successfully!");
            println!("   (This proves REST API connectivity and permissions are working)");
        } else {
            println!("❌ Group has no lights!");
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

    println!("🧪 Static DTLS Test (GREEN Pattern) + Monitor...");
    let groups = get_entertainment_groups(&config).await?;
    let group = groups
        .iter()
        .find(|g| g.id == config.entertainment_group_id)
        .context("Group not found")?;

    println!("📡 Activating stream (Resetting)...");
    set_stream_active(&config, &group.id, false).await.ok();
    tokio::time::sleep(Duration::from_millis(1000)).await;
    set_stream_active(&config, &group.id, true).await?;

    // Spawn Monitor Task
    let group_id = group.id.clone();
    let config_monitor = config_arc.clone();

    let monitor_handle = tokio::spawn(async move {
        // Use native-tls by using simple builder
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        loop {
            let url = format!(
                "https://{}/api/{}/groups/{}",
                config_monitor.bridge_ip, config_monitor.username, group_id
            );
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(stream) = json.get("stream") {
                        println!("   [Monitor] Stream Status: {}", stream);
                    } else {
                        // println!("   [Monitor] 'stream' field missing or error");
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    println!("🔒 Connecting DTLS...");
    let mut streamer =
        HueStreamer::connect(&config.bridge_ip, &config.username, &config.client_key)?;

    let mut light_map = HashMap::new();
    // Try sending Channel Index (0..N) instead of Light ID
    for (i, _node) in group.lights.iter().enumerate() {
        light_map.insert(i as u8, (0, 255, 0)); // GREEN
    }

    println!("🎨 Sending GREEN frames (Channel Index Mode) for 10 seconds...");
    // Print the FIRST packet bytes for debugging
    let packet = hue_flow_core::stream::protocol::create_message("area", &light_map);
    println!("📦 Packet Hex Dump: {:02X?}", packet);

    let mut tick_interval = interval(Duration::from_millis(100));
    for _ in 0..100 {
        tick_interval.tick().await;
        let packet = hue_flow_core::stream::protocol::create_message("area", &light_map);
        streamer.write_all(&packet)?;
    }

    monitor_handle.abort();
    set_stream_active(&config, &group.id, false).await.ok();
    println!("✅ Test finished.");
    Ok(())
}
