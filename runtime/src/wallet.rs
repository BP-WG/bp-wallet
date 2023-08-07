// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2023 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2023 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2023 Dr Maxim Orlovsky. All rights reserved.
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroU32;
use std::ops::Deref;

use bp::{
    Address, AddressNetwork, Bip32Keychain, Chain, DeriveSpk, DerivedAddr, Idx, Keychain,
    NormalIndex, Outpoint, Sats, Terminal, Txid,
};

use crate::{AddrInfo, BlockInfo, Indexer, MayError, TxInfo, UtxoInfo};

pub struct AddrIter<'descr, D: DeriveSpk, C: Keychain> {
    script_pubkey: &'descr D,
    network: AddressNetwork,
    keychain: C,
    index: NormalIndex,
}

impl<'descr, D: DeriveSpk, C: Keychain> Iterator for AddrIter<'descr, D, C> {
    type Item = DerivedAddr<C>;
    fn next(&mut self) -> Option<Self::Item> {
        let addr =
            self.script_pubkey.derive_address(self.network, self.keychain, self.index).ok()?;
        let derived = DerivedAddr::new(addr, self.keychain, self.index);
        self.index.wrapping_inc_assign();
        Some(derived)
    }
}

#[derive(Getters, Clone, Eq, PartialEq, Debug)]
pub struct WalletDescr<D, C = Bip32Keychain>
where
    D: DeriveSpk,
    C: Keychain,
{
    pub(crate) script_pubkey: D,
    pub(crate) keychains: BTreeSet<C>,
    #[getter(as_copy)]
    pub(crate) chain: Chain,
}

impl<D: DeriveSpk, C: Keychain> WalletDescr<D, C> {
    pub fn new_standard(descr: D, network: Chain) -> Self {
        WalletDescr {
            script_pubkey: descr,
            keychains: C::STANDARD_SET.iter().copied().collect(),
            chain: network,
        }
    }

    pub fn with_keychains(descr: D, network: Chain, keychain: impl IntoIterator<Item = C>) -> Self {
        WalletDescr {
            script_pubkey: descr,
            keychains: keychain.into_iter().collect(),
            chain: network,
        }
    }

    pub fn addresses(&self) -> AddrIter<D, C> {
        AddrIter {
            script_pubkey: &self.script_pubkey,
            network: self.chain.into(),
            keychain: *self.keychains.first().expect("keychain must contain at least one index"),
            index: NormalIndex::ZERO,
        }
    }
}

impl<D: DeriveSpk, C: Keychain> Deref for WalletDescr<D, C> {
    type Target = D;

    fn deref(&self) -> &Self::Target { &self.script_pubkey }
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct WalletData {
    pub name: String,
    pub tx_annotations: BTreeMap<Txid, String>,
    pub txout_annotations: BTreeMap<Outpoint, String>,
    pub txin_annotations: BTreeMap<Outpoint, String>,
    pub addr_annotations: BTreeMap<Address, String>,
    pub last_used: NormalIndex,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WalletCache<C: Keychain> {
    pub(crate) last_height: u32,
    pub(crate) headers: HashMap<NonZeroU32, BlockInfo>,
    pub(crate) tx: HashMap<Txid, TxInfo<C>>,
    pub(crate) utxo: HashMap<Address, HashSet<UtxoInfo<C>>>,
    pub(crate) addr: HashMap<Terminal<C>, AddrInfo<C>>,
    pub(crate) max_known: HashMap<NormalIndex, NormalIndex>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Wallet<D: DeriveSpk, C: Keychain = Bip32Keychain> {
    pub(crate) descr: WalletDescr<D, C>,
    pub(crate) data: WalletData,
    pub(crate) cache: WalletCache<C>,
}

impl<D: DeriveSpk, C: Keychain> Deref for Wallet<D, C> {
    type Target = WalletDescr<D, C>;

    fn deref(&self) -> &Self::Target { &self.descr }
}

impl<D: DeriveSpk, C: Keychain> Wallet<D, C> {
    pub fn new(descr: D, network: Chain) -> Self {
        Wallet {
            descr: WalletDescr::new_standard(descr, network),
            data: empty!(),
            cache: WalletCache::new(),
        }
    }

    pub fn with<I: Indexer>(
        descr: D,
        network: Chain,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        let mut wallet = Wallet::new(descr, network);
        wallet.update(indexer).map(|_| wallet)
    }

    pub fn restore(descr: WalletDescr<D, C>, data: WalletData, cache: WalletCache<C>) -> Self {
        Wallet { descr, data, cache }
    }

    pub fn update<B: Indexer>(&mut self, blockchain: &B) -> MayError<(), Vec<B::Error>> {
        WalletCache::with(&self.descr, blockchain).map(|cache| self.cache = cache)
    }

    pub fn balance(&self) -> Sats {
        self.cache.utxo.values().flatten().map(|utxo| utxo.value).sum::<Sats>()
    }

    pub fn coins(&self) -> impl Iterator<Item = UtxoInfo<C>> + '_ {
        self.cache.utxo.values().flatten().copied()
    }

    pub fn address_coins(
        &self,
    ) -> impl Iterator<Item = (Address, impl Iterator<Item = UtxoInfo<C>> + '_)> + '_ {
        self.cache.utxo.iter().map(|(k, v)| (*k, v.iter().copied()))
    }

    pub fn address_all(&self) -> impl Iterator<Item = AddrInfo<C>> + '_ {
        self.descr.addresses().map(|derived| match self.cache.addr.get(&derived.terminal) {
            None => AddrInfo::from(derived),
            Some(info) => *info,
        })
    }

    pub fn derivation_index_tip(&self, keychain: NormalIndex) -> NormalIndex {
        let last_known = self.cache.max_known.get(&keychain).copied().unwrap_or_default();
        if keychain == NormalIndex::ZERO {
            self.data.last_used.max(last_known)
        } else {
            last_known
        }
    }
}

impl<C: Keychain> WalletCache<C> {
    pub(crate) fn new() -> Self {
        WalletCache {
            last_height: 0,
            headers: none!(),
            tx: none!(),
            utxo: none!(),
            addr: none!(),
            max_known: none!(),
        }
    }
}

impl<C: Keychain> WalletCache<C> {
    pub fn with<I: Indexer, D: DeriveSpk>(
        descriptor: &WalletDescr<D, C>,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        indexer.create(descriptor)
    }

    pub fn update<I: Indexer, D: DeriveSpk>(
        &mut self,
        descriptor: &WalletDescr<D, C>,
        indexer: &I,
    ) -> (usize, Vec<I::Error>) {
        indexer.update(descriptor, self)
    }
}
