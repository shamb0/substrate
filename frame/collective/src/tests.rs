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

//! Collective pallet tests.

#![cfg(test)]

use super::*;
use frame_support::{Hashable, assert_ok, assert_noop, parameter_types, weights::Weight};
use frame_system::{self as system, EventRecord, Phase};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{
	Perbill, traits::{BlakeTwo256, IdentityLookup, Block as BlockT}, testing::Header,
	BuildStorage,
};
use crate as collective;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
	pub const MotionDuration: u64 = 3;
	pub const MaxProposals: u32 = 100;
	pub const MaxMembers: u32 = 100;
}
impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}
impl Config<Instance1> for Test {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = PrimeDefaultVote;
	type WeightInfo = ();
}
impl Config<Instance2> for Test {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityThenPrimeDefaultVote;
	type WeightInfo = ();
}
impl Config for Test {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = PrimeDefaultVote;
	type WeightInfo = ();
}

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, u64, Call, ()>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{Module, Call, Event<T>},
		Collective: collective::<Instance1>::{Module, Call, Event<T>, Origin<T>, Config<T>},
		CollectiveMajority: collective::<Instance2>::{Module, Call, Event<T>, Origin<T>, Config<T>},
		DefaultCollective: collective::{Module, Call, Event<T>, Origin<T>, Config<T>},
	}
);

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = GenesisConfig {
		collective_Instance1: Some(collective::GenesisConfig {
			members: vec![1, 2, 3],
			phantom: Default::default(),
		}),
		collective_Instance2: Some(collective::GenesisConfig {
			members: vec![1, 2, 3, 4, 5],
			phantom: Default::default(),
		}),
		collective: None,
	}.build_storage().unwrap().into();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn make_proposal(value: u64) -> Call {
	Call::System(frame_system::Call::remark(value.encode()))
}

#[test]
fn motions_basic_environment_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Collective::members(), vec![1, 2, 3]);
		assert_eq!(Collective::proposals(), Vec::<H256>::new());
	});
}

#[test]
fn close_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));

		System::set_block_number(3);
		assert_noop!(
			Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::TooEarly
		);

		System::set_block_number(4);
		assert_ok!(Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len));

		let record = |event| EventRecord { phase: Phase::Initialization, event, topics: vec![] };
		assert_eq!(System::events(), vec![
			record(Event::collective_Instance1(RawEvent::Proposed(1, 0, hash.clone(), 3))),
			record(Event::collective_Instance1(RawEvent::Voted(2, hash.clone(), true, 2, 0))),
			record(Event::collective_Instance1(RawEvent::Closed(hash.clone(), 2, 1))),
			record(Event::collective_Instance1(RawEvent::Disapproved(hash.clone())))
		]);
	});
}

#[test]
fn proposal_weight_limit_works_on_approve() {
	new_test_ext().execute_with(|| {
		let proposal = Call::Collective(crate::Call::set_members(vec![1, 2, 3], None, MaxMembers::get()));
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		// Set 1 as prime voter
		Prime::<Test, Instance1>::set(Some(1));
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		// With 1's prime vote, this should pass
		System::set_block_number(4);
		assert_noop!(
			Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight - 100, proposal_len),
			Error::<Test, Instance1>::WrongProposalWeight
		);
		assert_ok!(Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len));
	})
}

#[test]
fn proposal_weight_limit_ignored_on_disapprove() {
	new_test_ext().execute_with(|| {
		let proposal = Call::Collective(crate::Call::set_members(vec![1, 2, 3], None, MaxMembers::get()));
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		// No votes, this proposal wont pass
		System::set_block_number(4);
		assert_ok!(
			Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight - 100, proposal_len)
		);
	})
}

#[test]
fn close_with_prime_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::set_members(Origin::root(), vec![1, 2, 3], Some(3), MaxMembers::get()));

		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));

		System::set_block_number(4);
		assert_ok!(Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len));

		let record = |event| EventRecord { phase: Phase::Initialization, event, topics: vec![] };
		assert_eq!(System::events(), vec![
			record(Event::collective_Instance1(RawEvent::Proposed(1, 0, hash.clone(), 3))),
			record(Event::collective_Instance1(RawEvent::Voted(2, hash.clone(), true, 2, 0))),
			record(Event::collective_Instance1(RawEvent::Closed(hash.clone(), 2, 1))),
			record(Event::collective_Instance1(RawEvent::Disapproved(hash.clone())))
		]);
	});
}

