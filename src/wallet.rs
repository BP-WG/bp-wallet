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

use std::cmp;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{AddAssign, Deref};

use bpstd::{
    Address, AddressNetwork, DerivedAddr, Descriptor, Idx, Keychain, Network, NormalIndex,
    Outpoint, Sats, ScriptPubkey, Txid, Vout,
};
use psbt::{Psbt, PsbtConstructor, PsbtMeta, Utxo};

use crate::{CoinRow, Party, TxCredit, TxDebit, TxRow, TxStatus, WalletAddr, WalletTx, WalletUtxo};

pub trait WalletCache {
    type SyncError;

    fn transactions(&self) -> impl Iterator<Item = (Txid, WalletTx)>;
    fn txos(&self) -> impl Iterator<Item = WalletUtxo>;
    fn utxos(&self) -> impl Iterator<Item = WalletUtxo>;
    fn coins(&self) -> impl Iterator<Item = CoinRow>;
    fn history(&self) -> impl Iterator<Item = TxRow>;
    fn balances(&self) -> impl Iterator<Item = WalletAddr>;

    fn has_outpoint(&self, outpoint: Outpoint) -> bool;
    fn outpoint(&self, outpoint: Outpoint) -> Option<(WalletUtxo, ScriptPubkey)>;
    fn is_unspent(&self, outpoint: Outpoint) -> bool;

    fn add_tx(&mut self, tx: WalletTx);
    fn add_utxo(&mut self, utxo: WalletUtxo);

    fn last_used(&self, keychain: Keychain) -> Option<NormalIndex>;
    fn set_last_used(&mut self, keychain: Keychain, index: NormalIndex);

    // NB: The indexer is internalized inside a concrete cache implementation
    fn update<K, D: Descriptor<K>>(&mut self, descriptor: &D) -> Result<(), Self::SyncError>;

    fn register_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta) {
        let unsigned_tx = psbt.to_unsigned_tx();
        let txid = unsigned_tx.txid();
        let inputs = psbt
            .inputs()
            .map(|input| {
                let addr = Address::with(&input.prev_txout().script_pubkey, meta.network).ok();
                TxCredit {
                    outpoint: input.previous_outpoint,
                    payer: match (self.outpoint(input.previous_outpoint), addr) {
                        (Some((utxo, _)), Some(addr)) => Party::Wallet(DerivedAddr::new(
                            addr,
                            utxo.terminal.keychain,
                            utxo.terminal.index,
                        )),
                        (_, Some(addr)) => Party::Counterparty(addr),
                        _ => Party::Unknown(input.prev_txout().script_pubkey.clone()),
                    },
                    sequence: unsigned_tx.inputs[input.index()].sequence,
                    coinbase: false,
                    script_sig: none!(),
                    witness: none!(),
                    value: input.value(),
                }
            })
            .collect();
        let outputs = psbt
            .outputs()
            .map(|output| {
                let vout = Vout::from_u32(output.index() as u32);
                let addr = Address::with(&output.script, meta.network).ok();
                TxDebit {
                    outpoint: Outpoint::new(txid, vout),
                    beneficiary: match (meta.change, addr) {
                        (Some(change), Some(addr)) if change.vout == vout => Party::Wallet(
                            DerivedAddr::new(addr, change.terminal.keychain, change.terminal.index),
                        ),
                        (_, Some(addr)) => Party::Counterparty(addr),
                        (_, _) => Party::Unknown(output.script.clone()),
                    },
                    value: output.value(),
                    spent: None,
                }
            })
            .collect();

        let wallet_tx = WalletTx {
            txid,
            status: TxStatus::Mempool,
            inputs,
            outputs,
            fee: meta.fee,
            size: meta.size,
            weight: meta.weight,
            version: unsigned_tx.version,
            locktime: unsigned_tx.lock_time,
        };
        for output in &wallet_tx.outputs {
            if let Party::Wallet(derived_addr) = output.beneficiary {
                self.add_utxo(WalletUtxo {
                    outpoint: output.outpoint,
                    value: output.value,
                    terminal: derived_addr.terminal,
                    status: TxStatus::Mempool,
                });
            }
        }
        self.add_tx(wallet_tx);
    }
}

#[derive(Clone, Debug)]
pub struct Wallet<K, D: Descriptor<K>, C: WalletCache> {
    descriptor: D,
    network: Network,
    cache: C,
    _phantom: PhantomData<K>,
}

impl<K, D: Descriptor<K>, C: WalletCache> Deref for Wallet<K, D, C> {
    type Target = D;

