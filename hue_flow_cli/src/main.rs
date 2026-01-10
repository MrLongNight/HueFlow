use anyhow::{Context, Result};
use hue_flow_core::{
    api::{
        client::HueClient,
        discovery::discover_bridge,
        groups::{get_entertainment_groups, set_stream_active},
    },
    engine::EntertainmentEngine,
    models::{HueConfig},
    stream::{dtls::HueStreamer, manager::run_stream_loop},
    effects::{MultiBandEffect},
};
use std::fs;
use std::path::Path;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use inquire::{Select, Text, Confirm};

mod audio_input;
use audio_input::AudioInput;

const CONFIG_FILE: &str = "config.json";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting Hue Flow CLI...");

    // Step 1: HueConfig load or Wizard
    let config = load_or_create_config().await?;
    info!("Configuration loaded for bridge: {}", config.ip);

    // Get Group Info (lights) for the selected group
    let groups = get_entertainment_groups(&config).await
        .context("Failed to fetch entertainment groups")?;

    let group_id = config.entertainment_group_id.clone().ok_or_else(|| anyhow::anyhow!("No entertainment group ID in config"))?;

    let group = groups.iter().find(|g| g.id == group_id)
        .ok_or_else(|| anyhow::anyhow!("Selected group ID not found on bridge"))?;

    info!("Using Entertainment Group: {} ({} lights)", group.name, group.lights.len());

    // Step 2: Setup Channels
    let (audio_tx, audio_rx) = mpsc::channel(100);
    let (light_tx, light_rx) = mpsc::channel(100);

    // Step 3: Spawn AudioInput
    // We keep audio_input alive by holding it in main
    let (_audio_input, sample_rate) = AudioInput::new(audio_tx).context("Failed to start audio input")?;
    info!("Audio input started at {} Hz.", sample_rate);

    // Step 4: Spawn EntertainmentEngine
    let engine = EntertainmentEngine::new(
        audio_rx,
        light_tx,
        group.lights.clone(),
        Box::new(MultiBandEffect), // Use MultiBandEffect
        sample_rate,
    );

    tokio::spawn(async move {
        engine.run().await;
    });
    info!("Entertainment Engine started.");

    // Step 5: Activate Stream & Spawn HueStreamer
    info!("Activating stream on bridge...");
    set_stream_active(&config, &group_id, true).await
        .context("Failed to activate stream")?;

    info!("Connecting to DTLS stream...");
    let streamer = HueStreamer::connect(&config.ip, &config.username, &config.client_key)
        .context("Failed to connect to DTLS stream")?;

    let _stream_handle = tokio::spawn(async move {
        run_stream_loop(streamer, light_rx).await;
    });
    info!("Stream loop started. Press Ctrl+C to stop.");

    // Step 6: Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received.");

    // Shutdown
    info!("Deactivating stream...");
    if let Err(e) = set_stream_active(&config, &group_id, false).await {
        error!("Failed to deactivate stream: {}", e);
    } else {
        info!("Stream deactivated.");
    }

    Ok(())
}

async fn load_or_create_config() -> Result<HueConfig> {
    if Path::new(CONFIG_FILE).exists() {
        let content = fs::read_to_string(CONFIG_FILE)?;
        let config: HueConfig = serde_json::from_str(&content)?;
        return Ok(config);
    }

    info!("No configuration found. Starting setup wizard...");

    // 1. Discover
    info!("Discovering Hue Bridge...");
    let ip = match discover_bridge().await {
        Ok(ip) => {
            info!("Found bridge at {}", ip);
            ip
        },
        Err(_) => {
            warn!("Auto-discovery failed.");
            Text::new("Enter Bridge IP manually:").prompt()?
        }
    };

    // 2. Register
    println!("Please press the Link Button on your Hue Bridge now.");
    let mut config = loop {
        if Confirm::new("Have you pressed the button?").with_default(true).prompt()? {
            match HueClient::register_user(&ip, "hue_flow_cli").await {
                Ok(cfg) => break cfg,
                Err(e) => {
                    error!("Registration failed: {}. Try again?", e);
                    if !Confirm::new("Try again?").with_default(true).prompt()? {
                        return Err(anyhow::anyhow!("Setup aborted by user"));
                    }
                }
            }
        }
    };

    info!("User registered: {}", config.username);

    // 3. Select Group
    let groups = get_entertainment_groups(&config).await?;
    if groups.is_empty() {
        return Err(anyhow::anyhow!("No entertainment groups found on bridge. Please create one in the Hue App."));
    }

    let group_options: Vec<String> = groups.iter().map(|g| format!("{} ({})", g.name, g.id)).collect();
    let selection = Select::new("Select Entertainment Group:", group_options.clone()).prompt()?;

    // Find selected group ID
    // inquire returns the string, we need to map back or use index.
    // simpler: parse/find
    let selected_group = groups.iter().find(|g| format!("{} ({})", g.name, g.id) == selection).unwrap();

    config.entertainment_group_id = Some(selected_group.id.clone());

    // 4. Save
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(CONFIG_FILE, json)?;
    info!("Configuration saved to {}", CONFIG_FILE);

    Ok(config)
}