#[test]
fn close_with_voting_prime_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::set_members(Origin::root(), vec![1, 2, 3], Some(1), MaxMembers::get()));

		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));

		System::set_block_number(4);
		assert_ok!(Collective::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len));

		let record = |event| EventRecord { phase: Phase::Initialization, event, topics: vec![] };
		assert_eq!(System::events(), vec![
			record(Event::collective_Instance1(RawEvent::Proposed(1, 0, hash.clone(), 3))),
			record(Event::collective_Instance1(RawEvent::Voted(2, hash.clone(), true, 2, 0))),
			record(Event::collective_Instance1(RawEvent::Closed(hash.clone(), 3, 0))),
			record(Event::collective_Instance1(RawEvent::Approved(hash.clone()))),
			record(Event::collective_Instance1(RawEvent::Executed(hash.clone(), Err(DispatchError::BadOrigin))))
		]);
	});
}

#[test]
fn close_with_no_prime_but_majority_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(CollectiveMajority::set_members(Origin::root(), vec![1, 2, 3, 4, 5], Some(5), MaxMembers::get()));

		assert_ok!(CollectiveMajority::propose(Origin::signed(1), 5, Box::new(proposal.clone()), proposal_len));
		assert_ok!(CollectiveMajority::vote(Origin::signed(2), hash.clone(), 0, true));
		assert_ok!(CollectiveMajority::vote(Origin::signed(3), hash.clone(), 0, true));

		System::set_block_number(4);
		assert_ok!(CollectiveMajority::close(Origin::signed(4), hash.clone(), 0, proposal_weight, proposal_len));

		let record = |event| EventRecord { phase: Phase::Initialization, event, topics: vec![] };
		assert_eq!(System::events(), vec![
			record(Event::collective_Instance2(RawEvent::Proposed(1, 0, hash.clone(), 5))),
			record(Event::collective_Instance2(RawEvent::Voted(2, hash.clone(), true, 2, 0))),
			record(Event::collective_Instance2(RawEvent::Voted(3, hash.clone(), true, 3, 0))),
			record(Event::collective_Instance2(RawEvent::Closed(hash.clone(), 5, 0))),
			record(Event::collective_Instance2(RawEvent::Approved(hash.clone()))),
			record(Event::collective_Instance2(RawEvent::Executed(hash.clone(), Err(DispatchError::BadOrigin))))
		]);
	});
}

#[test]
fn removal_of_old_voters_votes_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		let end = 4;
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![1, 2], nays: vec![], end })
		);
		Collective::change_members_sorted(&[4], &[1], &[2, 3, 4]);
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::propose(Origin::signed(2), 2, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(3), hash.clone(), 1, false));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![3], end })
		);
		Collective::change_members_sorted(&[], &[3], &[2, 4]);
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works_with_set_members() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		let end = 4;
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![1, 2], nays: vec![], end })
		);
		assert_ok!(Collective::set_members(Origin::root(), vec![2, 3, 4], None, MaxMembers::get()));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::propose(Origin::signed(2), 2, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(3), hash.clone(), 1, false));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![3], end })
		);
		assert_ok!(Collective::set_members(Origin::root(), vec![2, 4], None, MaxMembers::get()));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn propose_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_eq!(Collective::proposals(), vec![hash]);
		assert_eq!(Collective::proposal_of(&hash), Some(proposal));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![1], nays: vec![], end })
		);

		assert_eq!(System::events(), vec![
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Proposed(
					1,
					0,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					3,
				)),
				topics: vec![],
			}
		]);
	});
}

#[test]
fn limit_active_proposals() {
	new_test_ext().execute_with(|| {
		for i in 0..MaxProposals::get() {
			let proposal = make_proposal(i as u64);
			let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
			assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		}
		let proposal = make_proposal(MaxProposals::get() as u64 + 1);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len),
			Error::<Test, Instance1>::TooManyProposals
		);
	})
}

