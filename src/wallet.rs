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
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::marker::PhantomData;
use std::ops::{AddAssign, Deref, DerefMut};
#[cfg(feature = "fs")]
use std::path::PathBuf;

use bpstd::{
    Address, AddressNetwork, DerivedAddr, Descriptor, Idx, IdxBase, Keychain, Network, NormalIndex,
    Outpoint, Sats, Txid, Vout,
};
use psbt::{PsbtConstructor, Utxo};

use crate::{
    BlockInfo, CoinRow, Indexer, Layer2, Layer2Cache, Layer2Data, Layer2Descriptor, MayError,
    MiningInfo, NoLayer2, TxRow, WalletAddr, WalletTx, WalletUtxo,
};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(doc_comments)]
pub enum NonWalletItem {
    /// transaction {0} is not known to the wallet.
    NonWalletTx(Txid),
    /// transaction {0} doesn't contains output number {1}.
    NoOutput(Txid, Vout),
    /// transaction output {0} doesn't belong to the wallet.
    NonWalletUtxo(Outpoint),
}

pub struct AddrIter<'descr, K, D: Descriptor<K>> {
    generator: &'descr D,
    network: AddressNetwork,
    keychain: Keychain,
    index: NormalIndex,
    _phantom: PhantomData<K>,
}

impl<'descr, K, D: Descriptor<K>> Iterator for AddrIter<'descr, K, D> {
    type Item = DerivedAddr;

    fn next(&mut self) -> Option<Self::Item> {
        let addr = self.generator.derive_address(self.network, self.keychain, self.index).ok()?;
        let derived = DerivedAddr::new(addr, self.keychain, self.index);
        self.index.wrapping_inc_assign();
        Some(derived)
    }
}

#[cfg_attr(
    feature = "serde",
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
#[derive(Getters, Clone, Eq, PartialEq, Debug, Hash)]
pub struct WalletDescr<K, D, L2 = NoLayer2>
where
    D: Descriptor<K>,
    L2: Layer2Descriptor,
{
    generator: D,
    #[getter(as_copy)]
    network: Network,
    layer2: L2,
    #[cfg_attr(feature = "serde", serde(skip))]
    _phantom: PhantomData<K>,
}

impl<K, D: Descriptor<K>> WalletDescr<K, D, NoLayer2> {
    pub fn new_standard(descr: D, network: Network) -> Self {
        WalletDescr {
            generator: descr,
            network,
            layer2: None,
            _phantom: PhantomData,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> WalletDescr<K, D, L2> {
    pub fn new_layer2(descr: D, layer2: L2, network: Network) -> Self {
        WalletDescr {
            generator: descr,
            network,
            layer2,
            _phantom: PhantomData,
        }
    }

    pub fn addresses(&self, keychain: impl Into<Keychain>) -> AddrIter<K, D> {
        AddrIter {
            generator: &self.generator,
            network: self.network.into(),
            keychain: keychain.into(),
            index: NormalIndex::ZERO,
            _phantom: PhantomData,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> Deref for WalletDescr<K, D, L2> {
    type Target = D;

    fn deref(&self) -> &Self::Target { &self.generator }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> DerefMut for WalletDescr<K, D, L2> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.generator }
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(serialize = "L2: serde::Serialize", deserialize = "L2: serde::Deserialize<'de>")
    )
)]
pub struct WalletData<L2: Layer2Data> {
    pub name: String,
    pub tx_annotations: BTreeMap<Txid, String>,
    pub txout_annotations: BTreeMap<Outpoint, String>,
    pub txin_annotations: BTreeMap<Outpoint, String>,
    pub addr_annotations: BTreeMap<Address, String>,
    pub layer2_annotations: L2,
    pub last_used: BTreeMap<Keychain, NormalIndex>,
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(serialize = "L2: serde::Serialize", deserialize = "L2: serde::Deserialize<'de>")
    )
)]
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WalletCache<L2: Layer2Cache> {
    pub last_block: MiningInfo,
    pub last_change: NormalIndex,
    pub headers: BTreeSet<BlockInfo>,
    pub tx: BTreeMap<Txid, WalletTx>,
    pub utxo: BTreeSet<Outpoint>,
    pub addr: BTreeMap<Keychain, BTreeSet<WalletAddr>>,
    pub layer2: L2,
}

impl<L2: Layer2Cache> Default for WalletCache<L2> {
    fn default() -> Self { WalletCache::new() }
}

impl<L2C: Layer2Cache> WalletCache<L2C> {
    pub(crate) fn new() -> Self {
        WalletCache {
            last_block: MiningInfo::genesis(),
            last_change: NormalIndex::ZERO,
            headers: none!(),
            tx: none!(),
            utxo: none!(),
            addr: none!(),
            layer2: none!(),
        }
    }

