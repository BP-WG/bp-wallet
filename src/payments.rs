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

use std::num::ParseIntError;
use std::str::FromStr;

use bpstd::{
    Address, AddressParseError, Keychain, LockTime, Outpoint, Sats, ScriptPubkey, SeqNo, Terminal,
    Vout,
};
use descriptors::Descriptor;
use psbt::{Psbt, PsbtError, PsbtVer};

use crate::{Layer2, Wallet};

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ConstructionError {
    #[display(inner)]
    Psbt(PsbtError),

    /// impossible to construct transaction having no inputs.
    NoInputs,

    /// the total payment amount ({0} sats) exceeds number of sats in existence.
    Overflow(Sats),

    /// attempt to spend more than present in transaction inputs. Total transaction inputs are
    /// {input_value} sats, but output is {output_value} sats.
    OutputExceedsInputs {
        input_value: Sats,
        output_value: Sats,
    },

    /// not enough funds to pay fee of {fee} sats; all inputs contain {input_value} sats and
    /// outputs spends {output_value} sats out of them.
    NoFundsForFee {
        input_value: Sats,
        output_value: Sats,
        fee: Sats,
    },
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum InvoiceParseError {
    #[display("invalid format of the invoice")]
    InvalidFormat,

    #[from]
    Int(ParseIntError),

    #[from]
    Address(AddressParseError),
}

/*
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Display)]
#[display("{int}.{fract}")]
pub struct Btc {
    int: BtcInt,
    fract: BtcFract,
}
 */

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display, From)]
pub enum Amount {
    #[from]
    #[display(inner)]
    Fixed(Sats),
    #[display("MAX")]
    Max,
}

impl Amount {
    #[inline]
    pub fn sats(&self) -> Option<Sats> {
        match self {
            Amount::Fixed(sats) => Some(*sats),
            Amount::Max => None,
        }
    }

    #[inline]
    pub fn unwrap_or(&self, default: impl Into<Sats>) -> Sats {
        self.sats().unwrap_or(default.into())
    }

    #[inline]
    pub fn is_max(&self) -> bool { *self == Amount::Max }
}

impl FromStr for Amount {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "MAX" {
            return Ok(Amount::Max);
        }
        // let (int, fract) = s.split_once('.').unwrap_or((s, ""));
        // Sats::from_btc(u32::from_str(int)?) + Sats::from_sats(u32::from_str(fract)?),
        // TODO: check for sats overflow
        Sats::from_str(s).map(Amount::Fixed)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display)]
#[display("{amount}@{address}", alt = "bitcoin:{address}?amount={amount}")]
pub struct Beneficiary {
    pub address: Address,
    pub amount: Amount,
}

impl Beneficiary {
    #[inline]
    pub fn new(address: Address, amount: impl Into<Amount>) -> Self {
        Beneficiary {
            address,
            amount: amount.into(),
        }
    }
    #[inline]
    pub fn with_max(address: Address) -> Self {
        Beneficiary {
            address,
            amount: Amount::Max,
        }
    }
    #[inline]
    pub fn is_max(&self) -> bool { self.amount.is_max() }
    #[inline]
    pub fn script_pubkey(&self) -> ScriptPubkey { self.address.script_pubkey() }
}

impl FromStr for Beneficiary {
    type Err = InvoiceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (amount, beneficiary) = s.split_once('@').ok_or(InvoiceParseError::InvalidFormat)?;
        Ok(Beneficiary::new(Address::from_str(beneficiary)?, Amount::from_str(amount)?))
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct TxParams {
    pub fee: Sats,
    pub lock_time: Option<LockTime>,
    pub seq_no: SeqNo,
    pub change_shift: bool,
    pub change_keychain: Keychain,
}

impl TxParams {
    pub fn with(fee: Sats) -> Self {
        TxParams {
            fee,
            lock_time: None,
            seq_no: SeqNo::from_consensus_u32(0),
            change_shift: true,
            change_keychain: Keychain::INNER,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PsbtMeta {
    pub change_vout: Option<Vout>,
    pub change_terminal: Option<Terminal>,
}

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, L2> {
    pub fn construct_psbt<'a, 'b>(
        &mut self,
        coins: impl IntoIterator<Item = Outpoint>,
        beneficiaries: impl IntoIterator<Item = &'b Beneficiary>,
        params: TxParams,
    ) -> Result<(Psbt, PsbtMeta), ConstructionError> {
        let mut psbt = Psbt::create(PsbtVer::V2);

        // Set locktime
        psbt.fallback_locktime = params.lock_time;

        // Add xpubs
        for spec in self.descr.generator.xpubs() {
            psbt.xpubs.insert(*spec.xpub(), spec.origin().clone());
        }

        // 1. Add inputs
        for coin in coins {
            let utxo = self.utxo(coin).expect("wallet data inconsistency");
            psbt.construct_input_expect(
                utxo.to_prevout(),
                &self.descr.generator,
                utxo.terminal,
                params.seq_no,
            );
        }
        if psbt.inputs().count() == 0 {
            return Err(ConstructionError::NoInputs);
        }

        // 2. Add outputs
        let input_value = psbt.input_sum();
        let mut max = Vec::new();
        let mut output_value = Sats::ZERO;
        for beneficiary in beneficiaries {
            let amount = beneficiary.amount.unwrap_or(Sats::ZERO);
            output_value
                .checked_add_assign(amount)
                .ok_or(ConstructionError::Overflow(output_value))?;
            let out = psbt.construct_output_expect(beneficiary.script_pubkey(), amount);
            if beneficiary.amount.is_max() {
                max.push(out.index());
            }
        }
        let mut remaining_value = input_value
            .checked_sub(output_value)
            .ok_or(ConstructionError::OutputExceedsInputs {
                input_value,
                output_value,
            })?
            .checked_sub(params.fee)
            .ok_or(ConstructionError::NoFundsForFee {
                input_value,
                output_value,
                fee: params.fee,
            })?;
        if !max.is_empty() {
            let portion = remaining_value / max.len();
            for out in psbt.outputs_mut() {
                if max.contains(&out.index()) {
                    out.amount = portion;
                }
            }
            remaining_value = Sats::ZERO;
        }

        // 3. Add change - only if exceeded the dust limit
        let (change_vout, change_terminal) = if remaining_value
            > self.descr.generator.class().dust_limit()
        {
            let change_index =
                self.next_derivation_index(params.change_keychain, params.change_shift);
            let change_terminal = Terminal::new(params.change_keychain, change_index);
            let change_vout = psbt
                .construct_change_expect(&self.descr.generator, change_terminal, remaining_value)
                .index();
            (Some(Vout::from_u32(change_vout as u32)), Some(change_terminal))
        } else {
            (None, None)
        };

        Ok((psbt, PsbtMeta {
            change_vout,
            change_terminal,
        }))
    }
}