#[test]
fn correct_validate_and_get_proposal() {
	new_test_ext().execute_with(|| {
		let proposal = Call::Collective(crate::Call::set_members(vec![1, 2, 3], None, MaxMembers::get()));
		let length = proposal.encode().len() as u32;
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), length));

		let hash = BlakeTwo256::hash_of(&proposal);
		let weight = proposal.get_dispatch_info().weight;
		assert_noop!(
			Collective::validate_and_get_proposal(&BlakeTwo256::hash_of(&vec![3; 4]), length, weight),
			Error::<Test, Instance1>::ProposalMissing
		);
		assert_noop!(
			Collective::validate_and_get_proposal(&hash, length - 2, weight),
			Error::<Test, Instance1>::WrongProposalLength
		);
		assert_noop!(
			Collective::validate_and_get_proposal(&hash, length, weight - 10),
			Error::<Test, Instance1>::WrongProposalWeight
		);
		let res = Collective::validate_and_get_proposal(&hash, length, weight);
		assert_ok!(res.clone());
		let (retrieved_proposal, len) = res.unwrap();
		assert_eq!(length as usize, len);
		assert_eq!(proposal, retrieved_proposal);
	})
}

#[test]
fn motions_ignoring_non_collective_proposals_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(Origin::signed(42), 3, Box::new(proposal.clone()), proposal_len),
			Error::<Test, Instance1>::NotMember
		);
	});
}

#[test]
fn motions_ignoring_non_collective_votes_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_noop!(
			Collective::vote(Origin::signed(42), hash.clone(), 0, true),
			Error::<Test, Instance1>::NotMember,
		);
	});
}

#[test]
fn motions_ignoring_bad_index_collective_vote_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_noop!(
			Collective::vote(Origin::signed(2), hash.clone(), 1, true),
			Error::<Test, Instance1>::WrongIndex,
		);
	});
}

#[test]
fn motions_revoting_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![1], nays: vec![], end })
		);
		assert_noop!(
			Collective::vote(Origin::signed(1), hash.clone(), 0, true),
			Error::<Test, Instance1>::DuplicateVote,
		);
		assert_ok!(Collective::vote(Origin::signed(1), hash.clone(), 0, false));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![], nays: vec![1], end })
		);
		assert_noop!(
			Collective::vote(Origin::signed(1), hash.clone(), 0, false),
			Error::<Test, Instance1>::DuplicateVote,
		);

		assert_eq!(System::events(), vec![
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Proposed(
					1,
					0,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					2,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					1,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					false,
					0,
					1,
				)),
				topics: vec![],
			}
		]);

	});
}

#[test]
fn motions_all_first_vote_free_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		assert_eq!(
			Collective::voting(&hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![1], nays: vec![], end })
		);

		// For the motion, acc 2's first vote, expecting Ok with Pays::No ...
		assert_eq!( Collective::vote(Origin::signed(2), hash.clone(), 0, true),
			Ok(
				PostDispatchInfo {
					actual_weight: Some(
						207711000,
					),
					pays_fee: Pays::No,
				}
			)
		);

		// Duplicate vote, expecting error with Pays::Yes ...
		let vote_rval: DispatchResultWithPostInfo = Collective::vote(Origin::signed(2), hash.clone(), 0, true);
		match vote_rval {
			Ok(_) => {
				println!( "@[{:#?}::{:#?}]::vote-fee() | Should not Occur",
					file!(),
					line!() );
			},
			Err(err) => {
				assert_eq!( err.post_info.pays_fee, Pays::Yes );
			},
		}

		// Modifying vote, expecting ok with Pays::Yes ...
		assert_eq!( Collective::vote(Origin::signed(2), hash.clone(), 0, false),
			Ok(
				PostDispatchInfo {
					actual_weight: Some(
						207711000,
					),
					pays_fee: Pays::Yes,
				}
			)
		);

		// For the motion, acc 3's first vote, expecting Ok with Pays::No ...
		assert_eq!( Collective::vote(Origin::signed(3), hash.clone(), 0, true),
			Ok(
				PostDispatchInfo {
					actual_weight: Some(
						207711000,
					),
					pays_fee: Pays::No,
				}
			)
		);

		// acc 3 modify the vote, expecting Ok with Pays::Yes ...
		assert_eq!( Collective::vote(Origin::signed(3), hash.clone(), 0, false),
			Ok(
				PostDispatchInfo {
					actual_weight: Some(
						207711000,
					),
					pays_fee: Pays::Yes,
				}
			)
		);

		// Test close() Extrincis | Check DispatchResultWithPostInfo with Pay Info

		let proposal_weight = proposal.get_dispatch_info().weight;

		// trying to close the valid proposal, Expecting OK with Pays::No
		assert_eq!(
			Collective::close(Origin::signed(2), hash.clone(), 0, proposal_weight, proposal_len),
			Ok(
				PostDispatchInfo {
					actual_weight: Some(
						437711000,
					),
					pays_fee: Pays::No,
				},
			)
		);

		// trying to close the proposal, which is already closed. Expecting error "ProposalMissing" with Pays::Yes
		let close_rval: DispatchResultWithPostInfo = Collective::close(Origin::signed(2), hash.clone(), 0, proposal_weight, proposal_len);
		match close_rval {
			Ok(_) => {
				println!( "@[{:#?}::{:#?}]::vote-fee() | Should not Occur",
					file!(),
					line!() );
			},
			Err(err) => {
				assert_eq!( err.post_info.pays_fee, Pays::Yes );
			},
		}

		// Verify the events pool
		assert_eq!(System::events(), vec![
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Proposed(
					1,
					0,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					2
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					2,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					true,
					2,
					0
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					2,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					false,
					1,
					1
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					3,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					true,
					2,
					1
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					3,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					false,
					1,
					2
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Closed(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					1,
					2,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Disapproved(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
				)),
				topics: vec![],
			},
		]);
	});
}

