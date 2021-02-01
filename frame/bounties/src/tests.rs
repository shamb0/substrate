// This file is part of Substrate.

// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
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

//! bounties pallet tests.

#![cfg(test)]

use super::*;
use std::cell::RefCell;

use frame_support::{
	assert_noop, assert_ok, impl_outer_origin, parameter_types, weights::Weight,
	impl_outer_event, traits::{OnInitialize}
};

use sp_core::H256;
use sp_runtime::{
	Perbill, ModuleId,
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, BadOrigin},
	DispatchError,
};

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

mod bounties {
	// Re-export needed for `impl_outer_event!`.
	pub use crate::*;
}

impl_outer_event! {
	pub enum Event for Test {
		system<T>,
		pallet_balances<T>,
		pallet_treasury<T>,
		bounties<T>,
	}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u128; // u64 is not enough to hold bytes used to generate bounty account
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = u64;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}
thread_local! {
	static TEN_TO_FOURTEEN: RefCell<Vec<u128>> = RefCell::new(vec![10,11,12,13,14]);
}
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: u64 = 1;
	pub const SpendPeriod: u64 = 2;
	pub const Burn: Permill = Permill::from_percent(50);
	pub const DataDepositPerByte: u64 = 1;
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
}
// impl pallet_treasury::Config for Test {
impl pallet_treasury::Config for Test {
	type ModuleId = TreasuryModuleId;
	type Currency = pallet_balances::Module<Test>;
	type ApproveOrigin = frame_system::EnsureRoot<u128>;
	type RejectOrigin = frame_system::EnsureRoot<u128>;
	type Event = Event;
	type OnSlash = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();  // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = Bounties;
}
parameter_types! {
	pub const BountyDepositBase: u64 = 80;
	pub const BountyDepositPayoutDelay: u64 = 3;
	pub const BountyUpdatePeriod: u32 = 20;
	pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
	pub const BountyValueMinimum: u64 = 1;
	pub const MaximumReasonLength: u32 = 16384;
	pub const MaxSubBountyCount: u32 = 3;
}
impl Config for Test {
	type Event = Event;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type BountyCuratorDeposit = BountyCuratorDeposit;
	type BountyValueMinimum = BountyValueMinimum;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type MaxSubBountyCount = MaxSubBountyCount;
	type WeightInfo = ();
}
type System = frame_system::Module<Test>;
type Balances = pallet_balances::Module<Test>;
type Treasury = pallet_treasury::Module<Test>;
type Bounties = Module<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test>{
		// Total issuance will be 200 with treasury account initialized at ED.
		balances: vec![(0, 100), (1, 98), (2, 1)],
	}.assimilate_storage(&mut t).unwrap();
	pallet_treasury::GenesisConfig::default().assimilate_storage::<Test, _>(&mut t).unwrap();
	t.into()
}

fn last_event() -> RawEvent<u64, u128> {
	System::events().into_iter().map(|r| r.event)
		.filter_map(|e| {
			if let Event::bounties(inner) = e { Some(inner) } else { None }
		})
		.last()
		.unwrap()
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Treasury::pot(), 0);
		assert_eq!(Treasury::proposal_count(), 0);
	});
}

#[test]
fn minting_works() {
	new_test_ext().execute_with(|| {
		// Check that accumulate works when we have Some value in Dummy already.
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);
	});
}

#[test]
fn spend_proposal_takes_min_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 1, 3));
		assert_eq!(Balances::free_balance(0), 99);
		assert_eq!(Balances::reserved_balance(0), 1);
	});
}

#[test]
fn spend_proposal_takes_proportional_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_eq!(Balances::free_balance(0), 95);
		assert_eq!(Balances::reserved_balance(0), 5);
	});
}

#[test]
fn spend_proposal_fails_when_proposer_poor() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Treasury::propose_spend(Origin::signed(2), 100, 3),
			Error::<Test>::InsufficientProposersBalance,
		);
	});
}

#[test]
fn accepted_spend_proposal_ignored_outside_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0));

		<Treasury as OnInitialize<u64>>::on_initialize(1);
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Treasury::pot(), 100);
	});
}

#[test]
fn unused_pot_should_diminish() {
	new_test_ext().execute_with(|| {
		let init_total_issuance = Balances::total_issuance();
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 100);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(), 50);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 50);
	});
}

#[test]
fn rejected_spend_proposal_ignored_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Treasury::pot(), 50);
	});
}

#[test]
fn reject_already_rejected_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0));
		assert_noop!(Treasury::reject_proposal(Origin::root(), 0), Error::<Test>::InvalidIndex);
	});
}

#[test]
fn reject_non_existent_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(Treasury::reject_proposal(Origin::root(), 0), Error::<Test>::InvalidIndex);
	});
}

#[test]
fn accept_non_existent_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(Treasury::approve_proposal(Origin::root(), 0), Error::<Test>::InvalidIndex);
	});
}

#[test]
fn accept_already_rejected_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0));
		assert_noop!(Treasury::approve_proposal(Origin::root(), 0), Error::<Test>::InvalidIndex);
	});
}

#[test]
fn accepted_spend_proposal_enacted_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 100, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(3), 100);
		assert_eq!(Treasury::pot(), 0);
	});
}

#[test]
fn pot_underflow_should_not_diminish() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 150, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(), 100); // Pot hasn't changed

		let _ = Balances::deposit_into_existing(&Treasury::account_id(), 100).unwrap();
		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Balances::free_balance(3), 150); // Fund has been spent
		assert_eq!(Treasury::pot(), 25); // Pot has finally changed
	});
}

// Treasury account doesn't get deleted if amount approved to spend is all its free balance.
// i.e. pot should not include existential deposit needed for account survival.
#[test]
fn treasury_account_doesnt_get_deleted() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);
		let treasury_balance = Balances::free_balance(&Treasury::account_id());

		assert_ok!(Treasury::propose_spend(Origin::signed(0), treasury_balance, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(), 100); // Pot hasn't changed

		assert_ok!(Treasury::propose_spend(Origin::signed(0), Treasury::pot(), 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 1));

		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Treasury::pot(), 0); // Pot is emptied
		assert_eq!(Balances::free_balance(Treasury::account_id()), 1); // but the account is still there
	});
}

