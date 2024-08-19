// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Nicola Busanello <nicola.busanello@gmail.com>
//
// Copyright (C) 2024 LNP/BP Standards Association. All rights reserved.
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
use std::num::NonZeroU32;
use std::str::FromStr;

use amplify::hex::{FromHex, ToHex};
use amplify::ByteArray;
use bpstd::{
    Address, BlockHash, ConsensusDecode, LockTime, Outpoint, ScriptPubkey, SeqNo, SigScript, Tx,
    Txid, Weight, Witness,
};
use descriptors::Descriptor;
use electrum::{Client, ElectrumApi, Error, Param};
use serde_crate::Deserialize;

use super::BATCH_SIZE;
use crate::{
    Indexer, Layer2, MayError, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletAddr,
    WalletCache, WalletDescr, WalletTx,
};

impl From<VinExtended> for TxCredit {
    fn from(vine: VinExtended) -> Self {
        let vin = vine.vin;
        let txid = Txid::from_str(&vin.txid).expect("input txid should deserialize");
        TxCredit {
            outpoint: Outpoint::new(txid, vin.vout),
            sequence: SeqNo::from_consensus_u32(vin.sequence),
            coinbase: txid.is_coinbase(),
            script_sig: vine.sig_script,
            witness: vine.witness,
            value: vine.value.into(),
            payer: Party::Unknown(vine.payer),
        }
    }
}

#[derive(Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
struct Pubkey {
    hex: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
struct Vin {
    sequence: u32,
    txid: String,
    vout: u32,
}

#[derive(Debug)]
struct VinExtended {
    vin: Vin,
    sig_script: SigScript,
    witness: Witness,
    value: u64,
    payer: ScriptPubkey,
}

#[derive(Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
struct Vout {
    n: u64,
    script_pub_key: Pubkey,
    value: f64,
}

#[derive(Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
struct TxDetails {
    hex: String,
    locktime: u32,
    size: u32,
    version: i32,
    vin: Vec<Vin>,
    vout: Vec<Vout>,
}

impl Indexer for Client {
    type Error = Error;

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
                let script = derive.addr.script_pubkey();

