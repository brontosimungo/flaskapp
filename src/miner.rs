use crate::config::Config;

use quiver::types::{Template, Submission, Target};
use kernels::miner::KERNEL;

use sysinfo::System;
use tokio::sync::{Mutex, watch};
use anyhow::Result;
use tracing::{info, warn, error};
use rand::Rng;
use bytes::Bytes;

use nockapp::save::SaveableCheckpoint;
use nockapp::utils::NOCK_STACK_SIZE_TINY;
use nockapp::kernel::form::SerfThread;
use nockapp::noun::slab::NounSlab;
use nockapp::noun::AtomExt;
use nockapp::NounExt;
use nockapp::wire::{WireRepr, WireTag};

use nockvm::noun::{Atom, D, T};
use nockvm::interpreter::NockCancelToken;

use zkvm_jetpack::form::PRIME;

use nockvm_macros::tas;

pub async fn start(
    config: Config,
    mut template_rx: watch::Receiver<Template>,
    submission_tx: watch::Sender<Submission>,
) -> Result<()> {
    let num_threads = {
        let sys = System::new_all();
        let logical_cores = sys.cpus().len() as u32;
        let calculated_threads = logical_cores.saturating_sub(2).max(1);
        if let Some(max_threads) = config.max_threads {
            max_threads.min(calculated_threads) as u64
        } else {
            calculated_threads as u64
        }
    };
    info!("mining with {} threads", num_threads);

    let mut mining_attempts = tokio::task::JoinSet::<(
        SerfThread<SaveableCheckpoint>,
        u64,
        Result<NounSlab, anyhow::Error>,
    )>::new();

    let network_only = config.network_only;

    if network_only {
        info!("mining for network target only");
    } else {
        info!("mining for pool and network targets");
    }

    let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
    let test_jets_str = std::env::var("NOCK_TEST_JETS").unwrap_or_default();
    let test_jets = nockapp::kernel::boot::parse_test_jets(test_jets_str.as_str());

    let mining_data: Mutex<Option<Template>> = Mutex::new(None);
    let mut cancel_tokens: Vec<NockCancelToken> = Vec::<NockCancelToken>::new();

    loop {
        tokio::select! {
            mining_result = mining_attempts.join_next(), if !mining_attempts.is_empty() => {
                match mining_result {
                    Some(join_outcome) => match join_outcome {
                        Ok((serf, id, slab_res)) => {
                            let slab = match slab_res {
                                Ok(slab) => slab,
                                Err(e) => {
                                    error!(%id, error = ?e, "mining attempt returned error; restarting");
                                    mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                    continue;
                                }
                            };

                            let result = unsafe { slab.root() };
                            let result_cell = match result.as_cell() {
                                Ok(c) => c,
                                Err(_) => {
                                    // error!(%id, error = ?e, "invalid result noun (expected cell); restarting");
                                    mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                    continue;
                                }
                            };

                            let hed = result_cell.head();

                            if hed.is_atom() && hed.eq_bytes("poke") {
                                //  mining attempt was cancelled. restart with current block header.
                                info!("using new template on thread={id}");
                                mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                continue;
                            }

                            let effect = match hed.as_cell() {
                                Ok(c) => c,
                                Err(e) => {
                                    error!(%id, error = ?e, "invalid effect (expected cell head)");
                                    mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                    continue;
                                }
                            };

                            if effect.head().eq_bytes("miss") {
                                info!("solution did not hit targets on thread={id}, trying again");
                                let mut nonce_slab = NounSlab::new();
                                nonce_slab.copy_into(effect.tail());
                                mine(serf, mining_data.lock().await, &mut mining_attempts, Some(nonce_slab), id).await;
                                continue;
                            }

                            let target_type = if effect.head().eq_bytes("pool") {
                                Target::Pool
                            } else if effect.head().eq_bytes("network") {
                                Target::Network
                            } else {
                                warn!("solution found but invalid target on thread={id}: {:?}", effect.head());
                                mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                continue;
                            };

                            if network_only && target_type != Target::Network {
                                info!("solution did not hit network target on thread={id}, trying again");
                                mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                continue;
                            }

                            let success_message = match effect.tail().as_cell() {
                                Ok(c) => c,
                                Err(e) => {
                                    error!(%id, error = ?e, "invalid success message (expected cell)");
                                    mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                    continue;
                                }
                            };

                            // 2
                            let mut commit_slab: NounSlab = NounSlab::new();
                            commit_slab.copy_into(success_message.head());
                            let commit = commit_slab.jam();

                            // 3
                            let success_message_tail = match success_message.tail().as_cell() {
                                Ok(c) => c,
                                Err(e) => {
                                    error!(%id, error = ?e, "invalid success message tail (expected cell)");
                                    mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                                    continue;
                                }
                            };

                            // 6
                            let digest = Bytes::from(success_message_tail.head().as_atom()?.to_le_bytes());

                            // 7
                            let mut proof_slab: NounSlab = NounSlab::new();
                            proof_slab.copy_into(success_message_tail.tail());
                            let proof = proof_slab.jam();

                            let submission = Submission::new(target_type.clone(), commit, digest, proof);
                            info!(
                                "solution found on thread={id} for target={:?}. Proof size: {:?} KB. Submitting to nockpool.",
                                target_type,
                                ((submission.proof.len() as f64) / 1024.0 * 100.0).round() / 100.0,
                            );
                            if let Err(e) = submission_tx.send(submission) {
                                error!(%id, error = ?e, "failed to send submission");
                            }

                            mine(serf, mining_data.lock().await, &mut mining_attempts, None, id).await;
                        }
                        Err(e) => {
                            error!(error = ?e, "mining task failed to join");
                            continue;
                        }
                    },
                    None => {
                        warn!("join_next returned None unexpectedly");
                        continue;
                    }
                }
            }
            _ = template_rx.changed() => {
                let template = template_rx.borrow_and_update().clone();

                *(mining_data.lock().await) = Some(template);

                if mining_attempts.is_empty() {
                    let mut init_tasks = tokio::task::JoinSet::<(u64, Result<SerfThread<SaveableCheckpoint>, anyhow::Error>)>::new();
                    for i in 0..num_threads {
                        let kernel = Vec::from(KERNEL);
                        let hot_state = hot_state.clone();
                        let test_jets = test_jets.clone();
                        init_tasks.spawn(async move {
                            let res = SerfThread::<SaveableCheckpoint>::new(
                                kernel,
                                None,
                                hot_state,
                                NOCK_STACK_SIZE_TINY,
                                test_jets,
                                Default::default(),
                            )
                            .await
                            .map_err(|e| anyhow::anyhow!(e));
                            (i, res)
                        });
                    }
                    info!("Received nockpool template! Starting {} mining threads", num_threads);
                    while let Some(res) = init_tasks.join_next().await {
                        match res {
                            Ok((i, Ok(serf))) => {
                                cancel_tokens.push(serf.cancel_token.clone());
                                mine(serf, mining_data.lock().await, &mut mining_attempts, None, i).await;
                            }
                            Ok((i, Err(e))) => {
                                error!(thread_index = i, error = ?e, "Could not load mining kernel");
                            }
                            Err(e) => {
                                error!(error = ?e, "kernel init task join error");
                            }
                        }
                    }
                } else {
                    // Mining is already running so cancel all the running attemps
                    // which are mining on the old block.
                    info!("New nockpool template! Restarting {} mining threads", num_threads);
                    for token in &cancel_tokens {
                        token.cancel();
                    }
                }
            },
        }
    }
}
/*
        %template
        version=?(%0 %1 %2)
        commit=block-commitment:t
        nonce=noun-digest:tip5
        network-target=bignum:bignum
        pool-target=bignum:bignum
        pow-len=@        
*/
async fn mine(
    serf: SerfThread<SaveableCheckpoint>,
    template: tokio::sync::MutexGuard<'_, Option<Template>>,
    mining_attempts: &mut tokio::task::JoinSet<(
        SerfThread<SaveableCheckpoint>,
        u64,
        Result<NounSlab>,
    )>,
    nonce: Option<NounSlab>,
    id: u64,
) {
    let mut slab = NounSlab::new();
    // let's first deal with the nonce
    let nonce = if let Some(nonce) = nonce {
        nonce
    } else {
        let mut rng = rand::thread_rng();
        let mut nonce_slab: NounSlab = NounSlab::new();
        let first_atom = match Atom::from_value(&mut nonce_slab, rng.gen::<u64>() % PRIME) {
            Ok(atom) => atom.as_noun(),
            Err(e) => {
                error!(%id, error = ?e, "failed to create first nonce atom");
                return;
            }
        };
        let mut nonce_cell = first_atom;
        for _ in 1..5 {
            let nonce_atom = match Atom::from_value(&mut nonce_slab, rng.gen::<u64>() % PRIME) {
                Ok(atom) => atom.as_noun(),
                Err(e) => {
                    error!(%id, error = ?e, "failed to create nonce atom");
                    return;
                }
            };
            nonce_cell = T(&mut nonce_slab, &[nonce_atom, nonce_cell]);
        }
        nonce_slab.set_root(nonce_cell);
        nonce_slab
    };

    // now we deal with the rest of the template noun
    let template_ref = match template.as_ref() {
        Some(t) => t,
        None => {
            warn!(%id, "mining data not initialized; skipping attempt");
            return;
        }
    };

    let version_atom = Atom::from_bytes(&mut slab, (&template_ref.version.clone()).into());
    let commit = match slab.cue_into(template_ref.commit.clone().into()) {
        Ok(v) => v,
        Err(e) => {
            error!(%id, error = ?e, "Failed to cue commit");
            return;
        }
    };
    let nonce = slab.copy_into(unsafe { *(nonce.root()) });
    let network_target = match slab.cue_into(template_ref.network_target.clone().into()) {
        Ok(v) => v,
        Err(e) => {
            error!(%id, error = ?e, "Failed to cue network target");
            return;
        }
    };
    let pool_target = match slab.cue_into(template_ref.pool_target.clone().into()) {
        Ok(v) => v,
        Err(e) => {
            error!(%id, error = ?e, "Failed to cue pool target");
            return;
        }
    };
    let pow_len_atom = Atom::from_bytes(&mut slab, (&template_ref.pow_len.clone()).into());
    let noun = T(&mut slab, &[
        D(tas!(b"template")),
        version_atom.as_noun(),
        commit,
        nonce,
        network_target,
        pool_target,
        pow_len_atom.as_noun(),
    ]);

    slab.set_root(noun);

    let wire = WireRepr::new("miner", 1, vec![WireTag::String("candidate".to_string())]);
    mining_attempts.spawn(async move {
        info!("starting mining attempt on thread={id}");
        let result = serf.poke(wire.clone(), slab.clone()).await.map_err(|e| anyhow::anyhow!(e));
        (serf, id, result)
    });
}