// In case treasury account is not existing then it works fine.
// This is useful for chain that will just update runtime.
#[test]
fn inexistent_account_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test>{
		balances: vec![(0, 100), (1, 99), (2, 1)],
	}.assimilate_storage(&mut t).unwrap();
	// Treasury genesis config is not build thus treasury account does not exist
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(Balances::free_balance(Treasury::account_id()), 0); // Account does not exist
		assert_eq!(Treasury::pot(), 0); // Pot is empty

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 99, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0));
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 1, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 1));
		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(), 0); // Pot hasn't changed
		assert_eq!(Balances::free_balance(3), 0); // Balance of `3` hasn't changed

		Balances::make_free_balance_be(&Treasury::account_id(), 100);
		assert_eq!(Treasury::pot(), 99); // Pot now contains funds
		assert_eq!(Balances::free_balance(Treasury::account_id()), 100); // Account does exist

		<Treasury as OnInitialize<u64>>::on_initialize(4);

		assert_eq!(Treasury::pot(), 0); // Pot has changed
		assert_eq!(Balances::free_balance(3), 99); // Balance of `3` has changed
	});
}

#[test]
fn propose_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 10, b"1234567890".to_vec()));

		assert_eq!(last_event(), RawEvent::BountyProposed(0));

		let deposit: u64 = 85 + 5;
		assert_eq!(Balances::reserved_balance(0), deposit);
		assert_eq!(Balances::free_balance(0), 100 - deposit);

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 0,
			curator_deposit: 0,
			value: 10,
			bond: deposit,
			status: BountyStatus::Proposed,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Bounties::bounty_descriptions(0).unwrap(), b"1234567890".to_vec());

		assert_eq!(Bounties::bounty_count(), 1);
	});
}

#[test]
fn propose_bounty_validation_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_eq!(Treasury::pot(), 100);

		assert_noop!(
			Bounties::propose_bounty(Origin::signed(1), 0, [0; 17_000].to_vec()),
			Error::<Test>::ReasonTooBig
		);

		assert_noop!(
			Bounties::propose_bounty(Origin::signed(1), 10, b"12345678901234567890".to_vec()),
			Error::<Test>::InsufficientProposersBalance
		);

		assert_noop!(
			Bounties::propose_bounty(Origin::signed(1), 0, b"12345678901234567890".to_vec()),
			Error::<Test>::InvalidValue
		);
	});
}

#[test]
fn close_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_noop!(Bounties::close_bounty(Origin::root(), 0), Error::<Test>::InvalidIndex);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 10, b"12345".to_vec()));

		assert_ok!(Bounties::close_bounty(Origin::root(), 0));

		let deposit: u64 = 80 + 5;

		assert_eq!(last_event(), RawEvent::BountyRejected(0, deposit));

		assert_eq!(Balances::reserved_balance(0), 0);
		assert_eq!(Balances::free_balance(0), 100 - deposit);

		assert_eq!(Bounties::bounties(0), None);
		assert!(!pallet_treasury::Proposals::<Test>::contains_key(0));

		assert_eq!(Bounties::bounty_descriptions(0), None);
	});
}

#[test]
fn approve_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_noop!(Bounties::approve_bounty(Origin::root(), 0), Error::<Test>::InvalidIndex);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		let deposit: u64 = 80 + 5;

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 0,
			value: 50,
			curator_deposit: 0,
			bond: deposit,
			status: BountyStatus::Approved,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});
		assert_eq!(Bounties::bounty_approvals(), vec![0]);

		assert_noop!(Bounties::close_bounty(Origin::root(), 0), Error::<Test>::UnexpectedStatus);

		// deposit not returned yet
		assert_eq!(Balances::reserved_balance(0), deposit);
		assert_eq!(Balances::free_balance(0), 100 - deposit);

		<Treasury as OnInitialize<u64>>::on_initialize(2);

		// return deposit
		assert_eq!(Balances::reserved_balance(0), 0);
		assert_eq!(Balances::free_balance(0), 100);

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 0,
			curator_deposit: 0,
			value: 50,
			bond: deposit,
			status: BountyStatus::Funded,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Treasury::pot(), 100 - 50 - 25); // burn 25
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 50);
	});
}

#[test]
fn assign_curator_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_noop!(Bounties::propose_curator(Origin::root(), 0, 4, 4), Error::<Test>::InvalidIndex);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_noop!(Bounties::propose_curator(Origin::root(), 0, 4, 50), Error::<Test>::InvalidFee);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::CuratorProposed {
				curator: 4,
			},
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_noop!(Bounties::accept_curator(Origin::signed(1), 0), Error::<Test>::RequireCurator);
		assert_noop!(Bounties::accept_curator(Origin::signed(4), 0), pallet_balances::Error::<Test, _>::InsufficientBalance);

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 2,
			value: 50,
			bond: 85,
			status: BountyStatus::Active {
				curator: 4,
				update_due: 22,
			},
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Balances::free_balance(&4), 8);
		assert_eq!(Balances::reserved_balance(&4), 2);
	});
}

#[test]
fn unassign_curator_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_noop!(Bounties::unassign_curator(Origin::signed(1), 0), BadOrigin);

		assert_ok!(Bounties::unassign_curator(Origin::signed(4), 0));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::Funded,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_ok!(Bounties::unassign_curator(Origin::root(), 0));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::Funded,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Balances::free_balance(&4), 8);
		assert_eq!(Balances::reserved_balance(&4), 0); // slashed 2
	});
}


#[test]
fn award_and_claim_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		Balances::make_free_balance_be(&4, 10);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));
		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_eq!(Balances::free_balance(4), 8); // inital 10 - 2 deposit

		assert_noop!(Bounties::award_bounty(Origin::signed(1), 0, 3), Error::<Test>::RequireCurator);

		assert_ok!(Bounties::award_bounty(Origin::signed(4), 0, 3));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 2,
			value: 50,
			bond: 85,
			status: BountyStatus::PendingPayout {
				curator: 4,
				beneficiary: 3,
				unlock_at: 5
			},
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_noop!(Bounties::claim_bounty(Origin::signed(1), 0), Error::<Test>::Premature);

		System::set_block_number(5);
		<Treasury as OnInitialize<u64>>::on_initialize(5);

		assert_ok!(Balances::transfer(Origin::signed(0), Bounties::bounty_account_id(0), 10));

		assert_ok!(Bounties::claim_bounty(Origin::signed(1), 0));

		assert_eq!(last_event(), RawEvent::BountyClaimed(0, 56, 3));

		assert_eq!(Balances::free_balance(4), 14); // initial 10 + fee 4

		assert_eq!(Balances::free_balance(3), 56);
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);

		assert_eq!(Bounties::bounties(0), None);
		assert_eq!(Bounties::bounty_descriptions(0), None);
	});
}