    fn deref(&self) -> &Self::Target { &self.descriptor }
}

impl<K, D: Descriptor<K>, C: WalletCache> PsbtConstructor for Wallet<K, D, C> {
    type Key = K;
    type Descr = D;

    fn descriptor(&self) -> &D { &self.descriptor }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
        self.cache.outpoint(outpoint).map(|(utxo, spk)| (utxo.into_utxo(), spk))
    }

    fn network(&self) -> Network { self.network }

    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        let keychain = keychain.into();
        let mut idx = self.next_published_derivation_index(keychain);
        let mut last_index = self.cache.last_used(keychain).unwrap_or_default();
        idx = cmp::max(last_index, idx);
        if shift {
            last_index = idx.saturating_add(1u32);
            self.cache.set_last_used(keychain, last_index);
        }
        idx
    }

    fn after_construct_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta) {
        debug_assert_eq!(AddressNetwork::from(self.network), meta.network);
        self.cache.register_psbt(psbt, meta);
    }
}

impl<K, D: Descriptor<K>, C: WalletCache> Wallet<K, D, C> {
    pub fn with(network: Network, descriptor: D, cache: C) -> Wallet<K, D, C> {
        Self {
            descriptor,
            network,
            cache,
            _phantom: PhantomData,
        }
    }

    pub fn into_components(self) -> (D, C) { (self.descriptor, self.cache) }

    pub fn update(&mut self) -> Result<(), C::SyncError> {
        self.cache.update::<K, D>(&self.descriptor)
    }

    fn next_published_derivation_index(&self, keychain: impl Into<Keychain>) -> NormalIndex {
        let keychain = keychain.into();
        self.address_coins()
            .keys()
            .filter(|ad| ad.terminal.keychain == keychain)
            .map(|ad| ad.terminal.index)
            .max()
            .as_ref()
            .map(NormalIndex::saturating_inc)
            .unwrap_or_default()
    }

    pub fn next_address(&mut self, keychain: impl Into<Keychain>, shift: bool) -> Address {
        let keychain = keychain.into();
        let index = self.next_derivation_index(keychain, shift);
        let spk = self
            .descriptor
            .derive(keychain, index)
            .next()
            .expect("wallet descriptor mush always be able to produce a derivation")
            .to_script_pubkey();
        Address::with(&spk, self.network).expect("non-standard script pubkey")
    }

    pub fn balance(&self) -> Sats { self.cache.coins().map(|utxo| utxo.amount).sum::<Sats>() }

    #[inline]
    pub fn transactions(&self) -> impl Iterator<Item = (Txid, WalletTx)> + '_ {
        self.cache.transactions()
    }

    #[inline]
    pub fn coins(&self) -> impl Iterator<Item = CoinRow> + '_ { self.cache.coins() }

    pub fn address_coins(&self) -> HashMap<DerivedAddr, Vec<CoinRow>> {
        let map = HashMap::new();
        self.coins().fold(map, |mut acc, txo| {
            acc.entry(txo.address).or_default().push(txo);
            acc
        })
    }

    #[inline]
    pub fn balances(&self) -> impl Iterator<Item = WalletAddr> + '_ { self.cache.balances() }

    #[inline]
    pub fn history(&self) -> impl Iterator<Item = TxRow> + '_ { self.cache.history() }

    #[inline]
    pub fn has_outpoint(&self, outpoint: Outpoint) -> bool { self.cache.has_outpoint(outpoint) }

    #[inline]
    pub fn is_unspent(&self, outpoint: Outpoint) -> bool { self.cache.is_unspent(outpoint) }

    #[inline]
    pub fn outpoint_by(&self, outpoint: Outpoint) -> Option<(WalletUtxo, ScriptPubkey)> {
        self.cache.outpoint(outpoint)
    }

    #[inline]
    pub fn txos(&self) -> impl Iterator<Item = WalletUtxo> + '_ { self.cache.txos() }

    #[inline]
    pub fn utxos(&self) -> impl Iterator<Item = WalletUtxo> + '_ { self.cache.utxos() }

    pub fn coinselect<'a>(
        &'a self,
        up_to: Sats,
        selector: impl Fn(&WalletUtxo) -> bool + 'a,
    ) -> impl Iterator<Item = Outpoint> + 'a {
        let mut selected = Sats::ZERO;
        self.utxos()
            .filter(selector)
            .take_while(move |utxo| {
                if selected <= up_to {
                    selected.add_assign(utxo.value);
                    true
                } else {
                    false
                }
            })
            .map(|utxo| utxo.outpoint)
    }
}
