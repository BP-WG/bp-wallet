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

use std::borrow::Cow;
use std::cmp;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::marker::PhantomData;
use std::ops::{AddAssign, Deref};

use bpstd::{
    Address, AddressNetwork, DerivedAddr, Descriptor, Idx, IdxBase, Keychain, Network, NormalIndex,
    Outpoint, Sats, ScriptPubkey, Txid, Vout,
};
use nonasync::persistence::{
    CloneNoPersistence, Persistence, PersistenceError, PersistenceProvider, Persisting,
};
use psbt::{Psbt, PsbtConstructor, PsbtMeta, Utxo};

use crate::{
    BlockInfo, CoinRow, Counterparty, Indexer, Layer2, Layer2Cache, Layer2Data, Layer2Descriptor,
    Layer2Empty, MayError, MiningInfo, NoLayer2, OpType, Party, TxCredit, TxDebit, TxRow, TxStatus,
    WalletAddr, WalletTx, WalletUtxo,
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
    /// transaction output {0} is spent.
    Spent(Outpoint),
}

pub struct AddrIter<'descr, K, D: Descriptor<K>> {
    generator: &'descr D,
    network: AddressNetwork,
    keychain: Keychain,
    index: NormalIndex,
    remainder: VecDeque<DerivedAddr>,
    _phantom: PhantomData<K>,
}

impl<K, D: Descriptor<K>> Iterator for AddrIter<'_, K, D> {
    type Item = DerivedAddr;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(derived) = self.remainder.pop_front() {
                return Some(derived);
            }
            self.remainder = self
                .generator
                .derive_address(self.network, self.keychain, self.index)
                .map(|addr| DerivedAddr::new(addr, self.keychain, self.index))
                .collect();
            self.index.checked_inc_assign()?;
        }
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
#[derive(Getters, Debug)]
pub struct WalletDescr<K, D, L2 = Layer2Empty>
where
    D: Descriptor<K>,
    L2: Layer2Descriptor,
{
    #[getter(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    persistence: Option<Persistence<Self>>,

    generator: D,
    #[getter(as_copy)]
    network: Network,
    layer2: L2,
    #[cfg_attr(feature = "serde", serde(skip))]
    _phantom: PhantomData<K>,
}