                eprint!(".");
                let mut txids = Vec::new();
                match self.script_get_history(&script) {
                    Err(err) => {
                        errors.push(err);
                        break;
                    }
                    Ok(hres) if hres.is_empty() => {
                        empty_count += 1;
                        if empty_count >= BATCH_SIZE as usize {
                            break;
                        }
                    }
                    Ok(hres) => {
                        empty_count = 0;

                        // build WalletTx's from script tx history, collecting indexer errors
                        let results: Vec<Result<WalletTx, Self::Error>> = hres
                            .into_iter()
                            .map(|hr| {
                                let txid = Txid::from_hex(&hr.tx_hash.to_hex())
                                    .expect("txid should deserialize");
                                txids.push(txid);
                                // get the tx details (requires electrum verbose support)
                                let tx_details =
                                    self.raw_call("blockchain.transaction.get", vec![
                                        Param::String(hr.tx_hash.to_string()),
                                        Param::Bool(true),
                                    ])?;
                                let tx = serde_json::from_value::<TxDetails>(tx_details.clone())
                                    .expect("tx details should deserialize");
                                // build TxStatus
                                let status = if hr.height < 1 {
                                    TxStatus::Mempool
                                } else {
                                    let blockhash = tx_details
                                        .get("blockhash")
                                        .expect("blockhash should be present")
                                        .as_str()
                                        .expect("blockhash should be a str");
                                    let blocktime = tx_details
                                        .get("blocktime")
                                        .expect("blocktime should be present")
                                        .as_u64()
                                        .expect("blocktime should be a u64");
                                    TxStatus::Mined(MiningInfo {
                                        height: NonZeroU32::try_from(hr.height as u32)
                                            .unwrap_or(NonZeroU32::MIN),
                                        time: blocktime,
                                        block_hash: BlockHash::from_str(blockhash)
                                            .expect("blockhash should deserialize"),
                                    })
                                };
                                // get inputs to build TxCredit's and total amount,
                                // collecting indexer errors
                                let hex_bytes = Vec::<u8>::from_hex(&tx.hex)
                                    .expect("tx hex should convert to u8 vec");
                                let bp_tx = Tx::consensus_deserialize(hex_bytes)
                                    .expect("tx should deserialize");
                                let mut input_tot: u64 = 0;
                                let input_results: Vec<Result<VinExtended, Self::Error>> = tx
                                    .vin
                                    .iter()
                                    .map(|v| {
                                        let input = bp_tx
                                            .inputs
                                            .iter()
                                            .find(|i| {
                                                i.prev_output.txid.to_string() == v.txid
                                                    && i.prev_output.vout.to_u32() == v.vout
                                            })
                                            .expect("input should be present");
                                        let witness = input.witness.clone();
                                        // get value from previous output tx
                                        let prev_txid = Txid::from_byte_array(
                                            input.prev_output.txid.to_byte_array(),
                                        );
                                        let prev_tx = self.transaction_get(&prev_txid)?;
                                        let value = prev_tx.outputs
                                            [input.prev_output.vout.into_usize()]
                                        .value
                                        .0;
                                        input_tot += value;
                                        let payer = prev_tx.outputs
                                            [input.prev_output.vout.into_usize()]
                                        .script_pubkey
                                        .clone();
                                        Ok(VinExtended {
                                            vin: v.clone(),
                                            sig_script: input.sig_script.clone(),
                                            witness,
                                            value,
                                            payer,
                                        })
                                    })
                                    .collect();
                                let (input_oks, input_errs): (Vec<_>, Vec<_>) =
                                    input_results.into_iter().partition(Result::is_ok);
                                input_errs.into_iter().for_each(|e| errors.push(e.unwrap_err()));
                                // get outputs and total amount, build TxDebit's
                                let mut output_tot: u64 = 0;
                                let outputs = tx
                                    .vout
                                    .into_iter()
                                    .map(|vout| {
                                        let value = (vout.value * 100_000_000.0) as u64;
                                        output_tot += value;
                                        let script_pubkey =
                                            ScriptPubkey::from_hex(&vout.script_pub_key.hex)
                                                .expect("script pubkey hex should deserialize");
                                        TxDebit {
                                            outpoint: Outpoint::new(txid, vout.n as u32),
                                            beneficiary: Party::Unknown(script_pubkey),
                                            value: value.into(),
                                            spent: None,
                                        }
                                    })
                                    .collect();
                                // build the WalletTx
                                Ok(WalletTx {
                                    txid,
                                    status,
                                    inputs: input_oks
                                        .into_iter()
                                        .map(Result::unwrap)
                                        .map(TxCredit::from)
                                        .collect(),
                                    outputs,
                                    fee: (input_tot - output_tot).into(),
                                    size: tx.size,
                                    weight: bp_tx.weight_units().to_u32(),
                                    version: tx.version,
                                    locktime: LockTime::from_consensus_u32(tx.locktime),
                                })
                            })
                            .collect();

                        // update cache and errors
                        let (oks, errs): (Vec<_>, Vec<_>) =
                            results.into_iter().partition(Result::is_ok);
                        errs.into_iter().for_each(|e| errors.push(e.unwrap_err()));
                        cache.tx.extend(oks.into_iter().map(|tx| {
                            let tx = tx.unwrap();
                            (tx.txid, tx)
                        }));
                    }
                }

                let wallet_addr = WalletAddr::<i64>::from(derive);
                address_index.insert(script, (wallet_addr, txids));
            }
        }

        // TODO: Update headers & tip

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
                            if tx.status.is_mined() {
                                cache.utxo.remove(&outpoint);
                            }
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

        if errors.is_empty() { MayError::ok(cache) } else { MayError::err(cache, errors) }
    }

    fn update<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        _descr: &WalletDescr<K, D, L2::Descr>,
        _cache: &mut WalletCache<L2::Cache>,
    ) -> MayError<usize, Vec<Self::Error>> {
        todo!()
    }

    fn publish(&self, tx: &Tx) -> Result<(), Self::Error> {
        self.transaction_broadcast(tx)?;
        Ok(())
    }
}