#[test]
fn claim_handles_high_fee() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		Balances::make_free_balance_be(&4, 30);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 49));
		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_ok!(Bounties::award_bounty(Origin::signed(4), 0, 3));

		System::set_block_number(5);
		<Treasury as OnInitialize<u64>>::on_initialize(5);

		// make fee > balance
		let _ = Balances::slash(&Bounties::bounty_account_id(0), 10);

		assert_ok!(Bounties::claim_bounty(Origin::signed(1), 0));

		assert_eq!(last_event(), RawEvent::BountyClaimed(0, 0, 3));

		assert_eq!(Balances::free_balance(4), 70); // 30 + 50 - 10
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);

		assert_eq!(Bounties::bounties(0), None);
		assert_eq!(Bounties::bounty_descriptions(0), None);
	});
}

#[test]
fn cancel_and_refund() {
	new_test_ext().execute_with(|| {

		System::set_block_number(1);

		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Balances::transfer(Origin::signed(0), Bounties::bounty_account_id(0), 10));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 0,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::Funded,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 60);

		assert_noop!(Bounties::close_bounty(Origin::signed(0), 0), BadOrigin);

		assert_ok!(Bounties::close_bounty(Origin::root(), 0));

		assert_eq!(Treasury::pot(), 85); // - 25 + 10

	});

}

#[test]
fn award_and_cancel() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 0, 10));
		assert_ok!(Bounties::accept_curator(Origin::signed(0), 0));

		assert_eq!(Balances::free_balance(0), 95);
		assert_eq!(Balances::reserved_balance(0), 5);

		assert_ok!(Bounties::award_bounty(Origin::signed(0), 0, 3));

		// Cannot close bounty directly when payout is happening...
		assert_noop!(Bounties::close_bounty(Origin::root(), 0), Error::<Test>::PendingPayout);

		// Instead unassign the curator to slash them and then close.
		assert_ok!(Bounties::unassign_curator(Origin::root(), 0));
		assert_ok!(Bounties::close_bounty(Origin::root(), 0));

		assert_eq!(last_event(), RawEvent::BountyCanceled(0));

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);

		// Slashed.
		assert_eq!(Balances::free_balance(0), 95);
		assert_eq!(Balances::reserved_balance(0), 0);

		assert_eq!(Bounties::bounties(0), None);
		assert_eq!(Bounties::bounty_descriptions(0), None);
	});
}

#[test]
fn expire_and_unassign() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 1, 10));
		assert_ok!(Bounties::accept_curator(Origin::signed(1), 0));

		assert_eq!(Balances::free_balance(1), 93);
		assert_eq!(Balances::reserved_balance(1), 5);

		System::set_block_number(22);
		<Treasury as OnInitialize<u64>>::on_initialize(22);

		assert_noop!(Bounties::unassign_curator(Origin::signed(0), 0), Error::<Test>::Premature);

		System::set_block_number(23);
		<Treasury as OnInitialize<u64>>::on_initialize(23);

		assert_ok!(Bounties::unassign_curator(Origin::signed(0), 0));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 10,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::Funded,
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Balances::free_balance(1), 93);
		assert_eq!(Balances::reserved_balance(1), 0); // slashed

	});
}

#[test]
fn extend_expiry() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);
		Balances::make_free_balance_be(&4, 10);
		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_noop!(Bounties::extend_bounty_expiry(Origin::signed(1), 0, Vec::new()), Error::<Test>::UnexpectedStatus);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 10));
		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_eq!(Balances::free_balance(4), 5);
		assert_eq!(Balances::reserved_balance(4), 5);

		System::set_block_number(10);
		<Treasury as OnInitialize<u64>>::on_initialize(10);

		assert_noop!(Bounties::extend_bounty_expiry(Origin::signed(0), 0, Vec::new()), Error::<Test>::RequireCurator);
		assert_ok!(Bounties::extend_bounty_expiry(Origin::signed(4), 0, Vec::new()));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 10,
			curator_deposit: 5,
			value: 50,
			bond: 85,
			status: BountyStatus::Active { curator: 4, update_due: 30 },
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_ok!(Bounties::extend_bounty_expiry(Origin::signed(4), 0, Vec::new()));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 10,
			curator_deposit: 5,
			value: 50,
			bond: 85,
			status: BountyStatus::Active { curator: 4, update_due: 30 }, // still the same
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		System::set_block_number(25);
		<Treasury as OnInitialize<u64>>::on_initialize(25);

		assert_noop!(Bounties::unassign_curator(Origin::signed(0), 0), Error::<Test>::Premature);
		assert_ok!(Bounties::unassign_curator(Origin::signed(4), 0));

		assert_eq!(Balances::free_balance(4), 10); // not slashed
		assert_eq!(Balances::reserved_balance(4), 0);
	});
}

#[test]
fn genesis_funding_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let initial_funding = 100;
	pallet_balances::GenesisConfig::<Test>{
		// Total issuance will be 200 with treasury account initialized with 100.
		balances: vec![(0, 100), (Treasury::account_id(), initial_funding)],
	}.assimilate_storage(&mut t).unwrap();
	pallet_treasury::GenesisConfig::default().assimilate_storage::<Test, _>(&mut t).unwrap();
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(Balances::free_balance(Treasury::account_id()), initial_funding);
		assert_eq!(Treasury::pot(), initial_funding - Balances::minimum_balance());
	});
}

