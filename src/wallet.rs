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
    Outpoint, Sats, ScriptPubkey, Txid,
};
use psbt::{Psbt, PsbtConstructor, PsbtMeta, Utxo};

use crate::{CoinRow, TxRow, WalletAddr, WalletTx, WalletUtxo};

pub trait WalletCache {
    type SyncError;

    fn transactions(&self) -> impl Iterator<Item = (Txid, WalletTx)>;
    fn txos(&self) -> impl Iterator<Item = WalletUtxo>;
    fn utxos(&self) -> impl Iterator<Item = WalletUtxo>;
    fn coins(&self) -> impl Iterator<Item = CoinRow>;
    fn history(&self) -> impl Iterator<Item = TxRow>;
    fn balances(&self) -> impl Iterator<Item = WalletAddr>;
    fn outpoint(&self, outpoint: Outpoint) -> Option<(WalletUtxo, ScriptPubkey)>;

    fn has_outpoint(&self, outpoint: Outpoint) -> bool;
    fn is_unspent(&self, outpoint: Outpoint) -> bool;

    fn last_used(&self, keychain: Keychain) -> Option<NormalIndex>;
    fn set_last_used(&mut self, keychain: Keychain, index: NormalIndex);

    fn update<K, D: Descriptor<K>>(&mut self, descriptor: &D) -> Result<(), Self::SyncError>;

    fn register_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta);
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
