// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Nicola Busanello <nicola.busanello@gmail.com>
//
// Copyright (C) 2024 LNP/BP Standards Association. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU32;
use std::str::FromStr;

use bpstd::{
    Address, BlockHash, ConsensusEncode, DerivedAddr, Outpoint, Sats, Tx, TxIn, Txid, Weight,
};
use descriptors::Descriptor;
use electrum::{Client, ElectrumApi, Error, GetHistoryRes, Param};
use serde_json::Value;

use super::cache::TxDetail;
use super::{IndexerCache, BATCH_SIZE};
use crate::{
    Indexer, Layer2, MayError, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletAddr,
    WalletCache, WalletDescr, WalletTx,
};

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display, Error)]
#[display(doc_comments)]
pub enum ElectrumApiError {
    /// Electrum indexer returned invalid hex value for the transaction {0}.
    InvalidTx(Txid),
    /// Electrum indexer returned invalid block hash hex value for the transaction {0}.
    InvalidBlockHash(Txid),
    /// Electrum indexer returned invalid block time value for the transaction {0}.
    InvalidBlockTime(Txid),
    /// electrum indexer returned zero block height for the transaction {0}.
    InvalidBlockHeight(Txid),
    /// electrum indexer returned invalid previous transaction, which doesn't have an output spent
    /// by transaction {0} input {1:?}.
    PrevOutTxMismatch(Txid, TxIn),
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum ElectrumError {
    #[from]
    Api(ElectrumApiError),
    #[from]
    Client(Error),
}

pub struct ElectrumClient {
    client: Client,
    cache: IndexerCache,
}

impl ElectrumClient {
    pub fn new(url: &str, cache: IndexerCache) -> Result<Self, ElectrumError> {
        let client = Client::new(url).map_err(ElectrumError::Client)?;
        Ok(Self { client, cache })
    }
}

impl Indexer for ElectrumClient {
    type Error = ElectrumError;

    fn create<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descriptor: &WalletDescr<K, D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Self::Error>> {
        let mut cache = WalletCache::new();
        let mut errors = Vec::<ElectrumError>::new();

        let mut address_index = BTreeMap::new();
        for keychain in descriptor.keychains() {
            let mut empty_count = 0usize;
            eprint!(" keychain {keychain} ");
            for derive in descriptor.addresses(keychain) {
                let script = derive.addr.script_pubkey();

                eprint!(".");
                let mut txids = Vec::new();
                let hres = self.get_script_history(&derive, &mut errors);
                if hres.is_empty() {
                    empty_count += 1;
                    if empty_count >= BATCH_SIZE {
                        break;
                    }
                    continue;
                }

                empty_count = 0;

                // build wallet transactions from script tx history, collecting indexer errors
                for (_, hr) in hres {
                    match self.process_history_entry(hr, &mut txids) {
                        Ok(tx) => {
                            cache.tx.insert(tx.txid, tx);
                        }
                        Err(e) => errors.push(e.into()),
                    }
                }

                let wallet_addr = WalletAddr::<i64>::from(derive);
                address_index.insert(script, (wallet_addr, txids));
            }
        }

        // TODO: Update headers & tip

        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = cache.tx.remove(txid).expect("broken logic");
                for debit in &mut tx.outputs {
                    let Some(s) = debit.beneficiary.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        cache.utxo.insert(debit.outpoint);
                        debit.beneficiary = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.used = wallet_addr.used.saturating_add(1);
                        wallet_addr.volume.saturating_add_assign(debit.value);
                        wallet_addr.balance = wallet_addr
                            .balance
                            .saturating_add(debit.value.sats().try_into().expect("sats overflow"));
                    } else if debit.beneficiary.is_unknown() {
                        Address::with(&s, descriptor.network())
                            .map(|addr| {
                                debit.beneficiary = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                }
                cache.tx.insert(tx.txid, tx);
            }
        }

        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = cache.tx.remove(txid).expect("broken logic");
                for credit in &mut tx.inputs {
                    let Some(s) = credit.payer.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        credit.payer = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.balance = wallet_addr
                            .balance
                            .saturating_sub(credit.value.sats().try_into().expect("sats overflow"));
                    } else if credit.payer.is_unknown() {
                        Address::with(&s, descriptor.network())
                            .map(|addr| {
                                credit.payer = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                    if let Some(prev_tx) = cache.tx.get_mut(&credit.outpoint.txid) {
                        if let Some(txout) =
                            prev_tx.outputs.get_mut(credit.outpoint.vout_u32() as usize)
                        {
                            let outpoint = txout.outpoint;
                            if tx.status.is_mined() {
                                cache.utxo.remove(&outpoint);
                            }
                            txout.spent = Some(credit.outpoint.into())
                        };
                    }
                }
                cache.tx.insert(tx.txid, tx);
            }
            cache
                .addr
                .entry(wallet_addr.terminal.keychain)
                .or_default()
                .insert(wallet_addr.expect_transmute());
        }

        if errors.is_empty() { MayError::ok(cache) } else { MayError::err(cache, errors) }
    }

