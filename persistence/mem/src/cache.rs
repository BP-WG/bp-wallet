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

use std::collections::{HashMap, HashSet};

use bpwallet::{
    AddressBalance, Counterparty, Descriptor, Keychain, NonWalletItem, NormalIndex, OpType,
    Outpoint, Party, Sats, ScriptPubkey, Tx, Txid, WalletCache, WalletCoin, WalletOperation,
    WalletTx, WalletUtxo,
};

#[derive(Debug)]
pub struct MemCache {
    last_used: HashMap<Keychain, NormalIndex>,
    txes: HashMap<Txid, WalletTx>,
    utxos: HashSet<Outpoint>,
    addrs: HashMap<Keychain, HashSet<AddressBalance>>,
}

impl WalletCache for MemCache {
    type SyncError = ();

    fn transactions(&self) -> impl Iterator<Item = WalletTx> { self.txes.values().cloned() }

    // TODO: Rename WalletUtxo into WalletTxo and add `spent_by` optional field.
    fn txos(&self) -> impl Iterator<Item = WalletUtxo> {
        self.txes.iter().flat_map(|(txid, tx)| {
            tx.outputs.iter().enumerate().filter_map(|(vout, out)| {
                if let Party::Wallet(w) = out.beneficiary {
                    Some(WalletUtxo {
                        outpoint: Outpoint::new(*txid, vout as u32),
                        value: out.value,
                        terminal: w.terminal,
                        status: tx.status,
                    })
                } else {
                    None
                }
            })
        })
    }

    fn utxos(&self) -> impl Iterator<Item = WalletUtxo> {
        self.utxos.iter().filter_map(|outpoint| {
            let tx = self.txes.get(&outpoint.txid).expect("cache data inconsistency");
            let debit = tx.outputs.get(outpoint.vout_usize()).expect("cache data inconsistency");
            let terminal =
                debit.derived_addr().expect("UTXO doesn't belong to the wallet").terminal;
            if debit.spent.is_some() {
                None
            } else {
                Some(WalletUtxo {
                    outpoint: *outpoint,
                    value: debit.value,
                    terminal,
                    status: tx.status,
                })
            }
        })
    }

    fn coins(&self) -> impl Iterator<Item = WalletCoin> {
        self.utxos.iter().map(|outpoint| {
            let tx = self.txes.get(&outpoint.txid).expect("cache data inconsistency");
            let out = tx.outputs.get(outpoint.vout_usize()).expect("cache data inconsistency");
            WalletCoin {
                height: tx.status.map(|info| info.height),
                outpoint: *outpoint,
                address: out.derived_addr().expect("cache data inconsistency"),
                amount: out.value,
            }
        })
    }

    fn history(&self) -> impl Iterator<Item = WalletOperation> {
        self.txes.values().map(|tx| {
            let (credit, debit) = tx.credited_debited();
            let mut row = WalletOperation {
                height: tx.status.map(|info| info.height),
                operation: OpType::Credit,
                our_inputs: tx
                    .inputs
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, inp)| inp.derived_addr().map(|_| idx as u32))
                    .collect(),
                counterparties: none!(),
                own: none!(),
                txid: tx.txid,
                fee: tx.fee,
                weight: tx.weight,
                size: tx.size,
                total: tx.total_moved(),
                amount: Sats::ZERO,
                balance: Sats::ZERO,
            };
            // TODO: Add balance calculation
            row.own = tx
                .inputs
                .iter()
                .filter_map(|i| i.derived_addr().map(|a| (a, -i.value.sats_i64())))
                .chain(
                    tx.outputs
                        .iter()
                        .filter_map(|o| o.derived_addr().map(|a| (a, o.value.sats_i64()))),
                )
                .collect();
            if credit.is_non_zero() {
                row.counterparties = tx.credits().fold(Vec::new(), |mut cp, inp| {
                    let party = Counterparty::from(inp.payer.clone());
                    cp.push((party, inp.value.sats_i64()));
                    cp
                });
                row.counterparties.extend(tx.debits().fold(Vec::new(), |mut cp, out| {
                    let party = Counterparty::from(out.beneficiary.clone());
                    cp.push((party, -out.value.sats_i64()));
                    cp
                }));
                row.operation = OpType::Credit;
                row.amount = credit - debit - tx.fee;
            } else if debit.is_non_zero() {
                row.counterparties = tx.debits().fold(Vec::new(), |mut cp, out| {
                    let party = Counterparty::from(out.beneficiary.clone());
                    cp.push((party, -out.value.sats_i64()));
                    cp
                });
                row.operation = OpType::Debit;
                row.amount = debit;
            }
            row
        })
    }

    fn balances(&self) -> impl Iterator<Item = AddressBalance> {
        self.addrs.values().flatten().cloned()
    }

    fn has_txo(&self, outpoint: Outpoint) -> bool {
        let Some(tx) = self.txes.get(&outpoint.txid) else {
            return false;
        };
        let Some(out) = tx.outputs.get(outpoint.vout.to_usize()) else {
            return false;
        };
        matches!(out.beneficiary, Party::Wallet(_))
    }

    #[inline]
    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.utxos.contains(&outpoint) }

    fn transaction(&self, txid: Txid) -> Result<Tx, NonWalletItem> {
        self.txes.get(&txid).map(|tx| tx.to_tx()).ok_or(NonWalletItem::NonWalletTx(txid))
    }

    fn utxo(&self, outpoint: Outpoint) -> Result<(WalletUtxo, ScriptPubkey), NonWalletItem> {
        let tx = self.txes.get(&outpoint.txid).ok_or(NonWalletItem::NonWalletTx(outpoint.txid))?;
        let debit = tx
            .outputs
            .get(outpoint.vout.into_usize())
            .ok_or(NonWalletItem::NoOutput(outpoint.txid, outpoint.vout))?;
        let terminal = debit.derived_addr().ok_or(NonWalletItem::NonWalletUtxo(outpoint))?.terminal;
        // Check whether TXO is spend
        if debit.spent.is_some() {
            debug_assert!(!self.has_utxo(outpoint));
            return Err(NonWalletItem::Spent(outpoint));
        }
        debug_assert!(self.has_utxo(outpoint));
        let utxo = WalletUtxo {
            outpoint,
            value: debit.value,
            terminal,
            status: tx.status,
        };
        let spk =
            debit.beneficiary.script_pubkey().ok_or(NonWalletItem::NonWalletUtxo(outpoint))?;
        Ok((utxo, spk))
    }

    fn add_tx(&mut self, tx: WalletTx) { self.txes.insert(tx.txid, tx); }

    fn add_utxo(&mut self, utxo: WalletUtxo) { self.utxos.insert(utxo.outpoint); }

    fn last_used(&self, keychain: Keychain) -> Option<NormalIndex> {
        self.last_used.get(&keychain).copied()
    }

    fn set_last_used(&mut self, keychain: Keychain, index: NormalIndex) {
        self.last_used.insert(keychain, index);
    }

    fn sync<K, D: Descriptor<K>>(&mut self, _descriptor: &D) -> Result<(), Self::SyncError> {
        todo!()
    }
}

/*
pub fn with<I: Indexer, K, D: Descriptor<K>>(
    descriptor: &D,
    indexer: &I,
) -> MayError<Self, Vec<I::Error>> {
    indexer.create::<K, D>(descriptor)
}

pub fn update<I: Indexer, K, D: Descriptor<K>>(
    &mut self,
    descriptor: &D,
    indexer: &I,
) -> MayError<usize, Vec<I::Error>> {
    let res = indexer.update::<K, D>(descriptor, self);
    self.mark_dirty();
    res
}
 */