    pub fn with<I: Indexer, K, D: Descriptor<K>, L2: Layer2<Cache = L2C>>(
        descriptor: &WalletDescr<K, D, L2::Descr>,
        indexer: &I,
    ) -> MayError<Self, Vec<I::Error>> {
        indexer.create::<K, D, L2>(descriptor)
    }

    pub fn update<I: Indexer, K, D: Descriptor<K>, L2: Layer2<Cache = L2C>>(
        &mut self,
        descriptor: &WalletDescr<K, D, L2::Descr>,
        indexer: &I,
    ) -> MayError<usize, Vec<I::Error>> {
        indexer.update::<K, D, L2>(descriptor, self)
    }

    pub fn addresses_on(&self, keychain: Keychain) -> &BTreeSet<WalletAddr> {
        self.addr.get(&keychain).unwrap_or_else(|| {
            panic!("keychain #{keychain} is not supported by the wallet descriptor")
        })
    }

    pub fn utxo(&self, outpoint: Outpoint) -> Result<WalletUtxo, NonWalletItem> {
        let tx = self.tx.get(&outpoint.txid).ok_or(NonWalletItem::NonWalletTx(outpoint.txid))?;
        let debit = tx
            .outputs
            .get(outpoint.vout.into_usize())
            .ok_or(NonWalletItem::NoOutput(outpoint.txid, outpoint.vout))?;
        let terminal = debit.derived_addr().ok_or(NonWalletItem::NonWalletUtxo(outpoint))?.terminal;
        // TODO: Check whether TXO is spend
        Ok(WalletUtxo {
            outpoint,
            value: debit.value,
            terminal,
            status: tx.status,
        })
    }

    pub fn all_utxos(&self) -> impl Iterator<Item = WalletUtxo> + '_ {
        self.utxo.iter().map(|outpoint| {
            let tx = self.tx.get(&outpoint.txid).expect("cache data inconsistency");
            let debit = tx.outputs.get(outpoint.vout_usize()).expect("cache data inconsistency");
            let terminal =
                debit.derived_addr().expect("UTXO doesn't belong to the wallet").terminal;
            // TODO: Check whether TXO is spend
            WalletUtxo {
                outpoint: *outpoint,
                value: debit.value,
                terminal,
                status: tx.status,
            }
        })
    }
}

#[cfg(feature = "fs")]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FsConfig {
    pub path: PathBuf,
    pub autosave: bool,
}

pub trait Save {
    type SaveErr: Error;
    fn save(&self) -> Result<bool, Self::SaveErr>;
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Wallet<K, D: Descriptor<K>, L2: Layer2 = NoLayer2>
where Self: Save
{
    descr: WalletDescr<K, D, L2::Descr>,
    data: WalletData<L2::Data>,
    cache: WalletCache<L2::Cache>,
    layer2: L2,
    #[cfg(feature = "fs")]
    fs: Option<FsConfig>,
    dirty: bool,
}

impl<K, D: Descriptor<K>, L2: Layer2> Deref for Wallet<K, D, L2>
where Self: Save
{
    type Target = WalletDescr<K, D, L2::Descr>;

    fn deref(&self) -> &Self::Target { &self.descr }
}

impl<K, D: Descriptor<K>, L2: Layer2> PsbtConstructor for Wallet<K, D, L2>
where Self: Save
{
    type Key = K;
    type Descr = D;

    fn descriptor(&self) -> &D { &self.descr.generator }

    fn utxo(&self, outpoint: Outpoint) -> Option<Utxo> {
        self.cache.utxo(outpoint).ok().map(WalletUtxo::into_utxo)
    }

    fn network(&self) -> Network { self.descr.network }

    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        let keychain = keychain.into();
        let mut idx = self.last_published_derivation_index(keychain);
        let last_index = self.data.last_used.entry(keychain).or_default();
        idx = cmp::max(*last_index, idx);
        if shift {
            *last_index = idx.saturating_add(1u32);
            self.set_dirty();
        }
        idx
    }
}

