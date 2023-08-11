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
#[cfg(feature = "serde")]
use serde_with::DisplayFromStr;

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

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(
            serialize = "C: serde::Serialize, D: serde::Serialize",
            deserialize = "C: serde::Deserialize<'de>, D: serde::Deserialize<'de>"
        )
    )
)]
#[derive(Getters, Clone, Eq, PartialEq, Debug)]
pub struct WalletDescr<D, C = Bip32Keychain>
where
    D: DeriveSpk,
    C: Keychain,
{
    pub(crate) script_pubkey: D,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeSet<_>"))]
    pub(crate) keychains: BTreeSet<C>,
    #[getter(as_copy)]
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
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

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct WalletData {
    pub name: String,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub tx_annotations: BTreeMap<Txid, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub txout_annotations: BTreeMap<Outpoint, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub txin_annotations: BTreeMap<Outpoint, String>,
    #[cfg_attr(feature = "serde", serde_as(as = "BTreeMap<DisplayFromStr, _>"))]
    pub addr_annotations: BTreeMap<Address, String>,
    pub last_used: NormalIndex,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase", bound = "")
)]
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WalletCache<C: Keychain> {
    pub(crate) last_height: u32,
    pub(crate) headers: HashMap<NonZeroU32, BlockInfo>,
    pub(crate) tx: HashMap<Txid, TxInfo<C>>,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub(crate) utxo: HashMap<Address, HashSet<UtxoInfo<C>>>,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub(crate) addr: HashMap<Terminal<C>, AddrInfo<C>>,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub(crate) max_known: HashMap<NormalIndex, NormalIndex>,
}

impl<C: Keychain> Default for WalletCache<C> {
    fn default() -> Self {
        WalletCache {
            last_height: 0,
            headers: empty!(),
            tx: empty!(),
            utxo: empty!(),
            addr: empty!(),
            max_known: empty!(),
        }
    }
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

    pub fn set_name(&mut self, name: String) { self.data.name = name; }

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

    impl<D: DeriveSpk, C: Keychain> Wallet<D, C>
    where for<'de> WalletDescr<D, C>: serde::Serialize + serde::Deserialize<'de>
    {
        pub fn load(path: &Path) -> Result<Self, crate::LoadError> {
            let files = WalletFiles::new(path);

            let descr = fs::read_to_string(files.descr)?;
            let descr = toml::from_str(&descr)?;

            let data = fs::read_to_string(files.data)?;
            let data = toml::from_str(&data)?;

            let cache = fs::read_to_string(files.cache)?;
            let cache = toml::from_str(&cache)?;

            Ok(Wallet { descr, data, cache })
        }

        pub fn store(&self, path: &Path) -> Result<(), crate::StoreError> {
            fs::create_dir_all(path)?;
            let files = WalletFiles::new(path);
            fs::write(files.descr, toml::to_string_pretty(&self.descr)?)?;
            fs::write(files.data, toml::to_string_pretty(&self.data)?)?;
            fs::write(files.cache, toml::to_string_pretty(&self.cache)?)?;

            Ok(())
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