#[test]
fn subbunty_add_subbounty_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough bounty fund & master-curator.
		// 2, Master-curator adds subbounty Subbounty-1, test for error like RequireCurator
		//    ,InsufficientProposersBalance, InsufficientBountyBalance with invalid arguments.
		// 3, Master-curator adds subbounty Subbounty-1, moves to "Approved" state &
		//    test for the event SubBountyApproved.
		// 4, Test for DB state of `Bounties` & `SubBounties`.
		// 5, Observe fund transaction moment between Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 0,
			value: 50,
			bond: 85,
			status: BountyStatus::CuratorProposed {
				curator: 4,
			},
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			fee: 4,
			curator_deposit: 2,
			value: 50,
			bond: 85,
			status: BountyStatus::Active {
				curator: 4,
				update_due: 22,
			},
			subbountycount: 0u32.into(),
			activesubbounty: Default::default(),
		});

		assert_eq!(Balances::free_balance(&4), 8);
		assert_eq!(Balances::reserved_balance(&4), 2);

		// ===Pre-steps :: Add subbounty===
		// Acc-4 is the master curator
		// Call from invalid origin & check for error "RequireCurator"
		assert_noop!(
			Bounties::add_subbounty(Origin::signed(0), 0, 50, b"12345-p1".to_vec()),
			Error::<Test>::RequireCurator,
		);

		// Update the master curator
		Balances::make_free_balance_be(&4, 101);

		// master curator deposit is reserved for parent bounty.
		assert_eq!(Balances::free_balance(4), 101);
		assert_eq!(Balances::reserved_balance(4), 2);

		// master curator fee is reserved on parent bounty account.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 46);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// Add subbounty value is equal or greater than parent bounty value,
		// & check for error InsufficientBountyBalance.
		assert_noop!(
			Bounties::add_subbounty(Origin::signed(4), 0, 50, b"12345-p1".to_vec()),
			Error::<Test>::InsufficientBountyBalance,
		);

		// Add subbounty with valid value, which can be funded by parent bounty.
		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-p1".to_vec())
		);

		// Check for the event SubBountyApproved
		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 101);
		assert_eq!(Balances::reserved_balance(4), 2);

		// DB Status
		// Check the parent bounty status.
		assert_eq!(Bounties::bounties(0).unwrap(), Bounty {
			proposer: 0,
			value: 50,
			fee: 4,
			curator_deposit: 2,
			bond: 85,
			status: BountyStatus::Active {
				curator: 4,
				update_due: 22,
			},
			subbountycount: 1,
			activesubbounty: [
				1,
			].to_vec(),
		});

		// Check the subbounty status.
		assert_eq!(Bounties::subbounties(0,1).unwrap(), SubBounty {
			proposer: 4,
			value: 10,
			fee: 0,
			curator_deposit: 0,
			status: BountyStatus::Approved,
		});

		// Check the subbounty description status.
		assert_eq!(
			Bounties::subbounty_descriptions(0,1).unwrap(),
			b"12345-p1".to_vec(),
		);

		// Check the subbounty approval status.
		assert_eq!(Bounties::subbounty_approvals(),
			[(0,1)].to_vec()
		);

	});
}

#[test]
fn subbunty_assign_subcurator_works() {
	new_test_ext().execute_with(|| {
// TestProcedure
		// 1, Create bounty & move to active state with enough bounty fund & master-curator.
		// 2, Master-curator adds subbounty Subbounty-1, test for error like RequireCurator
		//    with invalid arguments.
		// 3, Master-curator adds subbounty Subbounty-1, moves to "Active" state.
		// 4, Test for DB state of `SubBounties`.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 50, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		Balances::make_free_balance_be(&4, 101);

		// ===Pre-steps :: Add subbounty===
		// Acc-4 is the master curator & make sure enough deposit
		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-p1".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		System::set_block_number(6);
		<Treasury as OnInitialize<u64>>::on_initialize(6);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4), 0, 1, 4, 2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap(), SubBounty {
			proposer: 4,
			value: 10,
			fee: 0,
			curator_deposit: 0,
			status: BountyStatus::CuratorProposed {
				curator: 4,
			},
		});

		assert_eq!(Balances::free_balance(4), 101);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_noop!(
			Bounties::accept_subcurator(Origin::signed(3),0,1),
			Error::<Test>::RequireCurator,
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(4),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap(), SubBounty {
				proposer: 4,
				value: 10,
				fee: 0,
				curator_deposit: 0,
				status: BountyStatus::Active {
					curator: 4,
					update_due: 26,
				},
			}
		);
		// no deposit for subcurator who is also curator of parnet bounty
		assert_eq!(Balances::free_balance(4), 101);
		assert_eq!(Balances::reserved_balance(4), 2);
	});
}

#[test]
fn subbunty_cfg_max_subbounty_count_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough bounty fund & master-curator.
		// 2, Test runtime is configured to take 3 subbounties.
		// 3, Master-curator adds 3 subbounty Subbounty-1,2 & 3 & test for error
		//    like SubBountyMaxOverflow.
		// 4, Move active subbounty, to "Active" state by assigning & accepting
		//    subcurator.
		// 4, Test for DB state of `SubBounties`.
		// 5, Observe fund transaction moment between Bounty, Subbounty,
		//    Curator, Subcurator.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 34, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// ===Pre-steps :: Add Max number of subbounty===
		// Acc-4 is the master curator & make sure enough
		// balance for subbounty deposit.
		Balances::make_free_balance_be(&4, 301);

		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb03".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,3));

		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Test runtime supports Max of 3 Subbounties
		assert_noop!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb04".to_vec()),
			Error::<Test>::SubBountyMaxOverflow,
		);

		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		System::set_block_number(6);
		<Treasury as OnInitialize<u64>>::on_initialize(6);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,3));

		// ===XXX_subcurator() api test===
		// test propose_subcurator() with invalid curator.
		assert_noop!(
			Bounties::propose_subcurator(Origin::signed(4),0,0,5,2),
			Error::<Test>::InvalidIndex,
		);

		// propose subcurator for all added 3 subbounties.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 5,
			},
		);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 6,
			},
		);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,3,7,2)
		);

		assert_eq!(Bounties::subbounties(0,3).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 7,
			},
		);

		// Test for curator balance, ensure all reserved deposits are reverted
		assert_eq!(Balances::free_balance(4), 301);
		assert_eq!(Balances::reserved_balance(4), 2);

		// ===Test accept_subcurator() for invalid curator===
		assert_noop!(
			Bounties::accept_subcurator(Origin::signed(3),0,1),
			Error::<Test>::RequireCurator,
		);

		// ===Test accept_subcurator() for invalid curator deposit balance===
		assert_noop!(
			Bounties::accept_subcurator(Origin::signed(5),0,1),
			pallet_balances::Error::<Test, _>::InsufficientBalance,
		);

		// ===Acc-5 accepts role of subcurator===
		Balances::make_free_balance_be(&5, 10);

		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 26,
			},
		);

		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// ===Acc-6 accepts role of subcurator===
		Balances::make_free_balance_be(&6, 10);

		assert_eq!(Balances::free_balance(6), 10);
		assert_eq!(Balances::reserved_balance(6), 0);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Active {
				curator: 6,
				update_due: 26,
			},
		);

		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// ===Acc-7 accepts role of subcurator===
		Balances::make_free_balance_be(&7, 10);

		assert_eq!(Balances::free_balance(7), 10);
		assert_eq!(Balances::reserved_balance(7), 0);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(7),0,3)
		);
		assert_eq!(Bounties::subbounties(0,3).unwrap().status,
			BountyStatus::Active {
				curator: 7,
				update_due: 26,
			},
		);
		assert_eq!(Balances::free_balance(7), 9);
		assert_eq!(Balances::reserved_balance(7), 1);
	});
}

