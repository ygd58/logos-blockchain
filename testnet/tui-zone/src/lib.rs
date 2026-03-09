mod state;

use std::{fs, io::Write as _, path::Path};

use clap::Parser;
use lb_core::{
    codec::DeserializeOp as _,
    mantle::{NoteId, ops::channel::ChannelId},
};
use lb_key_management_system_service::keys::{ED25519_SECRET_KEY_SIZE, Ed25519Key, ZkKey};
use lb_zone_sdk::sequencer::{SequencerCheckpoint, ZoneSequencer};
use reqwest::Url;

use crate::state::{Address, State, StateTransition};

#[derive(Parser, Debug)]
#[command(about = "Terminal UI zone sequencer - publish text inscriptions")]
pub struct InscribeArgs {
    /// Logos blockchain node HTTP endpoint
    #[arg(long, default_value = "http://localhost:8080", env = "NODE_URL")]
    node_url: String,

    /// Path to the signing key file (created if it doesn't exist)
    #[arg(long, default_value = "sequencer.key", env = "KEY_PATH")]
    key_path: String,

    /// Path to the checkpoint file for crash recovery
    #[arg(long, default_value = "sequencer.checkpoint", env = "CHECKPOINT_PATH")]
    checkpoint_path: String,
}

fn parse_zk_key(s: &str) -> Result<ZkKey, String> {
    let bytes = hex::decode(s).map_err(|e| format!("invalid hex: {e}"))?;
    ZkKey::from_bytes(&bytes).map_err(|e| format!("invalid zk key: {e}"))
}

fn parse_note_id(s: &str) -> Result<NoteId, String> {
    let bytes = hex::decode(s).map_err(|e| format!("invalid hex: {e}"))?;
    NoteId::from_bytes(&bytes).map_err(|e| format!("invalid note id: {e}"))
}

fn parse_address(s: &str) -> Result<Address, String> {
    let bytes = hex::decode(s).map_err(|e| format!("invalid hex: {e}"))?;
    Address::try_from(bytes.as_slice())
        .map_err(|()| format!("expected 32 bytes, got {}", bytes.len()))
}

fn save_checkpoint(path: &Path, checkpoint: &SequencerCheckpoint) {
    let data = serde_json::to_vec(checkpoint).expect("failed to serialize checkpoint");
    fs::write(path, data).expect("failed to write checkpoint file");
}

fn load_checkpoint(path: &Path) -> Option<SequencerCheckpoint> {
    if !path.exists() {
        return None;
    }
    let data = fs::read(path).expect("failed to read checkpoint file");
    Some(serde_json::from_slice(&data).expect("failed to deserialize checkpoint"))
}

fn load_or_create_signing_key(path: &Path) -> Ed25519Key {
    if path.exists() {
        let key_bytes = fs::read(path).expect("failed to read key file");
        assert!(
            key_bytes.len() == ED25519_SECRET_KEY_SIZE,
            "invalid key file: expected {} bytes, got {}",
            ED25519_SECRET_KEY_SIZE,
            key_bytes.len()
        );
        let key_array: [u8; ED25519_SECRET_KEY_SIZE] =
            key_bytes.try_into().expect("length already checked");
        Ed25519Key::from_bytes(&key_array)
    } else {
        let mut key_bytes = [0u8; ED25519_SECRET_KEY_SIZE];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key_bytes);
        fs::write(path, key_bytes).expect("failed to write key file");
        Ed25519Key::from_bytes(&key_bytes)
    }
}

