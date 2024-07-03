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

// TODO: Move to amplify library

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct MayError<T, E> {
    pub ok: T,
    pub err: Option<E>,
}

impl<T, E> MayError<T, E> {
    pub fn ok(result: T) -> Self {
        MayError {
            ok: result,
            err: None,
        }
    }

    pub fn err(ok: T, err: E) -> Self { MayError { ok, err: Some(err) } }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> MayError<U, E> {
        let ok = f(self.ok);
        MayError { ok, err: self.err }
    }

    pub fn split(self) -> (T, Option<E>) { (self.ok, self.err) }

    pub fn into_ok(self) -> T { self.ok }

    pub fn into_err(self) -> Option<E> { self.err }

    pub fn unwrap_err(self) -> E { self.err.unwrap() }

    pub fn into_result(self) -> Result<T, E> {
        match self.err {
            Some(err) => Err(err),
            None => Ok(self.ok),
        }
    }
}