#[test]
fn subbunty_award_claim_subbounty_works() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & propose sub-curator.
		// 3, Sub-curator accepts & award bounties.
		// 5, Ensure parent bounty is not closed after two subbounties are closed/claimed.
		// 6. Award the parent pounty and master-curator fee.
		// 7. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// ===XXX_subcurator() api test===
		// propose subcurator for all added 2 subbounties.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 5,
			},
		);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 6,
			},
		);

		// Test for curator balance, ensure all reserved deposits are reverted
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// ===Test accept_subcurator() for invalid curator===

		// ===Acc-5 accepts role of subcurator===
		Balances::make_free_balance_be(&5, 10);

		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// ===Acc-6 accepts role of subcurator===
		Balances::make_free_balance_be(&6, 10);

		assert_eq!(Balances::free_balance(6), 10);
		assert_eq!(Balances::reserved_balance(6), 0);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Active {
				curator: 6,
				update_due: 24,
			},
		);

		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// ===Award subbounties===
		// Test for invalid arguments
		assert_noop!(
			Bounties::award_subbounty(Origin::signed(3),0,1,7),
			Error::<Test>::RequireCurator,
		);

		// TODO :: Have to recheck
		// Master curator should be able to award bounty.
		assert_noop!(
			Bounties::award_subbounty(Origin::signed(3),0,1,7),
			Error::<Test>::RequireCurator,
		);

		// subbounty-1
		assert_ok!(Bounties::award_subbounty(Origin::signed(5),0,1,7));

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 5,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// subbounty-2
		assert_ok!(Bounties::award_subbounty(Origin::signed(6),0,2,8));

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 8,
				unlock_at: 7,
			},
		);

		// ===Claim subbounties===
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		Balances::make_free_balance_be(&7, 10);
		Balances::make_free_balance_be(&8, 10);

		assert_eq!(Balances::free_balance(7), 10);
		assert_eq!(Balances::reserved_balance(7), 0);

		assert_eq!(Balances::free_balance(8), 10);
		assert_eq!(Balances::reserved_balance(8), 0);

		// Test for Premature conditions
		assert_noop!(
			Bounties::claim_subbounty(Origin::signed(7),0,1),
			Error::<Test>::Premature,
		);

		System::set_block_number(9);

		assert_ok!(Bounties::claim_subbounty(Origin::signed(7),0,1));

		// Ensure subcurator is paid with curator fee & deposit refund
		assert_eq!(Balances::free_balance(5), 12);
		assert_eq!(Balances::reserved_balance(5), 0);

		// Ensure executor is paid with beneficiory amount.
		assert_eq!(Balances::free_balance(7), 18);
		assert_eq!(Balances::reserved_balance(7), 0);

		assert_ok!(Bounties::claim_subbounty(Origin::signed(7),0,2));

		// Ensure subcurator is paid with curator fee & deposit refund
		assert_eq!(Balances::free_balance(6), 12);
		assert_eq!(Balances::reserved_balance(6), 0);

		assert_eq!(Balances::free_balance(8), 18);
		assert_eq!(Balances::reserved_balance(8), 0);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===award parent pounty===
		// ensure state of parent bounty is active
		assert_eq!(Bounties::bounties(0).unwrap().status,
			BountyStatus::Active {
				curator: 4,
				update_due: 22,
			},
		);

		// call award bounty
		assert_ok!(Bounties::award_bounty(Origin::signed(4), 0, 4));
		assert_eq!(Bounties::bounties(0).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 4,
				beneficiary: 4,
				unlock_at: 12
			},
		);

		assert_noop!(
			Bounties::claim_bounty(Origin::signed(4), 0),
			Error::<Test>::Premature,
		);

		System::set_block_number(12);

		// Trust & Bounty balance before Claim
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::claim_bounty(Origin::signed(4), 0));

		// 205 = Initial balance(201) + master curator fee(4)
		assert_eq!(Balances::free_balance(4), 205);
		assert_eq!(Balances::reserved_balance(4), 0);

		// bounty account free & reserved is zero
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 0);

		// final treasury balance :: 12 = 11 + 1(from bounty free balance)
		assert_eq!(Balances::free_balance(Treasury::account_id()), 21);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}

#[test]
fn subbunty_close_subbounty_at_state_approved_works() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & moved to approved state.
		// 3, call close_subbounty() & observe for behaviour of Curator gets slashed.
		// 4. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Approved,
		);
		// ===Call close_subbounty===

		// Subbounty-1
		// TODO :: Have to recheck, Unable to close subbounty by root.
		// assert_ok!(Bounties::close_subbounty(Origin::root(), 0, 1));
		assert_ok!(Bounties::close_subbounty(Origin::signed(4), 0, 1));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// Subcurator or proposer of subbounty is slashed & lost deposit of 90
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Subbounty-2
		assert_ok!(Bounties::close_subbounty(Origin::signed(4), 0, 2));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// Subcurator or proposer of subbounty is slashed & lost deposit of 90
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);
	});
}

#[test]
fn subbunty_close_subbounty_at_state_funded_subcurator_proposed_works() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & moved to funded & subcurator proposed state.
		// 3, Call close_subbounty(), observe for bounty deposits are unreserved & no slashing.
		// 4. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Approved,
		);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// propose subcurator for subbounty-1.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 5,
			},
		);

		// Ensure subbounty-2 is at "Approved" state.
		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Funded,
		);

		// ===Call close_subbounty===

		// Subbounty-1
		// TODO :: Have to recheck, Unable to close subbounty by root.
		// assert_ok!(Bounties::close_subbounty(Origin::root(), 0, 1));
		assert_ok!(Bounties::close_subbounty(Origin::signed(4), 0, 1));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// No slash on Master-Curator
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Subbounty-2
		assert_ok!(Bounties::close_subbounty(Origin::signed(4), 0, 2));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// No slash on Master-Curator
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);
	});
}

#[test]
fn subbunty_close_subbounty_at_state_active_pendingpayout() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & move Subbounty-1 to active &
		//    Subbounty-2 to PendingPayout state.
		// 3, Call close_subbounty() from Rootorigin, observe for Subbounty-1
		//    subcurator deposit not gets slashed.
		// 4, Call close_subbounty() from subbounty-2 subcurator origin, observe
		//    for BadOrigin error.
		// 5, Call close_subbounty() from subbounty-2 mastercurator origin, observe
		//    for PendingPayout error.
		// 6. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Approved,
		);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move subbounty-1 to active state.
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// Move subbounty-2 to pending payout state.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_ok!(Bounties::award_subbounty(Origin::signed(6),0,2,7));

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// ===Call close_subbounty===

		// Subbounty-1
		assert_ok!(Bounties::close_subbounty(Origin::root(), 0, 1));

		// Subcurator deposit got unreserved without slashing.
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// Subbounty-2
		// Subcurator of Subbounty-2 tries to close_subbounty()
		// API throws "BadOrigin" error as expected.
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(6), 0, 2),
			DispatchError::BadOrigin,
		);

		// Master Curator of Subbounty-2 tries to close_subbounty()
		// API throws "PendingPayout" error as expected.
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(4), 0, 2),
			Error::<Test>::PendingPayout,
		);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// No slash on Master-Curator
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);
	});
}


