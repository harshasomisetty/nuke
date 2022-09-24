use {
    crate::bench::generate_keypairs,
    crate::bench_tps_client::*,
    crate::blockhash::*,
    log::{debug, error, info, trace, warn},
    rayon::prelude::*,
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
        instruction::{AccountMeta, Instruction},
        message::Message,
        native_token::Sol,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair, Signer},
        system_instruction,
        system_transaction,
        // timing::{duration_as_ms, duration_as_s, duration_as_us, timestamp},
        transaction::Transaction,
    },
    std::{
        collections::{HashSet, VecDeque},
        fs::File,
        io::Read,
        process::exit,
        // process::exit,
        sync::{
            atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering},
            mpsc, Arc, Mutex, RwLock,
        },
        thread::{sleep, Builder, JoinHandle},
        time::{Duration, Instant},
    },
};

fn make<T: 'static + BenchTpsClient + Send + Sync>(
    client: Arc<T>,
    to_fund: &[(&Keypair, Vec<(Pubkey, u64)>)],
) {
    let blockhash = Arc::new(RwLock::new(get_latest_blockhash(client.as_ref())));

    let exit_signal = Arc::new(AtomicBool::new(false));

    let blockhash_thread = {
        let exit_signal = exit_signal.clone();
        let blockhash = blockhash.clone();
        let client = client.clone();
        // let id = id.pubkey();
        Builder::new()
            .name("solana-blockhash-poller".to_string())
            .spawn(move || {
                poll_blockhash(&exit_signal, &blockhash, &client);
            })
            .unwrap()
    };

    let mut to_fund_txs: Vec<(&Keypair, Transaction)> = to_fund
        .par_iter()
        .map(|(k, t)| {
            let instructions = system_instruction::transfer_many(&k.pubkey(), t);
            let message = Message::new(&instructions, Some(&k.pubkey()));
            (*k, Transaction::new_unsigned(message))
        })
        .collect();

    to_fund_txs.par_iter_mut().for_each(|(k, tx)| {
        tx.sign(&[*k], *blockhash.read().unwrap());
    });

    let batch: Vec<_> = to_fund_txs
        .iter()
        .map(|(_keypair, tx)| tx.clone())
        .collect();

    println!("sending batch");
    client.send_batch(batch).expect("transfer");
}

pub fn defund_keypairs<T: 'static + BenchTpsClient + Send + Sync>(
    client: Arc<T>,
    funding_key: &Keypair,
    keypair_count: usize,
) {
    let blockhash = Arc::new(RwLock::new(get_latest_blockhash(client.as_ref())));

    let exit_signal = Arc::new(AtomicBool::new(false));

    let blockhash_thread = {
        let exit_signal = exit_signal.clone();
        let blockhash = blockhash.clone();
        let client = client.clone();
        // let id = id.pubkey();
        Builder::new()
            .name("solana-blockhash-poller".to_string())
            .spawn(move || {
                poll_blockhash(&exit_signal, &blockhash, &client);
            })
            .unwrap()
    };

    let (mut keypairs, extra) = generate_keypairs(funding_key, keypair_count as u64);

    let pubkey_group = keypairs.iter().map(|x| x.pubkey()).collect::<Vec<_>>();
    // println!("group keypair: {:?}", pubkey_group);

    for i in 0..keypairs.len() {
        let cur_key = &keypairs[i];
        let key_balance = client.get_balance(&cur_key.pubkey()).unwrap_or(0);
        println!(
            "\n\nBEFORE\naccount {}: address: {}, balance: {}",
            i,
            &cur_key.pubkey(),
            key_balance
        );

        // let transfer_bal = 19 * key_balance / 20 as u64;

        if key_balance > 5000 {
            let transfer_bal = key_balance - 5000;
            println!("transfer_bal: {}", transfer_bal);
            let tx = system_transaction::transfer(
                &cur_key,
                &funding_key.pubkey(),
                transfer_bal,
                *blockhash.read().unwrap(),
            );

            let result = client.send_transaction(tx);

            // println!("result {}", result.unwrap());
        }
    }

    sleep(Duration::from_millis(1000));

    for i in 0..keypair_count {
        let cur_key = &keypairs[i];
        let key_balance = client.get_balance(&cur_key.pubkey()).unwrap_or(0);
        println!(
            "AFTER: \naccount {}: address: {}, balance: {}",
            i,
            &cur_key.pubkey(),
            key_balance
        );
    }
}
