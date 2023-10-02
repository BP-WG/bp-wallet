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

use bp::{Address, AddressParseError, Descriptor, Idx, LockTime, Outpoint, Sats, SeqNo};
use psbt::{Psbt, PsbtError};

use crate::{Layer2, Wallet};

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ConstructionError {
    Psbt(PsbtError),
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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display)]
pub enum Amount {
    #[display(inner)]
    Fixed(Sats),
    #[display("MAX")]
    Max,
}

impl FromStr for Amount {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "MAX" {
            return Ok(Amount::Max);
        }
        let (int, fract) = s.split_once('.').unwrap_or((s, ""));
        // TODO: check for sats overflow
        Ok(Amount::Fixed(
            Sats::from_btc(u32::from_str(int)?) + Sats::from_sats(u32::from_str(fract)?),
        ))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display)]
#[display("{amount}@{beneficiary}", alt = "bitcoin:{beneficiary}?amount={amount}")]
pub struct Invoice {
    pub beneficiary: Address,
    pub amount: Amount,
}

impl FromStr for Invoice {
    type Err = InvoiceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (amount, beneficiary) = s.split_once('@').ok_or(InvoiceParseError::InvalidFormat)?;
        Ok(Invoice {
            beneficiary: Address::from_str(beneficiary)?,
            amount: Amount::from_str(amount)?,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct TxParams {
    pub fee: Sats,
    pub lock_time: Option<LockTime>,
    pub seq_no: SeqNo,
}

impl<K, D: Descriptor<K>, L2: Layer2> Wallet<K, D, L2> {
    pub fn construct_psbt(
        &mut self,
        coins: &[Outpoint],
        invoice: Invoice,
        params: TxParams,
    ) -> Result<Psbt, ConstructionError> {
        let mut psbt = Psbt::create();

        // Set locktime
        psbt.fallback_locktime = params.lock_time;

        // TODO: Add xpubs

        // 1. Add inputs
        for coin in coins {
            let utxo = self.utxo(*coin).expect("wallet data inconsistency");
            psbt.construct_input_expect(
                utxo.to_prevout(),
                &self.descr.generator,
                utxo.terminal,
                params.seq_no,
            );
        }

        // 2. Add outputs
        // TODO: Check address network

        // 2. Add outputs and change
        let input_value = psbt.input_sum();
        if let Amount::Fixed(value) = invoice.amount {
            psbt.construct_output_expect(invoice.beneficiary.script_pubkey(), value);

            let output_value = psbt.output_sum();
            let change = psbt.construct_change_expect(
                &self.descr.generator,
                self.cache.last_change,
                Sats::ZERO,
            );
            change.amount = input_value - output_value - params.fee;
        } else {
            psbt.construct_output_expect(
                invoice.beneficiary.script_pubkey(),
                input_value - params.fee,
            );
        }

        self.cache.last_change.wrapping_inc_assign();

        psbt.complete_construction();

        Ok(psbt)
    }
}