#[test]
fn subbunty_close_subbounty_iterative_call_from_close_bounty_works() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & move Subbounty-1 to active &
		//    Subbounty-2 to PendingPayout state.
		// 3, Call close_bounty() from Rootorigin, observe for Subbounty-1 gets closed with
		//    subcurator deposit not gets slashed and Subbounty-2 returs error.
		// 4, Check the state of parent bounty for Invalid index.
		// 5, Claim on Subbounty-2 should work without any issue.
		// 7, Ensure master curator is not paid any fee, but deposit got unreserved.
		// 6. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Create the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for Master-curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Approved,
		);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move subbounty-1 to active state.
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);
		Balances::make_free_balance_be(&7, 10);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// Move subbounty-2 to pending payout state.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_ok!(Bounties::award_subbounty(Origin::signed(6),0,2,7));

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// ===Call close_bounty===
		// close parent bounty
		assert_ok!(Bounties::close_bounty(Origin::root(), 0));

		// Subbounty-1
		// Check for InvalidIndex error
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(4), 0, 1),
			Error::<Test>::InvalidIndex,
		);

		// Master curator deposit got unreserved without slashing.
		// Even though subbounty is partially executed. but no fee
		// paid to curator
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		// Subcurator deposit got unreserved without slashing.
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// As expected fund is holded for subbounty-2 payout
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 10);

		// Subbounty-2
		// close_subbounty check for active state of parent bounty
		// as expected call throws InvalidIndex error.
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(4), 0, 2),
			Error::<Test>::InvalidIndex,
		);

		// ensure subbounty-2 is still in pending payout state.
		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// move it to mature state.
		assert_noop!(
			Bounties::claim_subbounty(Origin::signed(7),0,2),
			Error::<Test>::Premature,
		);

		System::set_block_number(7);

		// Claim the payout os Subbounty-2
		assert_ok!(Bounties::claim_subbounty(Origin::signed(7),0,2));

		// Ensure subcurator is paid with fee & the deposit got unreserved
		assert_eq!(Balances::free_balance(6), 12);
		assert_eq!(Balances::reserved_balance(6), 0);

		// Ensure beneficiary got paid
		assert_eq!(Balances::free_balance(7), 18);
		assert_eq!(Balances::reserved_balance(7), 0);

		// TODO :: Have to recheck
		// Master curator deposit got unreserved without slashing.
		// Even though subbounty is partially executed(1/2). but no fee
		// paid to curator.
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		// graceful cleanup on parent bounty & treasury account
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 0);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 0);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 35);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}

#[test]
fn subbunty_unassign_subcurator_at_all_possible_state_works() {
	new_test_ext().execute_with(|| {

		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & move Subbounty-1 to "Funded" state &
		//    Subbounty-2 to CuratorProposed state.
		// 3, Call unassign_subcurator() for Subbounty-1 from Rootorigin,
		//    observe for error UnexpectedStatus.
		// 4, Call unassign_subcurator() for Subbounty-2 from Rootorigin,
		//    observe for OK status & Subbounty-2 moves to funded state.
		// 5, Master-curator moves Subbounty-1 & Subbounty-2 to "CuratorProposed" state.
		// 6, And test for BadOrigin error, by calling unassign_subcurator() from invalid origin.
		// 7, Call unassign_subcurator() on subbounty-1 from master curator origin.
		//    and observe OK result with subbounty-1 moved to "Funded" state
		// 8, Call unassign_subcurator() on subbounty-2 from subcurator origin,
		//    and observe OK result with subbounty-2 moved to "Funded" state
		// 9, Subbounty-1 & Subbounty-2 moved to "Active" state.
		// 10,Call unassign_subcurator() on subbounty-1 from Root origin,
		//    observe for OK result, with subbounty-1 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 11,Call unassign_subcurator() on subbounty-2 from Master curator origin,
		//    observe for OK result, with subbounty-2 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 12,Move Subbounty-1 to "Active" state & Subbounty-2 to PendingPayout state.
		// 13,Call unassign_subcurator() on subbounty-1 from subcurator origin,
		//    observe for OK result, with subbounty-1 moved to "Funded" state
		//    & subcurator deposit got unreserved.
		// 14,Call unassign_subcurator() on subbounty-2 from subcurator origin,
		//    observe for BadOrigin result.
		// 15,Call unassign_subcurator() on subbounty-2 from master curator origin,
		//    observe for OK result, with subbounty-2 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 6. Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_eq!(
			Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Approved,
		);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move subbounty-1 to active state.
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// Move subbounty-2 to pending payout state.
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_ok!(Bounties::award_subbounty(Origin::signed(6),0,2,7));

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// Ensure Subcurator deposit got reserved.
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// ===Call close_subbounty===

		// Subbounty-1
		assert_ok!(Bounties::close_subbounty(Origin::root(), 0, 1));

		// Subcurator deposit got unreserved without slashing.
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// Subbounty-2
		// Subcurator of Subbounty-2 tries to close_subbounty()
		// API throws "BadOrigin" error as expected.
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(6), 0, 2),
			DispatchError::BadOrigin,
		);

		// Master Curator of Subbounty-2 tries to close_subbounty()
		// API throws "PendingPayout" error as expected.
		assert_noop!(
			Bounties::close_subbounty(Origin::signed(4), 0, 2),
			Error::<Test>::PendingPayout,
		);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// As expected fund reserved for subbounty moved back to parent bounty
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		// No slash on Master-Curator
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);
	});
}

#[test]
fn subbunty_unassign_subcurator_at_state1_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds subbounty Subbounty-1 & moves to "Approved" state.
		// 3, Call unassign_subcurator() for Subbounty-1 from Rootorigin,
		//    observe for error UnexpectedStatus.
		// 4, Move Subbounty-1 to funded state.
		// 4, Call unassign_subcurator() for Subbounty-1 from Rootorigin,
		//    observe for OK status & Subbounty-1 moves to funded state.
		// 5, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.

		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		//Check for UnexpectedStatus error code
		assert_noop!(
			Bounties::unassign_subcurator(Origin::root(),0,1),
			Error::<Test>::UnexpectedStatus
		);

		// Check for subbounty-1 "Approved" state
		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,1));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move Subbounty-1 to curator proposed state
		Balances::make_free_balance_be(&5, 10);
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 5,
			},
		);

		// Call unassign subcurator from Invalid orign & look for Badorigin error
		assert_noop!(
			Bounties::unassign_subcurator(Origin::signed(6),0,1),
			DispatchError::BadOrigin,
		);

		// Call unassign subcurator from Rootorigin
		assert_ok!(Bounties::unassign_subcurator(Origin::root(),0,1));

		// No slash on Subcurator deposit
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

	});
}