    fn update<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
        cache: &mut WalletCache<L2::Cache>,
    ) -> MayError<usize, Vec<Self::Error>> {
        let mut errors = Vec::<ElectrumError>::new();
        let mut update_size = 0;

        let mut address_index = BTreeMap::new();
        for keychain in descr.keychains() {
            let mut empty_count = 0usize;
            eprint!(" keychain {keychain} ");
            for derive in descr.addresses(keychain) {
                let script = derive.addr.script_pubkey();

                eprint!(".");
                let mut txids = Vec::new();
                let (updated_hres, append_hres) = self.update_script_history(&derive, &mut errors);
                if updated_hres.is_empty() && append_hres.is_empty() {
                    empty_count += 1;
                    if empty_count >= BATCH_SIZE {
                        break;
                    }
                    continue;
                }

                empty_count = 0;

                for (txid, hr) in updated_hres {
                    let tx = cache.tx.get_mut(&txid).expect("broken logic");

                    let status = if hr.height < 1 {
                        TxStatus::Mempool
                    } else {
                        let res = self.get_transaction_details(&txid).map_err(|e| errors.push(e));
                        if res.is_err() {
                            continue;
                        }

                        let TxDetail {
                            blockhash,
                            blocktime,
                            ..
                        } = res.expect("broken logic");

                        let height = NonZeroU32::try_from(hr.height as u32)
                            .expect("hr.height is cannot be zero");
                        TxStatus::Mined(MiningInfo {
                            height,
                            time: blocktime.expect("blocktime is missing"),
                            block_hash: blockhash.expect("blockhash is missing"),
                        })
                    };
                    tx.status = status;
                    update_size += 1;
                }

                for (_, hr) in append_hres {
                    match self.process_history_entry(hr, &mut txids) {
                        Ok(tx) => {
                            cache.tx.insert(tx.txid, tx);
                            update_size += 1;
                        }
                        Err(e) => errors.push(e.into()),
                    }
                }

                let wallet_addr_key = WalletAddr::from(derive);
                let old_wallet_addr = cache
                    .addr
                    .entry(wallet_addr_key.terminal.keychain)
                    .or_default()
                    .get(&wallet_addr_key)
                    .cloned()
                    .unwrap_or(wallet_addr_key);

                address_index.insert(script, (old_wallet_addr, txids));
            }
        }

        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = cache.tx.remove(txid).expect("broken logic");
                for debit in &mut tx.outputs {
                    let Some(s) = debit.beneficiary.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        cache.utxo.insert(debit.outpoint);
                        debit.beneficiary = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.used = wallet_addr.used.saturating_add(1);
                        wallet_addr.volume.saturating_add_assign(debit.value);
                        wallet_addr.balance = wallet_addr.balance.saturating_add(debit.value);
                    } else if debit.beneficiary.is_unknown() {
                        Address::with(&s, descr.network())
                            .map(|addr| {
                                debit.beneficiary = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                }
                cache.tx.insert(tx.txid, tx);
            }
        }

        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = cache.tx.remove(txid).expect("broken logic");
                for credit in &mut tx.inputs {
                    let Some(s) = credit.payer.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        credit.payer = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.balance = wallet_addr.balance.saturating_sub(credit.value);
                    } else if credit.payer.is_unknown() {
                        Address::with(&s, descr.network())
                            .map(|addr| {
                                credit.payer = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                    if let Some(prev_tx) = cache.tx.get_mut(&credit.outpoint.txid) {
                        if let Some(txout) =
                            prev_tx.outputs.get_mut(credit.outpoint.vout_u32() as usize)
                        {
                            let outpoint = txout.outpoint;
                            if tx.status.is_mined() {
                                cache.utxo.remove(&outpoint);
                            }
                            txout.spent = Some(credit.outpoint.into())
                        };
                    }
                }
                cache.tx.insert(tx.txid, tx);
            }

            // replace the old wallet_addr with the new one
            cache.addr.entry(wallet_addr.terminal.keychain).or_default().replace(*wallet_addr);
            update_size += 1;
        }

        if errors.is_empty() {
            MayError::ok(update_size)
        } else {
            MayError::err(update_size, errors)
        }
    }

    fn publish(&self, tx: &Tx) -> Result<(), Self::Error> {
        self.client.transaction_broadcast(tx)?;
        Ok(())
    }
}

impl ElectrumClient {
    fn get_script_history(
        &self,
        derived_addr: &DerivedAddr,
        errors: &mut Vec<ElectrumError>,
    ) -> HashMap<Txid, GetHistoryRes> {
        let mut cache = self.cache.script_history.lock().expect("poisoned lock");
        if let Some(history) = cache.get(derived_addr) {
            return history.clone();
        }

        let script = derived_addr.addr.script_pubkey();
        let hres = self
            .client
            .script_get_history(&script)
            .map_err(|err| errors.push(err.into()))
            .unwrap_or_default();
        let hres: HashMap<Txid, GetHistoryRes> =
            hres.into_iter().map(|hr| (hr.tx_hash, hr)).collect();

        cache.put(derived_addr.clone(), hres.clone());
        hres
    }

