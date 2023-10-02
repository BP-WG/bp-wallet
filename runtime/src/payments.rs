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

use bp::{Address, Descriptor, Idx, LockTime, Outpoint, Sats, SeqNo};
use psbt::{Psbt, PsbtError};

use crate::{BlockHeight, Layer2, Wallet};

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ConstructionError {
    Psbt(PsbtError),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Amount {
    Fixed(Sats),
    Max,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Invoice {
    pub beneficiary: Address,
    pub amount: Amount,
}

#[derive(Copy, Clone, PartialEq)]
pub struct TxParams {
    pub fee_rate: f64,
    pub lock_time: Option<LockTime>,
    pub rbf: bool,
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
            let utxo = self.utxo(coin);
            let seq_no = match params.rbf {
                true => SeqNo::rbf(),
                false => SeqNo::default(),
            };
            psbt.construct_input_expect(utxo.prevout(), &self.descr.generator, utxo.terminal(), seq_no);
        }

        // 2. Add outputs
        // TODO: Check address network

        // 2. Add outputs and change
        let input_value = psbt.input_sum();
        if let Amount::Fixed(value) = invoice.amount {
            psbt.construct_output_expect(invoice.beneficiary.script_pubkey(), value);

            let mut change = psbt.construct_change_expect(&self.descr, self.cache.last_change, Sats::ZERO);
            let output_value = psbt.output_sum();
            let fee_value = psbt.vbytes() * params.fee_rate;
            change.amount = input_value - output_value - fee_value;
        } else {
            let fee_value = psbt.vbytes() * params.fee_rate;
            psbt.construct_output_expect(invoice.beneficiary.script_pubkey(), input_value - fee_value);
        }

        self.cache.last_change.wrapping_inc_assign();

        psbt.complete_construction();

        Ok(psbt)
    }
}