#[test]
fn subbunty_unassign_subcurator_at_state2_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & moves Subbounty-1 & Subbounty-2
		//    to "CuratorProposed" state.
		// 3, Call unassign_subcurator() on subbounty-1 from master curator origin.
		//    and observe OK result with subbounty-1 moved to "Funded" state
		// 4, Call unassign_subcurator() on subbounty-2 from subcurator origin,
		//    and observe OK result with subbounty-2 moved to "Funded" state
		// 5, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move Subbounty-1 to curator proposed state
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 5,
			},
		);

		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::CuratorProposed {
				curator: 6,
			},
		);

		// subbounty-1
		// Check for subcurator account balance
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// Call unassign subcurator from Master curator origin.
		assert_ok!(Bounties::unassign_subcurator(Origin::signed(4),0,1));

		// No slash on Subcurator deposit
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// Check for subbounty-1 state moved to funded.
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Funded,
		);

		// subbounty-2
		// Check for subcurator account balance
		assert_eq!(Balances::free_balance(6), 10);
		assert_eq!(Balances::reserved_balance(6), 0);

		// Call unassign subcurator from Subcurator origin.
		assert_ok!(Bounties::unassign_subcurator(Origin::signed(6),0,2));

		// No slash on Subcurator deposit
		assert_eq!(Balances::free_balance(6), 10);
		assert_eq!(Balances::reserved_balance(6), 0);

		// Check for subbounty-2 state moved to funded.
		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Funded,
		);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

	});
}

#[test]
fn subbunty_unassign_subcurator_at_state3_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & moves Subbounty-1 &
		//    Subbounty-2 moved to "Active" state.
		// 3,Call unassign_subcurator() on subbounty-1 from Root origin,
		//    observe for OK result, with subbounty-1 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 4,Call unassign_subcurator() on subbounty-2 from Master curator origin,
		//    observe for OK result, with subbounty-2 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 5, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move Subbounty-1 to curator proposed state
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);

		// Subbounty-1
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		// Subcurator deposit got reserved
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// Subbounty-2
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,2,6,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6),0,2)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Active {
				curator: 6,
				update_due: 24,
			},
		);

		// Subcurator deposit got reserved
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// subbounty-1
		// Call unassign subcurator from Rootorigin.
		assert_ok!(Bounties::unassign_subcurator(Origin::root(),0,1));

		// Subcurator deposit got slashed
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 0);

		// State moved back to "Funded"
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Funded,
		);

		// Subbounty-2
		// Call unassign subcurator from Invalid origin.
		assert_noop!(
			Bounties::unassign_subcurator(Origin::signed(0), 0, 2),
			Error::<Test>::Premature,
		);

		// Move to mature state
		System::set_block_number(26);

		// Call unassign subcurator from Master curator origin.
		assert_ok!(Bounties::unassign_subcurator(Origin::signed(4),0,2));

		// Subcurator deposit got slashed
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 0);

		// Check for subbounty-2 state moved to funded.
		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Funded,
		);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}

#[test]
fn subbunty_unassign_subcurator_at_state4_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds two subbounties & moves Subbounty-1 to "Active" state
		//    & Subbounty-2 to PendingPayout state.
		// 3, Call unassign_subcurator() on subbounty-1 from subcurator origin,
		//    observe for OK result, with subbounty-1 moved to "Funded" state
		//    & subcurator deposit got unreserved.
		// 4, Call unassign_subcurator() on subbounty-2 from subcurator origin,
		//    observe for BadOrigin result.
		// 5, Call unassign_subcurator() on subbounty-2 from master curator origin,
		//    observe for OK result, with subbounty-2 moved to "Funded" state
		//    & subcurator deposit got slashed.
		// 6, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb02".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,2));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// reserved for two subbounties & master currator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		// Move the subbounties to "Funded State"
		System::set_block_number(4);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		// Test for the last funded subbounty
		assert_eq!(last_event(), RawEvent::SubBountyFunded(0,2));

		// Ensure Proposer/Master-Curator bond fund is unreserved.
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		// Move Subbounty-1 to curator proposed state
		Balances::make_free_balance_be(&5, 10);
		Balances::make_free_balance_be(&6, 10);

		// Subbounty-1
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4),0,1,5,2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);

		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 24,
			},
		);

		// Subcurator deposit got reserved
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// Subbounty-2
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4), 0, 2, 6, 2)
		);

		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(6), 0, 2)
		);

		assert_ok!(
			Bounties::award_subbounty(Origin::signed(6), 0, 2, 7)
		);

		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::PendingPayout {
				curator: 6,
				beneficiary: 7,
				unlock_at: 7,
			},
		);

		// Subcurator deposit got reserved
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 1);

		// subbounty-1
		// Call unassign subcurator from subcurator origin.
		assert_ok!(Bounties::unassign_subcurator(Origin::signed(5),0,1));

		// Subcurator deposit got unreserved as expected.
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// State moved back to "Funded"
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Funded,
		);

		// Subbounty-2
		// Call unassign subcurator from subcurator origin.
		assert_noop!(
			Bounties::unassign_subcurator(Origin::signed(6),0,2),
			DispatchError::BadOrigin,
		);

		// Call unassign subcurator from Master curator origin.
		assert_ok!(Bounties::unassign_subcurator(Origin::signed(4),0,2));

		// Subcurator deposit got slashed
		assert_eq!(Balances::free_balance(6), 9);
		assert_eq!(Balances::reserved_balance(6), 0);

		// Check for subbounty-2 state moved to funded.
		assert_eq!(Bounties::subbounties(0,2).unwrap().status,
			BountyStatus::Funded,
		);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 1);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 24);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 20);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}

