// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
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

use std::collections::BTreeMap;
use super::BATCH_SIZE;
use crate::{
    Indexer, Layer2, MayError, Party, WalletAddr,
    WalletCache, WalletDescr, WalletTx,
};
use bpstd::{Address, Txid};
use descriptors::Descriptor;
use reqwest::Error;

/// Mempool client
#[derive(Debug, Clone)]
pub struct MempoolClient {
    url: String,
}

/// Get all transactions for the specified address
fn get_scripthash_txs_all(
    mempool_client: &MempoolClient,
    address: &String,
) -> Result<Vec<esplora::Tx>, Error> {
    const PAGE_SIZE: usize = 25;
    let mut res = Vec::new();
    let mut last_seen = None;
    loop {
        let r = mempool_client.scripthash_txs(address, last_seen)?;
        match &r[..] {
            [a @ .., esplora::Tx { txid, .. }] if a.len() >= PAGE_SIZE - 1 => {
                last_seen = Some(*txid);
                res.extend(r);
            }
            _ => {
                res.extend(r);
                break;
            }
        }
    }
    Ok(res)
}

impl MempoolClient {
    /// Create a new instance of the MempoolClient
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Get confirmed transaction history for the specified address,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub fn scripthash_txs(
        &self,
        address: &String,
        last_seen: Option<Txid>,
    ) -> Result<Vec<esplora::Tx>, Error> {
        let blocking_client = reqwest::blocking::Client::new();

        let url = match last_seen {
            Some(last_seen) => format!("{}/address/{}/txs/chain/{}", self.url, address, last_seen),
            None => format!("{}/address/{}/txs", self.url, address),
        };
        let resp = blocking_client.get(&url).send()?.json()?;
        Ok(resp)
    }
}

impl Indexer for MempoolClient {
    type Error = Error;

    /// Create a new wallet cache from the mempool client
    fn create<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descriptor: &WalletDescr<K, D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Self::Error>> {
        let mut cache = WalletCache::new();
        let mut errors = vec![];

        let mut address_index = BTreeMap::new();
        for keychain in descriptor.keychains() {
            let mut empty_count = 0usize;
            eprint!(" keychain {keychain} ");
            for derive in descriptor.addresses(keychain) {
                let address = derive.addr.to_string();
                let script = derive.addr.script_pubkey();

                eprint!(".");
                let mut txids = Vec::new();
                match get_scripthash_txs_all(self, &address) {
                    Err(err) => {
                        errors.push(err);
                        break;
                    }
                    Ok(txes) if txes.is_empty() => {
                        empty_count += 1;
                        if empty_count >= BATCH_SIZE as usize {
                            break;
                        }
                    }
                    Ok(txes) => {
                        empty_count = 0;
                        txids = txes.iter().map(|tx| tx.txid).collect();
                        cache
                            .tx
                            .extend(txes.into_iter().map(WalletTx::from).map(|tx| (tx.txid, tx)));
                    }
                }

                let wallet_addr = WalletAddr::<i64>::from(derive);
                address_index.insert(script, (wallet_addr, txids));
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
                            cache.utxo.remove(&outpoint);
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

        if errors.is_empty() {
            MayError::ok(cache)
        } else {
            MayError::err(cache, errors)
        }
    }

    fn update<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        _descr: &WalletDescr<K, D, L2::Descr>,
        _cache: &mut WalletCache<L2::Cache>,
    ) -> MayError<usize, Vec<Self::Error>> {
        todo!()
    }
}