    fn update_script_history(
        &self,
        derived_addr: &DerivedAddr,
        errors: &mut Vec<ElectrumError>,
    ) -> (HashMap<Txid, GetHistoryRes>, HashMap<Txid, GetHistoryRes>) {
        let mut updated = HashMap::new();
        let mut append = HashMap::new();

        let mut cache = self.cache.script_history.lock().expect("poisoned lock");

        let old_history = cache.get(derived_addr).cloned().unwrap_or_default();

        let script = derived_addr.addr.script_pubkey();
        let new_history = self
            .client
            .script_get_history(&script)
            .map_err(|err| {
                errors.push(err.into());
            })
            .unwrap_or_default();
        if new_history.is_empty() {
            return (updated, append);
        }

        let new_history: HashMap<Txid, GetHistoryRes> =
            new_history.into_iter().map(|hr| (hr.tx_hash, hr)).collect();

        for (txid, hr) in new_history.iter() {
            if let Some(old_hr) = old_history.get(txid) {
                if old_hr.height != hr.height {
                    updated.insert(*txid, hr.clone());
                }
                continue;
            }

            append.insert(*txid, hr.clone());
        }

        cache.put(derived_addr.clone(), new_history.clone());
        (updated, append)
    }

    fn get_transaction_details(&self, txid: &Txid) -> Result<TxDetail, ElectrumError> {
        let mut cache = self.cache.tx_details.lock().expect("poisoned lock");
        if let Some(details) = cache.get(txid) {
            // if blockhash exists, the transaction has been confirmed and will not change
            // Otherwise, we need to get the latest information
            if details.blockhash.is_some() {
                return Ok(details.clone());
            }
        }

        let tx_details = self.client.raw_call("blockchain.transaction.get", vec![
            Param::String(txid.to_string()),
            Param::Bool(true),
        ])?;

        let inner: Tx = tx_details
            .get("hex")
            .and_then(Value::as_str)
            .and_then(|s| Tx::from_str(s).ok())
            .ok_or(ElectrumApiError::InvalidTx(txid.clone()))?;

        let blockhash = tx_details
            .get("blockhash")
            .and_then(Value::as_str)
            .and_then(|s| BlockHash::from_str(s).ok());
        let blocktime = tx_details.get("blocktime").and_then(Value::as_u64);

        let tx_detail = TxDetail {
            inner,
            blockhash,
            blocktime,
        };

        cache.put(*txid, tx_detail.clone());
        Ok(tx_detail)
    }

    // TODO: maybe WalletTx can be cached too
    fn process_history_entry(
        &self,
        hr: GetHistoryRes,
        txids: &mut Vec<Txid>,
    ) -> Result<WalletTx, ElectrumError> {
        let txid = hr.tx_hash;
        txids.push(txid);

        let TxDetail {
            inner: tx,
            blockhash,
            blocktime,
        } = self.get_transaction_details(&txid)?;

        // build TxStatus
        let status = if hr.height < 1 {
            TxStatus::Mempool
        } else {
            let height = NonZeroU32::try_from(hr.height as u32)
                .map_err(|_| ElectrumApiError::InvalidBlockHeight(txid))?;
            TxStatus::Mined(MiningInfo {
                height,
                time: blocktime.expect("blocktime is missing"),
                block_hash: blockhash.expect("blockhash is missing"),
            })
        };
        let tx_size = tx.consensus_serialize().len();
        let weight = tx.weight_units().to_u32();

        // get inputs to build TxCredit's and total amount,
        // collecting indexer errors
        let mut input_total = Sats::ZERO;
        let mut inputs = Vec::with_capacity(tx.inputs.len());
        for input in tx.inputs {
            // get value from previous output tx
            let prev_tx = self.get_transaction_details(&input.prev_output.txid)?.inner;
            let prev_out = prev_tx
                .outputs
                .get(input.prev_output.vout.into_usize())
                .ok_or_else(|| ElectrumApiError::PrevOutTxMismatch(txid, input.clone()))?;
            let value = prev_out.value;
            input_total += value;
            inputs.push(TxCredit {
                outpoint: input.prev_output,
                payer: Party::Unknown(prev_out.script_pubkey.clone()),
                sequence: input.sequence,
                coinbase: false,
                script_sig: input.sig_script,
                witness: input.witness,
                value,
            })
        }

        // get outputs and total amount, build TxDebit's
        let mut output_total = Sats::ZERO;
        let mut outputs = Vec::with_capacity(tx.outputs.len());
        for (no, txout) in tx.outputs.into_iter().enumerate() {
            output_total += txout.value;
            outputs.push(TxDebit {
                outpoint: Outpoint::new(txid, no as u32),
                beneficiary: Party::Unknown(txout.script_pubkey),
                value: txout.value,
                spent: None,
            })
        }

        // build the WalletTx
        return Ok(WalletTx {
            txid,
            status,
            inputs,
            outputs,
            fee: input_total - output_total,
            size: tx_size as u32,
            weight,
            version: tx.version,
            locktime: tx.lock_time,
        });
    }
}
