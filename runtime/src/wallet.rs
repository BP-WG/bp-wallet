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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::num::NonZeroU32;
use std::ops::Deref;

use bp::{
    Address, AddressNetwork, Chain, DeriveSpk, DerivedAddr, Idx, NormalIndex, Outpoint, Sats,
    Terminal, Txid,
};
#[cfg(feature = "serde")]
use serde_with::DisplayFromStr;

use crate::{
    AddrInfo, BlockInfo, Indexer, Layer2, Layer2Cache, Layer2Data, Layer2Descriptor, MayError,
    NoLayer2, TxInfo, TxoInfo,
};

pub struct AddrIter<'descr, D: DeriveSpk> {
    script_pubkey: &'descr D,
    network: AddressNetwork,
    keychain: u8,
    index: NormalIndex,
}

impl<'descr, D: DeriveSpk> Iterator for AddrIter<'descr, D> {
    type Item = DerivedAddr;

    fn next(&mut self) -> Option<Self::Item> {
        let addr =
            self.script_pubkey.derive_address(self.network, self.keychain, self.index).ok()?;
        let derived = DerivedAddr::new(addr, self.keychain, self.index);
        self.index.wrapping_inc_assign();
        Some(derived)
    }
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(
            serialize = "D: serde::Serialize, L2: serde::Serialize",
            deserialize = "D: serde::Deserialize<'de>, L2: serde::Deserialize<'de>"
        )
    )
)]
#[derive(Getters, Clone, Eq, PartialEq, Debug)]
pub struct WalletDescr<D, L2 = NoLayer2>
where
    D: DeriveSpk,
    L2: Layer2Descriptor,
{
    pub(crate) script_pubkey: D,
    #[getter(as_copy)]
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub(crate) chain: Chain,
    pub(crate) layer2: L2,
}

impl<D: DeriveSpk> WalletDescr<D, NoLayer2> {
    pub fn new_standard(descr: D, network: Chain) -> Self {
        WalletDescr {
            script_pubkey: descr,
            chain: network,
            layer2: None,
        }
    }
}

impl<D: DeriveSpk, L2: Layer2Descriptor> WalletDescr<D, L2> {
    pub fn new_layer2(descr: D, layer2: L2, network: Chain) -> Self {
        WalletDescr {
            script_pubkey: descr,
            chain: network,
            layer2,
        }
    }

    pub fn addresses(&self, keychain: u8) -> AddrIter<D> {
        AddrIter {
            script_pubkey: &self.script_pubkey,
            network: self.chain.into(),
            keychain,
            index: NormalIndex::ZERO,
        }
    }
}

impl<D: DeriveSpk, L2: Layer2Descriptor> Deref for WalletDescr<D, L2> {
    type Target = D;

    fn deref(&self) -> &Self::Target { &self.script_pubkey }
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(serialize = "L2: serde::Serialize", deserialize = "L2: serde::Deserialize<'de>")
    )
)]
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct WalletData<L2: Layer2Data> {
    pub name: String,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub tx_annotations: BTreeMap<Txid, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub txout_annotations: BTreeMap<Outpoint, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub txin_annotations: BTreeMap<Outpoint, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub addr_annotations: BTreeMap<Address, String>,
    pub layer2_annotations: L2,
    pub last_used: NormalIndex,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(serialize = "L2: serde::Serialize", deserialize = "L2: serde::Deserialize<'de>")
    )
)]
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WalletCache<L2: Layer2Cache> {
    pub(crate) last_height: u32,
    pub(crate) headers: HashMap<NonZeroU32, BlockInfo>,
    pub(crate) tx: HashMap<Txid, TxInfo>,
    pub(crate) outputs: HashSet<TxoInfo>,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub(crate) addr: HashMap<Terminal, AddrInfo>,
    pub(crate) layer2: L2,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub(crate) max_known: HashMap<u8, NormalIndex>,
}