impl<K, D: Descriptor<K>> WalletDescr<K, D, Layer2Empty> {
    pub fn new_standard(descr: D, network: Network) -> Self {
        WalletDescr {
            persistence: None,
            generator: descr,
            network,
            layer2: none!(),
            _phantom: PhantomData,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> WalletDescr<K, D, L2> {
    pub fn new_layer2(descr: D, layer2: L2, network: Network) -> Self {
        WalletDescr {
            persistence: None,
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
            remainder: VecDeque::new(),
            _phantom: PhantomData,
        }
    }

    pub fn with_descriptor<T, E>(
        &mut self,
        f: impl FnOnce(&mut D) -> Result<T, E>,
    ) -> Result<T, E> {
        let res = f(&mut self.generator)?;
        self.mark_dirty();
        Ok(res)
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> Deref for WalletDescr<K, D, L2> {
    type Target = D;

    fn deref(&self) -> &Self::Target { &self.generator }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> CloneNoPersistence for WalletDescr<K, D, L2> {
    fn clone_no_persistence(&self) -> Self {
        Self {
            persistence: None,
            generator: self.generator.clone(),
            network: self.network,
            layer2: self.layer2.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> Persisting for WalletDescr<K, D, L2> {
    #[inline]
    fn persistence(&self) -> Option<&Persistence<Self>> { self.persistence.as_ref() }
    #[inline]
    fn persistence_mut(&mut self) -> Option<&mut Persistence<Self>> { self.persistence.as_mut() }
    #[inline]
    fn as_mut_persistence(&mut self) -> &mut Option<Persistence<Self>> { &mut self.persistence }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> Drop for WalletDescr<K, D, L2> {
    fn drop(&mut self) {
        if self.is_autosave() && self.is_dirty() {
            if let Err(e) = self.store() {
                #[cfg(feature = "log")]
                log::error!("impossible to automatically-save wallet descriptor on Drop: {e}");
                #[cfg(not(feature = "log"))]
                eprintln!("impossible to automatically-save wallet descriptor on Drop: {e}")
            }
        }
    }
}

#[derive(Debug)]
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
    #[cfg_attr(feature = "serde", serde(skip))]
    persistence: Option<Persistence<Self>>,

    /// This field is used by applications to link data with other wallet components
    #[cfg_attr(feature = "serde", serde(skip))]
    pub id: Option<String>,
    pub name: String,
    pub tx_annotations: BTreeMap<Txid, String>,
    pub txout_annotations: BTreeMap<Outpoint, String>,
    pub txin_annotations: BTreeMap<Outpoint, String>,
    pub addr_annotations: BTreeMap<Address, String>,
    pub last_used: BTreeMap<Keychain, NormalIndex>,
    pub layer2: L2,
}

impl<L2: Layer2Data> CloneNoPersistence for WalletData<L2> {
    fn clone_no_persistence(&self) -> Self {
        Self {
            persistence: None,
            id: self.id.clone(),
            name: self.name.clone(),
            tx_annotations: self.tx_annotations.clone(),
            txout_annotations: self.txout_annotations.clone(),
            txin_annotations: self.txin_annotations.clone(),
            addr_annotations: self.addr_annotations.clone(),
            layer2: self.layer2.clone(),
            last_used: self.last_used.clone(),
        }
    }
}

impl<L2: Layer2Data> Persisting for WalletData<L2> {
    #[inline]
    fn persistence(&self) -> Option<&Persistence<Self>> { self.persistence.as_ref() }
    #[inline]
    fn persistence_mut(&mut self) -> Option<&mut Persistence<Self>> { self.persistence.as_mut() }
    #[inline]
    fn as_mut_persistence(&mut self) -> &mut Option<Persistence<Self>> { &mut self.persistence }
}

impl WalletData<Layer2Empty> {
    pub fn new_layer1() -> Self {
        WalletData {
            persistence: None,
            id: None,
            name: none!(),
            tx_annotations: empty!(),
            txout_annotations: empty!(),
            txin_annotations: empty!(),
            addr_annotations: empty!(),
            layer2: none!(),
            last_used: empty!(),
        }
    }
}

impl<L2: Layer2Data> WalletData<L2> {
    pub fn new_layer2() -> Self
    where L2: Default {
        WalletData {
            persistence: None,
            id: None,
            name: none!(),
            tx_annotations: empty!(),
            txout_annotations: empty!(),
            txin_annotations: empty!(),
            addr_annotations: empty!(),
            layer2: none!(),
            last_used: empty!(),
        }
    }
}

impl<L2: Layer2Data> Drop for WalletData<L2> {
    fn drop(&mut self) {
        if self.is_autosave() && self.is_dirty() {
            if let Err(e) = self.store() {
                #[cfg(feature = "log")]
                log::error!("impossible to automatically-save wallet data on Drop: {e}");
                #[cfg(not(feature = "log"))]
                eprintln!("impossible to automatically-save wallet data on Drop: {e}")
            }
        }
    }
}

pub trait WalletCacheProvider<L2C: Layer2Cache>: Persisting {
    fn layer2(&self) -> &L2C;

    fn layer2_mut(&mut self) -> &mut L2C;

    fn addr_by_address(&self, addr: &Address) -> Option<(Keychain, WalletAddr)>;

    fn addrs(&self) -> impl Iterator<Item = (Keychain, WalletAddr)>;

    fn new_tx(&mut self, txid: Txid, tx: WalletTx);

    fn tx(&self, txid: &Txid) -> Option<Cow<'_, WalletTx>>;

    fn txs(&self) -> impl Iterator<Item = (Txid, Cow<'_, WalletTx>)> + '_;

    fn new_utxo(&mut self, outpoint: Outpoint);

    fn utxo(&self, outpoint: &Outpoint) -> Option<Cow<'_, Outpoint>>;

    fn utxos(&self) -> impl Iterator<Item = Outpoint> + '_;

    fn coins(&self) -> impl Iterator<Item = CoinRow<L2C::Coin>> + '_ {
        self.utxos().map(|outpoint| {
            let tx = self.tx(&outpoint.txid).expect("cache data inconsistency");
            let out = tx.outputs.get(outpoint.vout_usize()).expect("cache data inconsistency");
            CoinRow {
                height: tx.status.map(|info| info.height),
                outpoint,
                address: out.derived_addr().expect("cache data inconsistency"),
                amount: out.value,
                layer2: none!(), // TODO: Add support to WalletTx
            }
        })
    }

    fn history(&self) -> impl Iterator<Item = TxRow<L2C::Tx>> + '_ {
        self.txs().map(|(_, tx)| {
            let (credit, debit) = tx.credited_debited();
            let mut row = TxRow {
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
                layer2: none!(), // TODO: Add support to WalletTx
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

    fn outpoint_by(&self, outpoint: Outpoint) -> Result<(WalletUtxo, ScriptPubkey), NonWalletItem> {
        let tx = self.tx(&outpoint.txid).ok_or(NonWalletItem::NonWalletTx(outpoint.txid))?;
        let debit = tx
            .outputs
            .get(outpoint.vout.into_usize())
            .ok_or(NonWalletItem::NoOutput(outpoint.txid, outpoint.vout))?;
        let terminal = debit.derived_addr().ok_or(NonWalletItem::NonWalletUtxo(outpoint))?.terminal;
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

    fn is_unspent(&self, outpoint: Outpoint) -> bool { self.utxo(&outpoint).is_some() }

    fn has_outpoint(&self, outpoint: Outpoint) -> bool {
        let Some(tx) = self.tx(&outpoint.txid) else {
            return false;
        };
        let Some(out) = tx.outputs.get(outpoint.vout.to_usize()) else {
            return false;
        };
        matches!(out.beneficiary, Party::Wallet(_))
    }

    fn register_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta) {
        let unsigned_tx = psbt.to_unsigned_tx();
        let txid = unsigned_tx.txid();
        let wallet_tx = WalletTx {
            txid,
            status: TxStatus::Mempool,
            inputs: psbt
                .inputs()
                .map(|input| {
                    let addr = Address::with(&input.prev_txout().script_pubkey, meta.network).ok();
                    TxCredit {
                        outpoint: input.previous_outpoint,
                        payer: match (self.utxo(&input.previous_outpoint), addr) {
                            (Some(_), Some(addr)) => {
                                let (keychain, index) = self
                                    .addr_by_address(&addr)
                                    .map(|(keychain, a)| (keychain, a.terminal.index))
                                    .expect("address cache inconsistency");
                                Party::Wallet(DerivedAddr::new(addr, keychain, index))
                            }
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
                .collect(),
            outputs: psbt
                .outputs()
                .map(|output| {
                    let vout = Vout::from_u32(output.index() as u32);
                    let addr = Address::with(&output.script, meta.network).ok();
                    TxDebit {
                        outpoint: Outpoint::new(txid, vout),
                        beneficiary: match (meta.change, addr) {
                            (Some(change), Some(addr)) if change.vout == vout => {
                                Party::Wallet(DerivedAddr::new(
                                    addr,
                                    change.terminal.keychain,
                                    change.terminal.index,
                                ))
                            }
                            (_, Some(addr)) => Party::Counterparty(addr),
                            (_, _) => Party::Unknown(output.script.clone()),
                        },
                        value: output.value(),
                        spent: None,
                    }
                })
                .collect(),
            fee: meta.fee,
            size: meta.size,
            weight: meta.weight,
            version: unsigned_tx.version,
            locktime: unsigned_tx.lock_time,
        };
        self.new_tx(txid, wallet_tx);
        if let Some(change) = meta.change {
            self.new_utxo(Outpoint::new(txid, change.vout));
        }
    }
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
#[derive(Debug)]
pub struct WalletCache<L2: Layer2Cache> {
    #[cfg_attr(feature = "serde", serde(skip))]
    persistence: Option<Persistence<Self>>,

    /// This field is used by applications to link data with other wallet components
    #[cfg_attr(feature = "serde", serde(skip))]
    pub id: Option<String>,
    pub last_block: MiningInfo,
    pub last_change: NormalIndex,
    pub headers: BTreeSet<BlockInfo>,
    pub tx: BTreeMap<Txid, WalletTx>,
    pub utxo: BTreeSet<Outpoint>,
    pub addr: BTreeMap<Keychain, BTreeSet<WalletAddr>>,
    pub layer2: L2,
}

impl<L2C> WalletCacheProvider<L2C> for WalletCache<L2C>
where L2C: Layer2Cache
{
    fn layer2(&self) -> &L2C { &self.layer2 }

    fn layer2_mut(&mut self) -> &mut L2C { &mut self.layer2 }

    fn addr_by_address(&self, addr: &Address) -> Option<(Keychain, WalletAddr)> {
        self.addrs().find(|(_, a)| a.addr == *addr)
    }

    fn addrs(&self) -> impl Iterator<Item = (Keychain, WalletAddr)> {
        self.addr.iter().flat_map(|(keychain, addrs)| addrs.iter().map(|addr| (*keychain, *addr)))
    }

    fn new_tx(&mut self, txid: Txid, tx: WalletTx) { self.tx.insert(txid, tx); }

    fn tx(&self, txid: &Txid) -> Option<Cow<'_, WalletTx>> { self.tx.get(txid).map(Cow::Borrowed) }

    fn txs(&self) -> impl Iterator<Item = (Txid, Cow<'_, WalletTx>)> + '_ {
        self.tx.iter().map(|(txid, wallet_tx)| (*txid, Cow::Borrowed(wallet_tx)))
    }

    fn new_utxo(&mut self, outpoint: Outpoint) { self.utxo.insert(outpoint); }

    fn utxo(&self, outpoint: &Outpoint) -> Option<Cow<'_, Outpoint>> {
        self.utxo.get(outpoint).map(Cow::Borrowed)
    }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> + '_ { self.utxo.iter().copied() }
}

impl<L2C: Layer2Cache> WalletCache<L2C> {
    pub(crate) fn new_nonsync() -> Self {
        WalletCache {
            persistence: None,
            id: None,
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
        let res = indexer.update::<K, D, L2>(descriptor, self);
        self.mark_dirty();
        res
    }

    pub fn register_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta) {
        let unsigned_tx = psbt.to_unsigned_tx();
        let txid = unsigned_tx.txid();
        let wallet_tx = WalletTx {
            txid,
            status: TxStatus::Mempool,
            inputs: psbt
                .inputs()
                .map(|input| {
                    let addr = Address::with(&input.prev_txout().script_pubkey, meta.network).ok();
                    TxCredit {
                        outpoint: input.previous_outpoint,
                        payer: match (self.utxo.get(&input.previous_outpoint), addr) {
                            (Some(_), Some(addr)) => {
                                let (keychain, index) = self
                                    .addr
                                    .iter()
                                    .flat_map(|(keychain, addrs)| {
                                        addrs.iter().map(|a| (*keychain, a))
                                    })
                                    .find(|(_, a)| a.addr == addr)
                                    .map(|(keychain, a)| (keychain, a.terminal.index))
                                    .expect("address cache inconsistency");
                                Party::Wallet(DerivedAddr::new(addr, keychain, index))
                            }
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
                .collect(),
            outputs: psbt
                .outputs()
                .map(|output| {
                    let vout = Vout::from_u32(output.index() as u32);
                    let addr = Address::with(&output.script, meta.network).ok();
                    TxDebit {
                        outpoint: Outpoint::new(txid, vout),
                        beneficiary: match (meta.change, addr) {
                            (Some(change), Some(addr)) if change.vout == vout => {
                                Party::Wallet(DerivedAddr::new(
                                    addr,
                                    change.terminal.keychain,
                                    change.terminal.index,
                                ))
                            }
                            (_, Some(addr)) => Party::Counterparty(addr),
                            (_, _) => Party::Unknown(output.script.clone()),
                        },
                        value: output.value(),
                        spent: None,
                    }
                })
                .collect(),
            fee: meta.fee,
            size: meta.size,
            weight: meta.weight,
            version: unsigned_tx.version,
            locktime: unsigned_tx.lock_time,
        };
        self.tx.insert(txid, wallet_tx);
        if let Some(change) = meta.change {
            self.utxo.insert(Outpoint::new(txid, change.vout));
        }
    }

    pub fn addresses_on(&self, keychain: Keychain) -> &BTreeSet<WalletAddr> {
        self.addr.get(&keychain).unwrap_or_else(|| {
            panic!("keychain #{keychain} is not supported by the wallet descriptor")
        })
    }

    pub fn has_outpoint(&self, outpoint: Outpoint) -> bool {
        let Some(tx) = self.tx.get(&outpoint.txid) else {
            return false;
        };
        let Some(out) = tx.outputs.get(outpoint.vout.to_usize()) else {
            return false;
        };
        matches!(out.beneficiary, Party::Wallet(_))
    }

    #[inline]
    pub fn is_unspent(&self, outpoint: Outpoint) -> bool { self.utxo.contains(&outpoint) }

    pub fn outpoint_by(
        &self,
        outpoint: Outpoint,
    ) -> Result<(WalletUtxo, ScriptPubkey), NonWalletItem> {
        let tx = self.tx.get(&outpoint.txid).ok_or(NonWalletItem::NonWalletTx(outpoint.txid))?;
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
        self.tx.iter().flat_map(|(txid, tx)| {
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
}

impl<L2: Layer2Cache> CloneNoPersistence for WalletCache<L2> {
    fn clone_no_persistence(&self) -> Self {
        Self {
            persistence: None,
            id: self.id.clone(),
            last_block: self.last_block,
            last_change: self.last_change,
            headers: self.headers.clone(),
            tx: self.tx.clone(),
            utxo: self.utxo.clone(),
            addr: self.addr.clone(),
            layer2: self.layer2.clone(),
        }
    }
}

impl<L2: Layer2Cache> Persisting for WalletCache<L2> {
    #[inline]
    fn persistence(&self) -> Option<&Persistence<Self>> { self.persistence.as_ref() }
    #[inline]
    fn persistence_mut(&mut self) -> Option<&mut Persistence<Self>> { self.persistence.as_mut() }
    #[inline]
    fn as_mut_persistence(&mut self) -> &mut Option<Persistence<Self>> { &mut self.persistence }
}

impl<L2: Layer2Cache> Drop for WalletCache<L2> {
    fn drop(&mut self) {
        if self.is_autosave() && self.is_dirty() {
            if let Err(e) = self.store() {
                #[cfg(feature = "log")]
                log::error!("impossible to automatically-save wallet cache on Drop: {e}");
                #[cfg(not(feature = "log"))]
                eprintln!("impossible to automatically-save wallet cache on Drop: {e}")
            }
        }
    }
}

#[derive(Debug)]
pub struct Wallet<K, D: Descriptor<K>, Cache: WalletCacheProvider<L2::Cache>, L2: Layer2 = NoLayer2>
{
    descr: WalletDescr<K, D, L2::Descr>,
    data: WalletData<L2::Data>,
    cache: Cache,
    layer2: L2,
}

impl<K, D: Descriptor<K>, Cache: WalletCacheProvider<L2::Cache>, L2: Layer2> Deref
    for Wallet<K, D, Cache, L2>
{
    type Target = WalletDescr<K, D, L2::Descr>;

    fn deref(&self) -> &Self::Target { &self.descr }
}

impl<
        K,
        D: Descriptor<K>,
        Cache: CloneNoPersistence + WalletCacheProvider<L2::Cache>,
        L2: Layer2,
    > CloneNoPersistence for Wallet<K, D, Cache, L2>
{
    fn clone_no_persistence(&self) -> Self {
        Self {
            descr: self.descr.clone_no_persistence(),
            data: self.data.clone_no_persistence(),
            cache: self.cache.clone_no_persistence(),
            layer2: self.layer2.clone_no_persistence(),
        }
    }
}

impl<K, D: Descriptor<K>, Cache: Persisting + WalletCacheProvider<L2::Cache>, L2: Layer2>
    PsbtConstructor for Wallet<K, D, Cache, L2>
{
    type Key = K;
    type Descr = D;

    fn descriptor(&self) -> &D { &self.descr.generator }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
        self.cache.outpoint_by(outpoint).ok().map(|(utxo, spk)| (utxo.into_utxo(), spk))
    }

    fn network(&self) -> Network { self.descr.network }

    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        let keychain = keychain.into();
        let mut idx = self.last_published_derivation_index(keychain);
        let last_index = self.data.last_used.entry(keychain).or_default();
        idx = cmp::max(*last_index, idx);
        if shift {
            *last_index = idx.saturating_add(1u32);
            self.data.mark_dirty();
        }
        idx
    }

    fn after_construct_psbt(&mut self, psbt: &Psbt, meta: &PsbtMeta) {
        debug_assert_eq!(AddressNetwork::from(self.network), meta.network);
        self.cache.register_psbt(psbt, meta);
    }
}

impl<K, D: Descriptor<K>> Wallet<K, D, WalletCache<Layer2Empty>> {
    pub fn new_layer1(descr: D, network: Network) -> Self {
        Wallet {
            cache: WalletCache::new_nonsync(),
            data: WalletData::new_layer1(),
            descr: WalletDescr::new_standard(descr, network),
            layer2: none!(),
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, WalletCache<L2::Cache>, L2> {
    pub fn new_layer2(descr: D, l2_descr: L2::Descr, layer2: L2, network: Network) -> Self {
        Wallet {
            cache: WalletCache::new_nonsync(),
            data: WalletData::new_layer2(),
            descr: WalletDescr::new_layer2(descr, l2_descr, network),
            layer2,
        }
    }

    pub fn txos(&self) -> impl Iterator<Item = WalletUtxo> + '_ { self.cache.txos() }
}

impl<K, D: Descriptor<K>, Cache: WalletCacheProvider<L2::Cache>, L2: Layer2>
    Wallet<K, D, Cache, L2>
{
    pub fn set_name(&mut self, name: String) {
        self.data.name = name;
        self.data.mark_dirty();
    }

    pub fn with_descriptor<T, E>(
        &mut self,
        f: impl FnOnce(&mut D) -> Result<T, E>,
    ) -> Result<T, E> {
        self.descr.with_descriptor(f)
    }

    pub fn data_l2(&self) -> &L2::Data { &self.data.layer2 }
    pub fn cache_l2(&self) -> &L2::Cache { &self.cache.layer2() }

    pub fn with_data<T, E>(
        &mut self,
        f: impl FnOnce(&mut WalletData<L2::Data>) -> Result<T, E>,
    ) -> Result<T, E> {
        let res = f(&mut self.data)?;
        self.data.mark_dirty();
        Ok(res)
    }

    pub fn with_data_l2<T, E>(
        &mut self,
        f: impl FnOnce(&mut L2::Data) -> Result<T, E>,
    ) -> Result<T, E> {
        let res = f(&mut self.data.layer2)?;
        self.data.mark_dirty();
        Ok(res)
    }

    pub fn with_cache_l2<T, E>(
        &mut self,
        f: impl FnOnce(&mut L2::Cache) -> Result<T, E>,
    ) -> Result<T, E> {
        let res = f(&mut self.cache.layer2_mut())?;
        self.cache.mark_dirty();
        Ok(res)
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
    pub fn transactions(&self) -> impl Iterator<Item = (Txid, Cow<'_, WalletTx>)> + '_ {
        self.cache.txs()
    }

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
        self.cache.addrs().map(|(_, addr)| addr)
    }

    #[inline]
    pub fn history(&self) -> impl Iterator<Item = TxRow<<L2::Cache as Layer2Cache>::Tx>> + '_ {
        self.cache.history()
    }

    pub fn has_outpoint(&self, outpoint: Outpoint) -> bool { self.cache.has_outpoint(outpoint) }
    pub fn is_unspent(&self, outpoint: Outpoint) -> bool { self.cache.is_unspent(outpoint) }

    pub fn outpoint_by(
        &self,
        outpoint: Outpoint,
    ) -> Result<(WalletUtxo, ScriptPubkey), NonWalletItem> {
        self.cache.outpoint_by(outpoint)
    }

    pub fn utxos(&self) -> impl Iterator<Item = WalletUtxo> + '_ {
        self.cache.utxos().flat_map(|outpoint| {
            let tx = self.cache.tx(&outpoint.txid).expect("cache data inconsistency");
            let debit = tx.outputs.get(outpoint.vout_usize()).expect("cache data inconsistency");
            let terminal =
                debit.derived_addr().expect("UTXO doesn't belong to the wallet").terminal;
            if debit.spent.is_some() {
                None
            } else {
                Some(WalletUtxo {
                    outpoint,
                    value: debit.value,
                    terminal,
                    status: tx.status,
                })
            }
        })
    }

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

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, WalletCache<L2::Cache>, L2> {
    pub fn set_id(&mut self, id: &impl ToString) {
        self.data.id = Some(id.to_string());
        self.cache.id = Some(id.to_string());
    }
}

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, WalletCache<L2::Cache>, L2> {
    #[must_use]
    pub fn update<I: Indexer>(&mut self, indexer: &I) -> MayError<(), Vec<I::Error>> {
        indexer.update::<K, D, L2>(&self.descr, &mut self.cache).map(drop)
    }
}

impl<K, D: Descriptor<K>, Cache: WalletCacheProvider<L2::Cache> + Persisting, L2: Layer2>
    Wallet<K, D, Cache, L2>
{
    pub fn load<P>(
        provider: P,
        autosave: bool,
    ) -> Result<Wallet<K, D, Cache, L2>, PersistenceError>
    where
        P: Clone
            + PersistenceProvider<WalletDescr<K, D, L2::Descr>>
            + PersistenceProvider<WalletData<L2::Data>>
            + PersistenceProvider<Cache>
            + PersistenceProvider<L2>
            + 'static,
    {
        let descr = WalletDescr::<K, D, L2::Descr>::load(provider.clone(), autosave)?;
        let data = WalletData::<L2::Data>::load(provider.clone(), autosave)?;
        let cache = Cache::load(provider.clone(), autosave)?;
        let layer2 = L2::load(provider, autosave)?;

        Ok(Wallet {
            descr,
            data,
            cache,
            layer2,
        })
    }

    pub fn make_persistent<P>(
        &mut self,
        provider: P,
        autosave: bool,
    ) -> Result<bool, PersistenceError>
    where
        P: Clone
            + PersistenceProvider<WalletDescr<K, D, L2::Descr>>
            + PersistenceProvider<WalletData<L2::Data>>
            + PersistenceProvider<Cache>
            + PersistenceProvider<L2>
            + 'static,
    {
        let a = self.descr.make_persistent(provider.clone(), autosave)?;
        let b = self.data.make_persistent(provider.clone(), autosave)?;
        let c = self.cache.make_persistent(provider.clone(), autosave)?;
        let d = self.layer2.make_persistent(provider, autosave)?;
        Ok(a && b && c && d)
    }

    pub fn store(&mut self) -> Result<(), PersistenceError> {
        // TODO: Revert on failure

        self.descr.store()?;
        self.data.store()?;
        self.cache.store()?;
        self.layer2.store()?;

        Ok(())
    }
}
