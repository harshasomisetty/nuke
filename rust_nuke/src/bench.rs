use {
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
    solana_measure::measure::Measure,
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

// pub mod bench_tps_client;

// The point at which transactions become "too old", in seconds.
const MAX_TX_QUEUE_AGE: u64 = (MAX_PROCESSING_AGE as f64 * DEFAULT_S_PER_SLOT) as u64;

pub const MAX_SPENDS_PER_TX: u64 = 4;

pub type SharedTransactions = Arc<RwLock<VecDeque<Vec<(Transaction, u64)>>>>;

fn verify_funding_transfer<T: BenchTpsClient>(
    client: &Arc<T>,
    tx: &Transaction,
    amount: u64,
) -> bool {
    for a in &tx.message().account_keys[1..] {
        match client.get_balance_with_commitment(a, CommitmentConfig::processed()) {
            Ok(balance) => return balance >= amount,
            Err(err) => error!("failed to get balance {:?}", err),
        }
    }
    false
}

trait FundingTransactions<'a> {
    fn fund<T: 'static + BenchTpsClient + Send + Sync>(
        &mut self,
        client: &Arc<T>,
        to_fund: &[(&'a Keypair, Vec<(Pubkey, u64)>)],
        to_lamports: u64,
    );
    fn make(&mut self, to_fund: &[(&'a Keypair, Vec<(Pubkey, u64)>)]);
    fn sign(&mut self, blockhash: &Arc<RwLock<Hash>>);
    fn send<T: BenchTpsClient>(&self, client: &Arc<T>);
    fn verify<T: 'static + BenchTpsClient + Send + Sync>(
        &mut self,
        client: &Arc<T>,
        to_lamports: u64,
    );
}

impl<'a> FundingTransactions<'a> for Vec<(&'a Keypair, Transaction)> {
    fn fund<T: 'static + BenchTpsClient + Send + Sync>(
        &mut self,
        client: &Arc<T>,
        to_fund: &[(&'a Keypair, Vec<(Pubkey, u64)>)],
        to_lamports: u64,
    ) {
        self.make(to_fund);

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

        let mut tries = 0;
        while !self.is_empty() {
            println!(
                "{} {} each to {} accounts in {} txs",
                if tries == 0 {
                    "transferring"
                } else {
                    "retrying"
                },
                to_lamports,
                self.len() * MAX_SPENDS_PER_TX as usize,
                self.len(),
            );

            // re-sign retained to_fund_txes with updated blockhash
            self.sign(&blockhash);
            self.send(client);

            // Sleep a few slots to allow transactions to process
            sleep(Duration::from_secs(1));

            self.verify(client, to_lamports);

            // retry anything that seems to have dropped through cracks
            //  again since these txs are all or nothing, they're fine to
            //  retry
            tries += 1;
        }
        info!("transferred");
    }

    fn make(&mut self, to_fund: &[(&'a Keypair, Vec<(Pubkey, u64)>)]) {
        let mut make_txs = Measure::start("make_txs");
        let to_fund_txs: Vec<(&Keypair, Transaction)> = to_fund
            .par_iter()
            .map(|(k, t)| {
                let instructions = system_instruction::transfer_many(&k.pubkey(), t);
                let message = Message::new(&instructions, Some(&k.pubkey()));
                (*k, Transaction::new_unsigned(message))
            })
            .collect();
        make_txs.stop();
        debug!(
            "make {} unsigned txs: {}us",
            to_fund_txs.len(),
            make_txs.as_us()
        );
        self.extend(to_fund_txs);
    }

    fn sign(&mut self, blockhash: &Arc<RwLock<Hash>>) {
        for i in 0..1 {
            println!("blockhash in sign method {}", *blockhash.read().unwrap());
            sleep(Duration::from_millis(1000));
        }

        let mut sign_txs = Measure::start("sign_txs");
        self.par_iter_mut().for_each(|(k, tx)| {
            tx.sign(&[*k], *blockhash.read().unwrap());
        });
        sign_txs.stop();
        debug!("sign {} txs: {}us", self.len(), sign_txs.as_us());
    }

    fn send<T: BenchTpsClient>(&self, client: &Arc<T>) {
        let mut send_txs = Measure::start("send_and_clone_txs");
        let batch: Vec<_> = self.iter().map(|(_keypair, tx)| tx.clone()).collect();

        println!("sending batch");
        client.send_batch(batch).expect("transfer");
        send_txs.stop();
        debug!("send {} {}", self.len(), send_txs);
    }

    fn verify<T: 'static + BenchTpsClient + Send + Sync>(
        &mut self,
        client: &Arc<T>,
        to_lamports: u64,
    ) {
        let starting_txs = self.len();
        let verified_txs = Arc::new(AtomicUsize::new(0));
        let too_many_failures = Arc::new(AtomicBool::new(false));
        let loops = if starting_txs < 1000 { 3 } else { 1 };

        println!("verify loops: {}", loops);
        // Only loop multiple times for small (quick) transaction batches
        let time = Arc::new(Mutex::new(Instant::now()));
        for _ in 0..loops {
            let time = time.clone();
            let failed_verify = Arc::new(AtomicUsize::new(0));
            let client = client.clone();
            let verified_txs = &verified_txs;
            let failed_verify = &failed_verify;
            let too_many_failures = &too_many_failures;
            let verified_set: HashSet<Pubkey> = self
                .par_iter()
                .filter_map(move |(k, tx)| {
                    if too_many_failures.load(Ordering::Relaxed) {
                        return None;
                    }

                    let verified = if verify_funding_transfer(&client, tx, to_lamports) {
                        verified_txs.fetch_add(1, Ordering::Relaxed);
                        Some(k.pubkey())
                    } else {
                        failed_verify.fetch_add(1, Ordering::Relaxed);
                        None
                    };

                    let verified_txs = verified_txs.load(Ordering::Relaxed);
                    let failed_verify = failed_verify.load(Ordering::Relaxed);
                    let remaining_count = starting_txs.saturating_sub(verified_txs + failed_verify);
                    if failed_verify > 100 && failed_verify > verified_txs {
                        too_many_failures.store(true, Ordering::Relaxed);
                        warn!(
                            "Too many failed transfers... {} remaining, {} verified, {} failures",
                            remaining_count, verified_txs, failed_verify
                        );
                    }
                    if remaining_count > 0 {
                        let mut time_l = time.lock().unwrap();
                        if time_l.elapsed().as_secs() > 2 {
                            info!(
                                "Verifying transfers... {} remaining, {} verified, {} failures",
                                remaining_count, verified_txs, failed_verify
                            );
                            *time_l = Instant::now();
                        }
                    }

                    verified
                })
                .collect();

            self.retain(|(k, _)| !verified_set.contains(&k.pubkey()));
            if self.is_empty() {
                break;
            }
            info!("Looping verifications");

            let verified_txs = verified_txs.load(Ordering::Relaxed);
            let failed_verify = failed_verify.load(Ordering::Relaxed);
            let remaining_count = starting_txs.saturating_sub(verified_txs + failed_verify);
            info!(
                "Verifying transfers... {} remaining, {} verified, {} failures",
                remaining_count, verified_txs, failed_verify
            );
            sleep(Duration::from_millis(100));
        }
    }
}

/// fund the dests keys by spending all of the source keys into MAX_SPENDS_PER_TX
/// on every iteration.  This allows us to replay the transfers because the source is either empty,
/// or full
pub fn fund_keys<T: 'static + BenchTpsClient + Send + Sync>(
    client: Arc<T>,
    source: &Keypair,
    dests: &[Keypair],
    total: u64,
    max_fee: u64,
    lamports_per_account: u64,
) {
    let mut funded: Vec<&Keypair> = vec![source];
    let mut funded_funds = total;
    let mut not_funded: Vec<&Keypair> = dests.iter().collect();

    while !not_funded.is_empty() {
        // Build to fund list and prepare funding sources for next iteration
        println!("loop not empty: {}", not_funded.len());
        let mut new_funded: Vec<&Keypair> = vec![];
        let mut to_fund: Vec<(&Keypair, Vec<(Pubkey, u64)>)> = vec![];
        let to_lamports = (funded_funds - lamports_per_account - max_fee) / MAX_SPENDS_PER_TX;
        for f in funded {
            println!(
                "not funded len {}, to lamports {}",
                not_funded.len(),
                to_lamports
            );

            let start = not_funded.len() - MAX_SPENDS_PER_TX as usize;
            println!("start: {}", start);
            let dests: Vec<_> = not_funded.drain(start..).collect();

            for dest in &dests {
                println!("dest {}", dest.pubkey());
            }

            let spends: Vec<_> = dests.iter().map(|k| (k.pubkey(), to_lamports)).collect();

            for spend in &spends {
                println!("pubkey {} and lamports {}", spend.0, spend.1);
            }

            to_fund.push((f, spends));
            new_funded.extend(dests.into_iter());
        }

        // try to transfer a "few" at a time with recent blockhash
        //  assume 4MB network buffers, and 512 byte packets
        const FUND_CHUNK_LEN: usize = 4 * 1024 * 1024 / 512;

        to_fund.chunks(FUND_CHUNK_LEN).for_each(|chunk| {
            Vec::<(&Keypair, Transaction)>::with_capacity(chunk.len()).fund(
                &client,
                chunk,
                to_lamports,
            );
        });

        info!("funded: {} left: {}", new_funded.len(), not_funded.len());
        funded = new_funded;
        funded_funds = to_lamports;
        println!(
            "after fund remaining funded: {}, non_funded {}",
            &funded.len(),
            &not_funded.len()
        );
    }
}

pub fn generate_keypairs(seed_keypair: &Keypair, count: u64) -> (Vec<Keypair>, u64) {
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_keypair.to_bytes()[..32]);
    let mut rnd = GenKeys::new(seed);

    let mut total_keys = 0;
    let mut extra = 0; // This variable tracks the number of keypairs needing extra transaction fees funded
    let mut delta = 1;
    println!(
        "extra {} delta {} total keys {} count {}",
        extra, delta, total_keys, count
    );
    while total_keys < count {
        extra += delta;
        delta *= MAX_SPENDS_PER_TX;
        total_keys += delta;
        println!(
            "extra {} delta {} total keys {} count {}",
            extra, delta, total_keys, count
        );
    }

    // total_keys instead of count
    (rnd.gen_n_keypairs(total_keys), extra)
}

pub fn generate_and_fund_keypairs<T: 'static + BenchTpsClient + Send + Sync>(
    client: Arc<T>,
    funding_key: &Keypair,
    keypair_count: usize,
    lamports_per_account: u64,
) -> Result<Vec<Keypair>> {
    let rent = client.get_minimum_balance_for_rent_exemption(0)?;
    let lamports_per_account = lamports_per_account + rent;

    let funding_key_balance = client.get_balance(&funding_key.pubkey()).unwrap_or(0);

    println!("Creating {} keypairs...", keypair_count);
    let (mut keypairs, extra) = generate_keypairs(funding_key, keypair_count as u64);

    for k in &keypairs {
        println!("key {}", k.pubkey())
    }

    println!(
        "keypair counts: keypairs {}, extras {}",
        keypairs.len(),
        extra
    );
    println!("funding {} keypairs...", keypair_count);
    println!(
        "actual funding keypair: {} bal {}",
        funding_key.pubkey(),
        funding_key_balance
    );

    fund_keypairs(client, funding_key, &keypairs, extra, lamports_per_account)?;

    // 'generate_keypairs' generates extra keys to be able to have size-aligned funding batches for fund_keys.
    keypairs.truncate(keypair_count);

    Ok(keypairs)
}

pub fn fund_keypairs<T: 'static + BenchTpsClient + Send + Sync>(
    client: Arc<T>,
    funding_key: &Keypair,
    keypairs: &[Keypair],
    extra: u64,
    lamports_per_account: u64,
) -> Result<()> {
    let rent = client.get_minimum_balance_for_rent_exemption(0)?;
    println!("Get lamports...");

    // Sample the first keypair, to prevent lamport loss on repeated solana-bench-tps executions
    let first_key = keypairs[0].pubkey();
    let first_keypair_balance = client.get_balance(&first_key).unwrap_or(0);

    println!(
        "first keypair? add: {} and bal {}",
        first_key, first_keypair_balance
    );
    // Sample the last keypair, to check if funding was already completed
    let last_key = keypairs[keypairs.len() - 1].pubkey();
    let last_keypair_balance = client.get_balance(&last_key).unwrap_or(0);

    println!(
        "last keypair? add: {} and bal {}",
        last_key, last_keypair_balance
    );

    // Repeated runs will eat up keypair balances from transaction fees. In order to quickly
    //   start another bench-tps run without re-funding all of the keypairs, check if the
    //   keypairs still have at least 80% of the expected funds. That should be enough to
    //   pay for the transaction fees in a new run.

    let enough_lamports = 8 * lamports_per_account / 10;
    if first_keypair_balance < enough_lamports || last_keypair_balance < enough_lamports {
        println!("\n\nfunding keys!!!");
        let single_sig_message = Message::new_with_blockhash(
            &[Instruction::new_with_bytes(
                Pubkey::new_unique(),
                &[],
                vec![AccountMeta::new(Pubkey::new_unique(), true)],
            )],
            None,
            &client.get_latest_blockhash().unwrap(),
        );
        let max_fee = client.get_fee_for_message(&single_sig_message).unwrap();
        let extra_fees = extra * max_fee;
        let total_keypairs = keypairs.len() as u64 + 1; // Add one for funding keypair
        let total = lamports_per_account * total_keypairs + extra_fees;

        let funding_key_balance = client.get_balance(&funding_key.pubkey()).unwrap_or(0);
        println!(
            "Funding keypair balance: {} max_fee: {} lamports_per_account: {} extra: {} total: {}",
            funding_key_balance, max_fee, lamports_per_account, extra, total
        );

        if funding_key_balance < total + rent {
            error!(
                "funder has {}, needed {}",
                Sol(funding_key_balance),
                Sol(total)
            );
            let latest_blockhash = get_latest_blockhash(client.as_ref());

            if client
                .request_airdrop_with_blockhash(
                    &funding_key.pubkey(),
                    total + rent - funding_key_balance,
                    &latest_blockhash,
                )
                .is_err()
            {
                println!("benchtps airdrop error");
                return Err(BenchTpsError::AirdropFailure);
            }
        }
        fund_keys(
            client,
            funding_key,
            keypairs,
            total,
            max_fee,
            lamports_per_account,
        );
    } else {
        println!("\n\nnot funding");
    }

    Ok(())
}
