use {
    crate::bench::*,
    crate::bench_tps_client::*,
    crate::send_back::*,
    solana_client::{
        connection_cache,
        rpc_client::RpcClient,
        // tpu_client::{TpuClient, TpuClientConfig},
    },
    solana_core::gen_keys::GenKeys,
    solana_sdk::{
        clock::{DEFAULT_MS_PER_SLOT, DEFAULT_S_PER_SLOT, MAX_PROCESSING_AGE},
        commitment_config::CommitmentConfig,
        hash::Hash,
        // instruction::{AccountMeta, Instruction},
        // message::Message,
        // native_token::Sol,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair, Signer},
        // system_instruction, system_transaction,
        // timing::{duration_as_ms, duration_as_s, duration_as_us, timestamp},
        // transaction::Transaction,
    },
    std::{
        // collections::{HashSet, VecDeque},
        // process::exit,
        sync::{
            // atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering},
            mpsc,
            Arc,
        },
        thread,
        thread::sleep,
        time::{Duration, Instant},
    },
};

pub mod bench;
pub mod bench_tps_client;
pub mod blockhash;
pub mod send_back;

fn main() {
    let json_rpc_url = "https://api.devnet.solana.com";
    let client = Arc::new(RpcClient::new_with_commitment(
        json_rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    let sender_path = "/Users/harshasomisetty/code/rust_nuke/sender.json";
    let receiver_path = "/Users/harshasomisetty/code/rust_nuke/receiver.json";
    let third_path = "/Users/harshasomisetty/code/rust_nuke/third.json";

    let sender_keypair = read_keypair_file(&sender_path).unwrap();
    let receiver_keypair = read_keypair_file(&receiver_path).unwrap();
    let third_keypair = read_keypair_file(&third_path).unwrap();

    let final_keypair = third_keypair;
    let final_keypair_balance = client.get_balance(&final_keypair.pubkey()).unwrap_or(0);
    println!(
        "final key {} and bal: {}",
        final_keypair.pubkey(),
        final_keypair_balance
    );

    let funded_keypairs = generate_and_fund_keypairs(client, &final_keypair, 6, 200000);

    let client = Arc::new(RpcClient::new_with_commitment(
        json_rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));
    let return_funds = defund_keypairs(client, &final_keypair, 6);
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn it_works() {
    //     let result = stuff();
    //     assert_eq!(result, 4);
    // }
}
