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

use bp::{Address, DeriveSpk, Idx, LockTime, Outpoint, Sats};
use psbt::{Psbt, PsbtError};

use crate::{BlockHeight, Layer2, Wallet};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ConstructionError {
    Psbt(PsbtError),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Invoice {
    pub beneficiary: Address,
    pub amount: Sats,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TxParams {
    pub fee_rate: f64,
    pub lock_time: Option<LockTime>,
    pub rbf: bool,
}

impl<D: DeriveSpk, L2: Layer2> Wallet<D, L2> {
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
            // Get terminal and txout matching outpoint
            let mut input = psbt::Input::new(txout, coin, &self.descr, terminal);
            // TODO: Set nSeq
            psbt.push_input(input);
        }

        // 2. Add beneficiary
        // TODO: Check address network
        psbt.push_output(invoice.beneficiary.script_pubkey(), invoice.amount);

        // 3. Add change
        // TODO: Find out change amount
        psbt.push_change(&self.descr, self.cache.last_change, change_amount);
        self.cache.last_change.wrapping_inc_assign();

        psbt.complete_construction();

        psbt.Ok(psbt)
    }
}