#[test]
fn subbunty_extend_subbounty_expiry_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds subbounty Subbounty-1 & moves to "Approved" state.
		// 3, Call extend_subbounty_expiry() for Subbounty-1 from Rootorigin,
		//    observe for error UnexpectedStatus.
		// 4, Move Subbounty-1 to Active state.
		// 5, Check for subbounty "Active" status & update filed.
		// 6, Call extend_subbounty_expiry() for Subbounty-1 from Invalid origin,
		//    observe for error RequireCurator.
		// 7, Call extend_subbounty_expiry() for Subbounty-1 from Master Curator,
		//    observe for OK status & look for event SubBountyExtended.
		// 8, Check for subbounty "Active" status & update filed.
		// 9, Call extend_subbounty_expiry() for Subbounty-1 from SubCurator,
		//    observe for OK status & look for event SubBountyExtended.
		// 10, Check for subbounty "Active" status & update filed.
		// 11, Call unassign_subcurator() for Subbounty-1 from Invalid origin,
		//    observe for error Premature.
		// 12, Update the block count move to mature state.
		// 13, Call unassign_subcurator() for Subbounty-1 from Subcurator origin,
		//    observe OK status & subcurator deposit got unreserved.
		// 14, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add two subbounty===
		// Acc-4 is the master curator & make sure enough
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		// Check for Bad Origin error.
		assert_noop!(
			Bounties::extend_subbounty_expiry(Origin::root(), 0, 1, Vec::new()),
			DispatchError::BadOrigin
		);

		// Check for UnexpectedStatus error.
		assert_noop!(
			Bounties::extend_subbounty_expiry(Origin::signed(0), 0, 1, Vec::new()),
			Error::<Test>::UnexpectedStatus,
		);

		// Move Subbounty-1 to Active state
		Balances::make_free_balance_be(&5, 10);
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4), 0, 1, 5, 2)
		);
		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 22,
			},
		);

		System::set_block_number(15);

		// Check for RequireCurator error.
		assert_noop!(
			Bounties::extend_subbounty_expiry(Origin::signed(0), 0, 1, Vec::new()),
			Error::<Test>::RequireCurator,
		);

		// Origin is from Master Curator
		assert_ok!(
			Bounties::extend_subbounty_expiry(Origin::signed(4), 0, 1, Vec::new()),
		);
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 35,
			},
		);
		// check for SubBountyExtended event
		assert_eq!(last_event(), RawEvent::SubBountyExtended(0,1));

		System::set_block_number(25);

		// Origin is from SubCurator
		assert_ok!(
			Bounties::extend_subbounty_expiry(Origin::signed(5), 0, 1, Vec::new()),
		);
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 45,
			},
		);
		// check for SubBountyExtended event
		assert_eq!(last_event(), RawEvent::SubBountyExtended(0,1));

		// Call from invalid origin and look for Premature error
		assert_noop!(
			Bounties::unassign_subcurator(Origin::signed(0), 0, 1),
			Error::<Test>::Premature
		);

		// Call from subcurator origin and look for OK
		assert_ok!(
			Bounties::unassign_subcurator(Origin::signed(5), 0, 1),
		);

		// No slash on Subcurator deposit
		assert_eq!(Balances::free_balance(5), 10);
		assert_eq!(Balances::reserved_balance(5), 0);

		// Check for subbounty status to funded
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Funded,
		);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}

#[test]
fn subbunty_extend_subbounty_from_extend_bounty_expiry_works() {
	new_test_ext().execute_with(|| {
		// TestProcedure
		// 1, Create bounty & move to active state with enough fund & master-curator.
		// 2, Master-curator adds subbounty Subbounty-1 & moves to "Approved" state.
		// 3, Call extend_subbounty_expiry() for Subbounty-1 from Rootorigin,
		//    observe for error UnexpectedStatus.
		// 4, Move Subbounty-1 to Active state.
		// 5, Check for subbounty "Active" status & update field.
		// 6, Call extend_expiry() for parent bounty from Master curator origin,
		//    observe for OK status.
		// 7, Check for subbounty & bounty "Active" status & update field.
		// 8, Check for event SubBountyExtended & BountyExtended.
		// 9, Observe fund transaction moment between Treasury, Bounty, Subbounty,
		//    Curator, Subcurator & beneficiary.
		// ===Pre-steps :: Make the bounty or parent bounty===
		System::set_block_number(1);
		Balances::make_free_balance_be(&Treasury::account_id(), 101);

		// Bouty curator initial balance
		Balances::make_free_balance_be(&4, 201);
		assert_eq!(Balances::free_balance(4), 201);
		assert_eq!(Balances::reserved_balance(4), 0);

		assert_ok!(Bounties::propose_bounty(Origin::signed(0), 25, b"12345".to_vec()));

		assert_ok!(Bounties::approve_bounty(Origin::root(), 0));

		assert_eq!(Balances::free_balance(Treasury::account_id()), 101);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		System::set_block_number(2);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		assert_ok!(Bounties::propose_curator(Origin::root(), 0, 4, 4));

		assert_ok!(Bounties::accept_curator(Origin::signed(4), 0));

		// bounty & treasury account balance
		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);

		// amount 4 is reserved for curator fee
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 21);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 4);

		// ===Pre-steps :: Add subbounty===
		// Acc-4 is the master curator & make sure enough deposit
		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_ok!(
			Bounties::add_subbounty(Origin::signed(4), 0, 10, b"12345-sb01".to_vec())
		);

		assert_eq!(last_event(), RawEvent::SubBountyApproved(0,1));

		assert_eq!(Balances::free_balance(4), 199);
		assert_eq!(Balances::reserved_balance(4), 2);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(
			Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Approved,
		);

		// Move Subbounty-1 to Active state
		Balances::make_free_balance_be(&5, 10);
		assert_ok!(
			Bounties::propose_subcurator(Origin::signed(4), 0, 1, 5, 2)
		);
		assert_ok!(
			Bounties::accept_subcurator(Origin::signed(5),0,1)
		);
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 22,
			},
		);

		System::set_block_number(20);

		assert_ok!(Bounties::extend_bounty_expiry(Origin::signed(4), 0, Vec::new()));

		// Check status of subbounty
		assert_eq!(Bounties::subbounties(0,1).unwrap().status,
			BountyStatus::Active {
				curator: 5,
				update_due: 40,
			},
		);

		// Check status of parent bounty
		assert_eq!(Bounties::bounties(0).unwrap().status,
			BountyStatus::Active {
				curator: 4,
				update_due: 40,
			},
		);

		// check for BountyExtended event
		assert_eq!(last_event(), RawEvent::BountyExtended(0));

		// No slash on Subcurator deposit
		assert_eq!(Balances::free_balance(5), 9);
		assert_eq!(Balances::reserved_balance(5), 1);

		// As expected fund reserved for subbounty not moved.
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0)), 11);
		assert_eq!(Balances::reserved_balance(Bounties::bounty_account_id(0)), 14);

		assert_eq!(Balances::free_balance(Treasury::account_id()), 39);
		assert_eq!(Balances::reserved_balance(Treasury::account_id()), 0);
	});
}
