// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Treasury pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_system::RawOrigin;
use frame_benchmarking::{benchmarks, account, whitelisted_caller};
use frame_support::traits::OnInitialize;

use crate::Module as Bounties;

const SEED: u32 = 0;

// // Create bounties that are approved for use in `on_initialize`.
// fn create_approved_bounties<T: Trait<I>, I: Instance>(n: u32) -> Result<(), &'static str> {
// 	for i in 0 .. n {
// 		let (caller, _curator, _fee, value, reason) = setup_bounty::<T>(i, MAX_BYTES);
// 		Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
// 		let bounty_id = BountyCount::get() - 1;
// 		Bounties::<T>::approve_bounty(RawOrigin::Root.into(), bounty_id)?;
// 	}
// 	ensure!(BountyApprovals::get().len() == n as usize, "Not all bounty approved");
// 	Ok(())
// }

// Create the pre-requisite information needed to create a treasury `propose_bounty`.
fn setup_bounty<T: Trait>(u: u32, d: u32) -> (
	T::AccountId,
	T::AccountId,
	BalanceOf<T>,
	BalanceOf<T>,
	Vec<u8>,
) {
	let caller = account("caller", u, SEED);
	let value: BalanceOf<T> = T::BountyValueMinimum::get().saturating_mul(100u32.into());
	let fee = value / 2u32.into();
	let deposit = T::BountyDepositBase::get() + T::DataDepositPerByte::get() * MAX_BYTES.into();
	let _ = T::Currency::make_free_balance_be(&caller, deposit);
	let curator = account("curator", u, SEED);
	let _ = T::Currency::make_free_balance_be(&curator, fee / 2u32.into());
	let reason = vec![0; d as usize];
	(caller, curator, fee, value, reason)
}

fn create_bounty<T: Trait>() -> Result<(
	<T::Lookup as StaticLookup>::Source,
	BountyIndex,
), &'static str> {
	let (caller, curator, fee, value, reason) = setup_bounty::<T>(0, MAX_BYTES);
	let curator_lookup = T::Lookup::unlookup(curator.clone());
	Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
	let bounty_id = BountyCount::get() - 1;
	Bounties::<T>::approve_bounty(RawOrigin::Root.into(), bounty_id)?;
	// Bounties::<T>::on_initialize(T::BlockNumber::zero());
	Bounties::<T>::propose_curator(RawOrigin::Root.into(), bounty_id, curator_lookup.clone(), fee)?;
	Bounties::<T>::accept_curator(RawOrigin::Signed(curator).into(), bounty_id)?;
	Ok((curator_lookup, bounty_id))
}

fn setup_pod_account<T: Trait>() {
	let pot_account = Bounties::<T>::account_id();
	let value = T::Currency::minimum_balance().saturating_mul(1_000_000_000u32.into());
	let _ = T::Currency::make_free_balance_be(&pot_account, value);
}

const MAX_BYTES: u32 = 16384;

benchmarks! {
	_ { }

	propose_bounty {
		let d in 0 .. MAX_BYTES;

		let (caller, curator, fee, value, description) = setup_bounty::<T>(0, d);
	}: _(RawOrigin::Signed(caller), value, description)

	approve_bounty {
		let (caller, curator, fee, value, reason) = setup_bounty::<T>(0, MAX_BYTES);
		Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
		let bounty_id = BountyCount::get() - 1;
	}: _(RawOrigin::Root, bounty_id)

	propose_curator {
		setup_pod_account::<T>();
		let (caller, curator, fee, value, reason) = setup_bounty::<T>(0, MAX_BYTES);
		let curator_lookup = T::Lookup::unlookup(curator.clone());
		Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
		let bounty_id = BountyCount::get() - 1;
		Bounties::<T>::approve_bounty(RawOrigin::Root.into(), bounty_id)?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
	}: _(RawOrigin::Root, bounty_id, curator_lookup, fee)

	// Worst case when curator is inactive and any sender unassigns the curator.
	unassign_curator {
		setup_pod_account::<T>();
		let (curator_lookup, bounty_id) = create_bounty::<T>()?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
		let bounty_id = BountyCount::get() - 1;
		frame_system::Module::<T>::set_block_number(T::BountyUpdatePeriod::get() + 1u32.into());
		let caller = whitelisted_caller();
	}: _(RawOrigin::Signed(caller), bounty_id)

	accept_curator {
		setup_pod_account::<T>();
		let (caller, curator, fee, value, reason) = setup_bounty::<T>(0, MAX_BYTES);
		let curator_lookup = T::Lookup::unlookup(curator.clone());
		Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
		let bounty_id = BountyCount::get() - 1;
		Bounties::<T>::approve_bounty(RawOrigin::Root.into(), bounty_id)?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
		Bounties::<T>::propose_curator(RawOrigin::Root.into(), bounty_id, curator_lookup, fee)?;
	}: _(RawOrigin::Signed(curator), bounty_id)

	award_bounty {
		setup_pod_account::<T>();
		let (curator_lookup, bounty_id) = create_bounty::<T>()?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::get() - 1;
		let curator = T::Lookup::lookup(curator_lookup)?;
		let beneficiary = T::Lookup::unlookup(account("beneficiary", 0, SEED));
	}: _(RawOrigin::Signed(curator), bounty_id, beneficiary)

	claim_bounty {
		setup_pod_account::<T>();
		let (curator_lookup, bounty_id) = create_bounty::<T>()?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::get() - 1;
		let curator = T::Lookup::lookup(curator_lookup)?;

		let beneficiary = T::Lookup::unlookup(account("beneficiary", 0, SEED));
		Bounties::<T>::award_bounty(RawOrigin::Signed(curator.clone()).into(), bounty_id, beneficiary)?;

		frame_system::Module::<T>::set_block_number(T::BountyDepositPayoutDelay::get());

	}: _(RawOrigin::Signed(curator), bounty_id)

	close_bounty_proposed {
		setup_pod_account::<T>();
		let (caller, curator, fee, value, reason) = setup_bounty::<T>(0, 0);
		Bounties::<T>::propose_bounty(RawOrigin::Signed(caller).into(), value, reason)?;
		let bounty_id = BountyCount::get() - 1;
	}: close_bounty(RawOrigin::Root, bounty_id)

	close_bounty_active {
		setup_pod_account::<T>();
		let (curator_lookup, bounty_id) = create_bounty::<T>()?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());
		let bounty_id = BountyCount::get() - 1;
	}: close_bounty(RawOrigin::Root, bounty_id)

	extend_bounty_expiry {
		setup_pod_account::<T>();
		let (curator_lookup, bounty_id) = create_bounty::<T>()?;
		Bounties::<T>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::get() - 1;
		let curator = T::Lookup::lookup(curator_lookup)?;
	}: _(RawOrigin::Signed(curator), bounty_id, Vec::new())

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_propose_bounty::<Test>());
			assert_ok!(test_benchmark_approve_bounty::<Test>());
			assert_ok!(test_benchmark_propose_curator::<Test>());
			assert_ok!(test_benchmark_unassign_curator::<Test>());
			assert_ok!(test_benchmark_accept_curator::<Test>());
			assert_ok!(test_benchmark_award_bounty::<Test>());
			assert_ok!(test_benchmark_claim_bounty::<Test>());
			assert_ok!(test_benchmark_close_bounty_proposed::<Test>());
			assert_ok!(test_benchmark_close_bounty_active::<Test>());
			assert_ok!(test_benchmark_extend_bounty_expiry::<Test>());
		});
	}
}
