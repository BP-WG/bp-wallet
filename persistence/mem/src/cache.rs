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

use std::collections::{BTreeMap, BTreeSet};

use bpwallet::{
    AddressBalance, Counterparty, Keychain, NonWalletItem, OpType, Outpoint, Party, Sats,
    ScriptPubkey, Txid, WalletCoin, WalletOperation, WalletTx, WalletUtxo,
};

#[derive(Debug)]
pub struct MemCache {
    txes: BTreeMap<Txid, WalletTx>,
    utxos: BTreeSet<Outpoint>,
    addrs: BTreeMap<Keychain, BTreeSet<AddressBalance>>,
}

impl MemCache {
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

    pub fn addresses_on(&self, keychain: Keychain) -> &BTreeSet<AddressBalance> {
        self.addrs.get(&keychain).unwrap_or_else(|| {
            panic!("keychain #{keychain} is not supported by the wallet descriptor")
        })
    }

    pub fn has_outpoint(&self, outpoint: Outpoint) -> bool {
        let Some(tx) = self.txes.get(&outpoint.txid) else {
            return false;
        };
        let Some(out) = tx.outputs.get(outpoint.vout.to_usize()) else {
            return false;
        };
        matches!(out.beneficiary, Party::Wallet(_))
    }

    #[inline]
    pub fn is_unspent(&self, outpoint: Outpoint) -> bool { self.utxos.contains(&outpoint) }

    pub fn outpoint_by(
        &self,
        outpoint: Outpoint,
    ) -> Result<(WalletUtxo, ScriptPubkey), NonWalletItem> {
        let tx = self.txes.get(&outpoint.txid).ok_or(NonWalletItem::NonWalletTx(outpoint.txid))?;
        let debit = tx
            .outputs
            .get(outpoint.vout.into_usize())
            .ok_or(NonWalletItem::NoOutput(outpoint.txid, outpoint.vout))?;
        let terminal = debit.derived_addr().ok_or(NonWalletItem::NonWalletUtxo(outpoint))?.terminal;
        // Check whether TXO is spend
        if debit.spent.is_some() {
            debug_assert!(!self.is_unspent(outpoint));
            return Err(NonWalletItem::Spent(outpoint));
        }
        debug_assert!(self.is_unspent(outpoint));
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

    // TODO: Rename WalletUtxo into WalletTxo and add `spent_by` optional field.
    pub fn txos(&self) -> impl Iterator<Item = WalletUtxo> + '_ {
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

    pub fn utxos(&self) -> impl Iterator<Item = WalletUtxo> + '_ {
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

    pub fn coins(&self) -> impl Iterator<Item = WalletCoin> + '_ {
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

    pub fn history(&self) -> impl Iterator<Item = WalletOperation> + '_ {
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
}