pub async fn run(args: InscribeArgs) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let node_url: Url = args.node_url.parse().expect("invalid node URL");
    let signing_key = load_or_create_signing_key(Path::new(&args.key_path));
    let channel_id = ChannelId::from(signing_key.public_key().to_bytes());

    println!("TUI Zone Sequencer");
    println!("  Node:       {node_url}");
    println!("  Key:        {}", args.key_path);
    println!("  Channel ID: {}", hex::encode(channel_id.as_ref()));
    println!();

    let checkpoint_path = Path::new(&args.checkpoint_path);
    let checkpoint = load_checkpoint(checkpoint_path);
    if checkpoint.is_some() {
        println!("  Restored checkpoint from {}", args.checkpoint_path);
    }

    let sequencer =
        ZoneSequencer::init(channel_id, signing_key, node_url.clone(), None, checkpoint);

    let mut state = State::new(); // TODO: recover from historical inscriptions

    println!();
    println!("Commands:");
    println!("  /inscribe <message>                                    Publish a text inscription");
    println!(
        "  /deposit <amount> <input-note-key> <input-note-id> <recipient-address>  Deposit to channel"
    );
    println!("Press Ctrl-D or type an empty line to exit.");
    println!();

    let stdin = std::io::stdin();
    let mut line = String::new();

    loop {
        print!("> ");
        std::io::stdout().flush().expect("failed to flush stdout");

        line.clear();
        let bytes_read = stdin.read_line(&mut line).expect("failed to read line");

        if bytes_read == 0 {
            println!();
            break;
        }

        let input = line.trim_end();
        if input.is_empty() {
            break;
        }

        if let Some(msg) = input.strip_prefix("/inscribe ") {
            if msg.is_empty() {
                println!("  usage: /inscribe <message>");
                continue;
            }
            match sequencer.publish(msg.as_bytes().to_vec()).await {
                Ok(result) => {
                    let tx_hash: [u8; 32] = result.inscription_id.into();
                    println!("  published: {}", hex::encode(tx_hash));
                    save_checkpoint(checkpoint_path, &result.checkpoint);
                }
                Err(e) => {
                    println!("  error: {e}");
                }
            }
        } else if let Some(rest) = input.strip_prefix("/deposit ") {
            // TODO: refactor to avoid duplication with "/inscribe"
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 4 {
                println!(
                    "  usage: /deposit <amount> <input-note-key> <input-note-id> <recipient-address>"
                );
                continue;
            }
            let amount = match parts[0].parse::<u64>() {
                Ok(v) => v,
                Err(e) => {
                    println!("  invalid amount: {e}");
                    continue;
                }
            };
            let input_note_key = match parse_zk_key(parts[1]) {
                Ok(key) => key,
                Err(e) => {
                    println!("  invalid input-note-key: {e}");
                    continue;
                }
            };
            let input_note_id = match parse_note_id(parts[2]) {
                Ok(id) => id,
                Err(e) => {
                    println!("  invalid input-note-id: {e}");
                    continue;
                }
            };
            let recipient_address = match parse_address(parts[3]) {
                Ok(addr) => addr,
                Err(e) => {
                    println!("  invalid recipient-address: {e}");
                    continue;
                }
            };

            let transition = StateTransition::Mint {
                amount,
                address: recipient_address,
            };
            let inscription_msg =
                serde_json::to_vec(&transition).expect("failed to serialize state transition");
            let deposit_metadata = recipient_address.as_bytes().to_vec();

            match sequencer
                .deposit(
                    inscription_msg,
                    amount,
                    deposit_metadata,
                    input_note_key,
                    input_note_id,
                )
                .await
            {
                Ok(result) => {
                    let tx_hash: [u8; 32] = result.inscription_id.into();
                    println!("  depositted: {}", hex::encode(tx_hash));
                    save_checkpoint(checkpoint_path, &result.checkpoint);

                    state.apply(&transition);
                    let balance = state
                        .balance(&recipient_address)
                        .expect("account must exist");
                    println!("  minted {amount} tokens for recipient {recipient_address}");
                    println!("  zone account balance: {balance}");
                }
                Err(e) => {
                    println!("  error: {e}");
                }
            }
        } else {
            println!("  unknown command. try /inscribe or /deposit");
        }
    }

    println!("Goodbye!");
}