#[test]
fn motions_reproposing_disapproved_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, false));
		assert_ok!(Collective::close(Origin::signed(2), hash.clone(), 0, proposal_weight, proposal_len));
		assert_eq!(Collective::proposals(), vec![]);
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		assert_eq!(Collective::proposals(), vec![hash]);
	});
}

#[test]
fn motions_disapproval_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 3, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, false));
		assert_ok!(Collective::close(Origin::signed(2), hash.clone(), 0, proposal_weight, proposal_len));

		assert_eq!(System::events(), vec![
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(
					RawEvent::Proposed(
						1,
						0,
						hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
						3,
					)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					2,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					false,
					1,
					1,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Closed(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(), 1, 1,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Disapproved(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
				)),
				topics: vec![],
			}
		]);
	});
}

#[test]
fn motions_approval_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));
		assert_ok!(Collective::close(Origin::signed(2), hash.clone(), 0, proposal_weight, proposal_len));

		assert_eq!(System::events(), vec![
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Proposed(
					1,
					0,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					2,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Voted(
					2,
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					true,
					2,
					0,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Closed(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(), 2, 0,
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Approved(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::Initialization,
				event: Event::collective_Instance1(RawEvent::Executed(
					hex!["68eea8f20b542ec656c6ac2d10435ae3bd1729efc34d1354ab85af840aad2d35"].into(),
					Err(DispatchError::BadOrigin),
				)),
				topics: vec![],
			}
		]);
	});
}

#[test]
fn close_disapprove_does_not_care_about_weight_or_len() {
	// This test confirms that if you close a proposal that would be disapproved,
	// we do not care about the proposal length or proposal weight since it will
	// not be read from storage or executed.
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		// First we make the proposal succeed
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));
		// It will not close with bad weight/len information
		assert_noop!(
			Collective::close(Origin::signed(2), hash.clone(), 0, 0, 0),
			Error::<Test, Instance1>::WrongProposalLength,
		);
		assert_noop!(
			Collective::close(Origin::signed(2), hash.clone(), 0, 0, proposal_len),
			Error::<Test, Instance1>::WrongProposalWeight,
		);
		// Now we make the proposal fail
		assert_ok!(Collective::vote(Origin::signed(1), hash.clone(), 0, false));
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, false));
		// It can close even if the weight/len information is bad
		assert_ok!(Collective::close(Origin::signed(2), hash.clone(), 0, 0, 0));
	})
}

#[test]
fn disapprove_proposal_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(Origin::signed(1), 2, Box::new(proposal.clone()), proposal_len));
		// Proposal would normally succeed
		assert_ok!(Collective::vote(Origin::signed(2), hash.clone(), 0, true));
		// But Root can disapprove and remove it anyway
		assert_ok!(Collective::disapprove_proposal(Origin::root(), hash.clone()));
		let record = |event| EventRecord { phase: Phase::Initialization, event, topics: vec![] };
		assert_eq!(System::events(), vec![
			record(Event::collective_Instance1(RawEvent::Proposed(1, 0, hash.clone(), 2))),
			record(Event::collective_Instance1(RawEvent::Voted(2, hash.clone(), true, 2, 0))),
			record(Event::collective_Instance1(RawEvent::Disapproved(hash.clone()))),
		]);
	})
}