impl<L2: Layer2Cache> Default for WalletCache<L2> {
    fn default() -> Self {
        WalletCache {
            last_height: 0,
            headers: empty!(),
            tx: empty!(),
            outputs: empty!(),
            addr: empty!(),
            layer2: empty!(),
            max_known: empty!(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Wallet<D: DeriveSpk, L2: Layer2 = NoLayer2> {
    pub(crate) descr: WalletDescr<D, L2::Descr>,
    pub(crate) data: WalletData<L2::Data>,
    pub(crate) cache: WalletCache<L2::Cache>,
    pub(crate) layer2: L2,
}

impl<D: DeriveSpk, L2: Layer2> Deref for Wallet<D, L2> {
    type Target = WalletDescr<D, L2::Descr>;

    fn deref(&self) -> &Self::Target { &self.descr }
}

impl<D: DeriveSpk> Wallet<D, NoLayer2> {
    pub fn new_standard(descr: D, network: Chain) -> Self {
        Wallet {
            descr: WalletDescr::new_standard(descr, network),
            data: empty!(),
            cache: WalletCache::new(),
            layer2: None,
        }
    }

    pub fn with_standard<I: Indexer>(
        descr: D,
        network: Chain,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        let mut wallet = Wallet::new_standard(descr, network);
        wallet.update(indexer).map(|_| wallet)
    }
}

impl<D: DeriveSpk, L2: Layer2> Wallet<D, L2> {
    pub fn new_layer2(descr: D, l2_descr: L2::Descr, layer2: L2, network: Chain) -> Self {
        Wallet {
            descr: WalletDescr::new_layer2(descr, l2_descr, network),
            data: empty!(),
            cache: WalletCache::new(),
            layer2,
        }
    }

    pub fn with_layer2<I: Indexer>(
        descr: D,
        l2_descr: L2::Descr,
        layer2: L2,
        network: Chain,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        let mut wallet = Wallet::new_layer2(descr, l2_descr, layer2, network);
        wallet.update(indexer).map(|_| wallet)
    }

    pub fn restore(
        descr: WalletDescr<D, L2::Descr>,
        data: WalletData<L2::Data>,
        cache: WalletCache<L2::Cache>,
        layer2: L2,
    ) -> Self {
        Wallet {
            descr,
            data,
            cache,
            layer2,
        }
    }

    pub fn set_name(&mut self, name: String) { self.data.name = name; }

    pub fn update<B: Indexer>(&mut self, blockchain: &B) -> MayError<(), Vec<B::Error>> {
        WalletCache::with::<_, _, L2>(&self.descr, blockchain).map(|cache| self.cache = cache)
    }

    pub fn balance(&self) -> Sats {
        self.cache
            .outputs
            .iter()
            .filter(|utxo| utxo.spent.is_none())
            .map(|utxo| utxo.value)
            .sum::<Sats>()
    }

    pub fn coins(&self) -> impl Iterator<Item = TxoInfo> + '_ {
        self.cache.outputs.iter().filter(|utxo| utxo.spent.is_none()).copied()
    }

    pub fn address_coins(
        &self,
    ) -> impl Iterator<Item = (Address, impl Iterator<Item = TxoInfo> + '_)> + '_ {
        self.coins()
            .fold(HashMap::<_, HashSet<TxoInfo>>::new(), |mut acc, txo| {
                acc.entry(txo.address).or_default().insert(txo);
                acc
            })
            .into_iter()
            .map(|(k, v)| (k, v.into_iter()))
    }

    pub fn address_all(&self, keychain: u8) -> impl Iterator<Item = AddrInfo> + '_ {
        self.descr.addresses(keychain).map(|derived| match self.cache.addr.get(&derived.terminal) {
            None => AddrInfo::from(derived),
            Some(info) => *info,
        })
    }

    pub fn derivation_index_tip(&self, keychain: u8) -> NormalIndex {
        let last_known = self.cache.max_known.get(&keychain).copied().unwrap_or_default();
        if keychain == 0 {
            self.data.last_used.max(last_known)
        } else {
            last_known
        }
    }
}

#[cfg(feature = "fs")]
mod fs {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    struct WalletFiles {
        pub descr: PathBuf,
        pub data: PathBuf,
        pub cache: PathBuf,
    }

    impl WalletFiles {
        pub fn new(path: &Path) -> Self {
            let mut descr = path.to_owned();
            descr.push("descriptor.toml");

            let mut data = path.to_owned();
            data.push("data.toml");

            let mut cache = path.to_owned();
            cache.push("cache.toml");

            WalletFiles { descr, data, cache }
        }
    }

    impl<D: DeriveSpk, L2: Layer2> Wallet<D, L2>
    where
        for<'de> WalletDescr<D>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Descr: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Data: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Cache: serde::Serialize + serde::Deserialize<'de>,
    {
        pub fn load(path: &Path) -> Result<Self, crate::LoadError<L2::LoadError>> {
            let files = WalletFiles::new(path);

            let descr = fs::read_to_string(files.descr)?;
            let descr = toml::from_str(&descr)?;

            let data = fs::read_to_string(files.data)?;
            let data = toml::from_str(&data)?;

            let cache = fs::read_to_string(files.cache)?;
            let cache = toml::from_str(&cache)?;

            let layer2 = L2::load(path).map_err(crate::LoadError::Layer2)?;

            Ok(Wallet {
                descr,
                data,
                cache,
                layer2,
            })
        }

        pub fn store(&self, path: &Path) -> Result<(), crate::StoreError<L2::StoreError>> {
            fs::create_dir_all(path)?;
            let files = WalletFiles::new(path);
            fs::write(files.descr, toml::to_string_pretty(&self.descr)?)?;
            fs::write(files.data, toml::to_string_pretty(&self.data)?)?;
            fs::write(files.cache, toml::to_string_pretty(&self.cache)?)?;
            self.layer2.store(path).map_err(crate::StoreError::Layer2)?;

            Ok(())
        }
    }
}

impl<L2: Layer2Cache> WalletCache<L2>
where L2: Default
{
    pub(crate) fn new() -> Self {
        WalletCache {
            last_height: 0,
            headers: none!(),
            tx: none!(),
            outputs: none!(),
            addr: none!(),
            layer2: none!(),
            max_known: none!(),
        }
    }
}

impl<L2C: Layer2Cache> WalletCache<L2C> {
    pub fn with<I: Indexer, D: DeriveSpk, L2: Layer2<Cache = L2C>>(
        descriptor: &WalletDescr<D, L2::Descr>,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        indexer.create::<_, L2>(descriptor)
    }

    pub fn update<I: Indexer, D: DeriveSpk, L2: Layer2<Cache = L2C>>(
        &mut self,
        descriptor: &WalletDescr<D, L2::Descr>,
        indexer: &I,
    ) -> (usize, Vec<I::Error>) {
        indexer.update::<_, L2>(descriptor, self)
    }
}