pub async fn benchmark() -> Result<()> {
    let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
    let test_jets_str = std::env::var("NOCK_TEST_JETS").unwrap_or_default();
    let test_jets = nockapp::kernel::boot::parse_test_jets(test_jets_str.as_str());

    let version = hex::decode("0200000000000000").map_err(|e| anyhow::anyhow!("Failed to decode version: {e}"))?;
    let commit = hex::decode("017ee86437eac9dbae690081199671e25cb54ce700ff8cdb259db500063b409e2aa64f968ec7ed801e75db735d82443707").map_err(|e| anyhow::anyhow!("Failed to decode commit: {e}"))?;
    let network_target = hex::decode("81177307ec6aacb01ef04b58dbf601823db967ef80ed5256e50304ab9031bb015f1c808d1d2058623e470ce8cdf54174c07f08c49a068e8f02").map_err(|e| anyhow::anyhow!("Failed to decode network target: {e}"))?;
    let pool_target = hex::decode("81177307ec6aacb01ef04b58dbf601823db967ef80ed5256e50304ab9031bb015f1c808d1d2058623e470ce8cdf54174c07f08c49a068e8f02").map_err(|e| anyhow::anyhow!("Failed to decode pool target: {e}"))?;
    let pow_len = hex::decode("4000000000000000").map_err(|e| anyhow::anyhow!("Failed to decode pow len: {e}"))?;

    let mining_data: Mutex<Option<Template>> = Mutex::new(Some(
        Template::new(
            Bytes::from(version),
            Bytes::from(commit),
            Bytes::from(network_target),
            Bytes::from(pool_target),
            Bytes::from(pow_len),
        )
    ));

    let mut mining_attempts = tokio::task::JoinSet::<(
        SerfThread<SaveableCheckpoint>,
        u64,
        Result<NounSlab>,
    )>::new();
    let kernel = Vec::from(KERNEL);
    let serf = SerfThread::<SaveableCheckpoint>::new(
        kernel,
        None,
        hot_state.clone(),
        NOCK_STACK_SIZE_TINY,
        test_jets.clone(),
        Default::default(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Could not load mining kernel: {e}"))?;
    let start = tokio::time::Instant::now();
    let _ = mine(serf, mining_data.lock().await, &mut mining_attempts, None, 1337).await;

    loop {
        tokio::select! {
            _ = mining_attempts.join_next() => {
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    info!("Generated proof in {:?}", elapsed);
    return Ok(());
}