impl<K, D: Descriptor<K>> Wallet<K, D>
where Self: Save
{
    pub fn new_layer1(descr: D, network: Network) -> Self {
        Wallet {
            descr: WalletDescr::new_standard(descr, network),
            data: empty!(),
            cache: WalletCache::new(),
            layer2: None,
            dirty: false,
            #[cfg(feature = "fs")]
            fs: None,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, L2>
where Self: Save
{
    pub fn new_layer2(descr: D, l2_descr: L2::Descr, layer2: L2, network: Network) -> Self {
        Wallet {
            descr: WalletDescr::new_layer2(descr, l2_descr, network),
            data: empty!(),
            cache: WalletCache::new(),
            layer2,
            dirty: false,
            #[cfg(feature = "fs")]
            fs: None,
        }
    }

    #[cfg(feature = "fs")]
    pub fn fs_config(&self) -> Option<&FsConfig> { self.fs.as_ref() }

    #[cfg(feature = "fs")]
    pub fn set_fs_config(&mut self, config: FsConfig) -> Result<Option<FsConfig>, fs::StoreError> {
        let mut last = Some(config);
        std::mem::swap(&mut self.fs, &mut last);
        self.set_dirty();
        Ok(last)
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
        #[cfg(feature = "fs")]
        if self.fs.as_ref().map(|fs| fs.autosave).unwrap_or_default() {
            let _ = self.save();
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.data.name = name;
        self.set_dirty();
    }

    pub fn descriptor_mut<R>(
        &mut self,
        f: impl FnOnce(&mut WalletDescr<K, D, L2::Descr>) -> R,
    ) -> R {
        let res = f(&mut self.descr);
        self.set_dirty();
        res
    }

    pub fn update<I: Indexer>(&mut self, indexer: &I) -> MayError<(), Vec<I::Error>> {
        if self.cache.tx.is_empty() {
            return WalletCache::with::<_, K, _, L2>(&self.descr, indexer).map(|cache| {
                self.cache = cache;
                self.set_dirty();
            });
        }

        self.cache.update::<I, K, D, L2>(&self.descr, indexer).map(|_| self.set_dirty())
    }

    pub fn to_deriver(&self) -> D
    where
        D: Clone,
        K: Clone,
    {
        self.descr.clone()
    }

    fn last_published_derivation_index(&self, keychain: impl Into<Keychain>) -> NormalIndex {
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

    pub fn last_derivation_index(&self, keychain: impl Into<Keychain>) -> NormalIndex {
        let keychain = keychain.into();
        let last_index = self.data.last_used.get(&keychain).copied().unwrap_or_default();
        cmp::max(last_index, self.last_published_derivation_index(keychain))
    }

    pub fn next_address(&mut self, keychain: impl Into<Keychain>, shift: bool) -> Address {
        let keychain = keychain.into();
        let index = self.next_derivation_index(keychain, shift);
        self.addresses(keychain)
            .nth(index.index() as usize)
            .expect("address iterator always can produce address")
            .addr
    }

    pub fn balance(&self) -> Sats { self.cache.coins().map(|utxo| utxo.amount).sum::<Sats>() }

    #[inline]
    pub fn transactions(&self) -> &BTreeMap<Txid, WalletTx> { &self.cache.tx }

    #[inline]
    pub fn coins(&self) -> impl Iterator<Item = CoinRow<<L2::Cache as Layer2Cache>::Coin>> + '_ {
        self.cache.coins()
    }

    pub fn address_coins(
        &self,
    ) -> HashMap<DerivedAddr, Vec<CoinRow<<L2::Cache as Layer2Cache>::Coin>>> {
        let map = HashMap::new();
        self.coins().fold(map, |mut acc, txo| {
            acc.entry(txo.address).or_default().push(txo);
            acc
        })
    }

    pub fn address_balance(&self) -> impl Iterator<Item = WalletAddr> + '_ {
        self.cache.addr.values().flat_map(|set| set.iter()).copied()
    }

    #[inline]
    pub fn history(&self) -> impl Iterator<Item = TxRow<<L2::Cache as Layer2Cache>::Tx>> + '_ {
        self.cache.history()
    }

    pub fn all_utxos(&self) -> impl Iterator<Item = WalletUtxo> + '_ { self.cache.all_utxos() }

    pub fn coinselect<'a>(
        &'a self,
        up_to: Sats,
        selector: impl Fn(&WalletUtxo) -> bool + 'a,
    ) -> impl Iterator<Item = Outpoint> + '_ {
        let mut selected = Sats::ZERO;
        self.all_utxos()
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

#[cfg(feature = "fs")]
pub mod fs {
    use std::convert::Infallible;
    use std::error::Error;
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    use amplify::IoError;

    use super::*;

    #[derive(Debug, Display, Error, From)]
    #[display(doc_comments)]
    pub enum LoadError<L2: Error = Infallible> {
        /// I/O error loading wallet - {0}
        #[from]
        #[from(io::Error)]
        Io(IoError),

        /// unable to parse TOML file - {0}
        #[from]
        Toml(toml::de::Error),

        #[display(inner)]
        Layer2(L2),

        #[display(inner)]
        #[from]
        Custom(String),
    }

    #[derive(Debug, Display, Error, From)]
    #[display(doc_comments)]
    pub enum StoreError<L2: Error = Infallible> {
        /// I/O error storing wallet - {0}
        #[from]
        #[from(io::Error)]
        Io(IoError),

        /// unable to serialize wallet data as TOML file - {0}
        #[from]
        Toml(toml::ser::Error),

        /// unable to serialize wallet cache as YAML file - {0}
        #[from]
        Yaml(serde_yaml::Error),

        #[display(inner)]
        Layer2(L2),

        #[display(inner)]
        #[from]
        Custom(String),
    }

    #[derive(Debug, Display)]
    #[display(doc_comments)]
    pub enum Warning {
        /// no cache file is found, initializing with empty cache
        CacheAbsent,
        /// wallet cache damaged or has invalid version; resetting ({0})
        CacheDamaged(serde_yaml::Error),
    }

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
            cache.push("cache.yaml");

            WalletFiles { descr, data, cache }
        }
    }

    impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, L2>
    where
        for<'de> WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Descr: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Data: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Cache: serde::Serialize + serde::Deserialize<'de>,
    {
        pub fn load(
            path: &Path,
            autosave: bool,
        ) -> Result<(Self, Vec<Warning>), LoadError<L2::LoadError>> {
            let mut warnings = Vec::new();

            let files = WalletFiles::new(path);

            let descr = fs::read_to_string(files.descr)?;
            let descr = toml::from_str(&descr)?;

            let data = fs::read_to_string(files.data)?;
            let data = toml::from_str(&data)?;

            let cache = fs::read_to_string(files.cache)
                .map_err(|_| Warning::CacheAbsent)
                .and_then(|cache| serde_yaml::from_str(&cache).map_err(Warning::CacheDamaged))
                .unwrap_or_else(|warn| {
                    warnings.push(warn);
                    WalletCache::default()
                });

            let layer2 = L2::load(path).map_err(LoadError::Layer2)?;

            let fs = Some(FsConfig {
                path: path.to_owned(),
                autosave,
            });

            let wallet = Wallet::<K, D, L2> {
                descr,
                data,
                cache,
                layer2,
                dirty: false,
                fs,
            };
            Ok((wallet, warnings))
        }
    }

    impl<K, D: Descriptor<K>, L2: Layer2> Save for Wallet<K, D, L2>
    where
        for<'de> WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Descr: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Data: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2::Cache: serde::Serialize + serde::Deserialize<'de>,
    {
        type SaveErr = StoreError<L2::StoreError>;

        fn save(&self) -> Result<bool, StoreError<L2::StoreError>> {
            let Some(path) = self.fs.as_ref().map(|fs| &fs.path) else {
                return Ok(false);
            };
            if self.dirty {
                fs::create_dir_all(path)?;
                let files = WalletFiles::new(path);
                fs::write(files.descr, toml::to_string_pretty(&self.descr)?)?;
                fs::write(files.data, toml::to_string_pretty(&self.data)?)?;
                fs::write(files.cache, serde_yaml::to_string(&self.cache)?)?;
                self.layer2.store(path).map_err(StoreError::Layer2)?;
            }

            Ok(true)
        }
    }

    impl<K, D: Descriptor<K>, L2: Layer2> Drop for Wallet<K, D, L2>
    where Wallet<K, D, L2>: Save
    {
        fn drop(&mut self) {
            if self.dirty && self.fs.as_ref().map(|fs| fs.autosave).unwrap_or_default() {
                let _ = self.save();
            }
        }
    }
}

#[cfg(not(feature = "fs"))]
impl<K, D: Descriptor<K>, L2: Layer2> Save for Wallet<K, D, L2> {
    type SaveErr = std::convert::Infallible;

    fn save(&self) -> Result<bool, Self::SaveErr> {
        panic!("Attempt to save wallet with no file system support during compilation");
    }
}
