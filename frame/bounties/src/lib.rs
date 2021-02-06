// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
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

//! # Bounties Module ( pallet-bounties )
//!
//! ## Bounty
//!
//! > NOTE: This pallet is tightly coupled with pallet-treasury.
//!
//! A Bounty Spending is a reward for a specified body of work - or specified set of objectives -
//! that needs to be executed for a predefined Treasury amount to be paid out. A curator is assigned
//! after the bounty is approved and funded by Council, to be delegated with the responsibility of
//! assigning a payout address once the specified set of objectives is completed.
//!
//! After the Council has activated a bounty, it delegates the work that requires expertise to a
//! curator in exchange of a deposit. Once the curator accepts the bounty, they get to close the
//! active bounty. Closing the active bounty enacts a delayed payout to the payout address, the
//! curator fee and the return of the curator deposit. The delay allows for intervention through
//! regular democracy. The Council gets to unassign the curator, resulting in a new curator
//! election. The Council also gets to cancel the bounty if deemed necessary before assigning a
//! curator or once the bounty is active or payout is pending, resulting in the slash of the
//! curator's deposit.
//!
//!
//! ### Terminology
//!
//! Bounty:
//! - **Bounty spending proposal:** A proposal to reward a predefined body of work upon completion
//!   by the Treasury.
//! - **Proposer:** An account proposing a bounty spending.
//! - **Curator:** An account managing the bounty and assigning a payout address receiving the
//!   reward for the completion of work.
//! - **Deposit:** The amount held on deposit for placing a bounty proposal plus the amount held on
//!   deposit per byte within the bounty description.
//! - **Curator deposit:** The payment from a candidate willing to curate an approved bounty. The
//!   deposit is returned when/if the bounty is completed.
//! - **Bounty value:** The total amount that should be paid to the Payout Address if the bounty is
//!   rewarded.
//! - **Payout address:** The account to which the total or part of the bounty is assigned to.
//! - **Payout Delay:** The delay period for which a bounty beneficiary needs to wait before
//!   claiming.
//! - **Curator fee:** The reserved upfront payment for a curator for work related to the bounty.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! Bounty protocol:
//! - `propose_bounty` - Propose a specific treasury amount to be earmarked for a predefined set of
//!   tasks and stake the required deposit.
//! - `approve_bounty` - Accept a specific treasury amount to be earmarked for a predefined body of
//!   work.
//! - `propose_curator` - Assign an account to a bounty as candidate curator.
//! - `accept_curator` - Accept a bounty assignment from the Council, setting a curator deposit.
//! - `extend_bounty_expiry` - Extend the expiry block number of the bounty and stay active.
//! - `award_bounty` - Close and pay out the specified amount for the completed work.
//! - `claim_bounty` - Claim a specific bounty amount from the Payout Address.
//! - `unassign_curator` - Unassign an accepted curator from a specific earmark.
//! - `close_bounty` - Cancel the earmark for a specific treasury amount and close the bounty.

#![cfg_attr(not(feature = "std"), no_std)]

mod tests;
mod benchmarking;
pub mod weights;

use sp_std::{
	prelude::*,
};

use frame_support::{decl_module, decl_storage, decl_event, ensure, decl_error};

use frame_support::traits::{
	Currency, Get, Imbalance, OnUnbalanced, ExistenceRequirement::{AllowDeath},
	ReservableCurrency, WithdrawReasons,
};

use sp_runtime::{Permill, RuntimeDebug, DispatchResult, traits::{
	Zero, StaticLookup, AccountIdConversion, Saturating, BadOrigin,
	CheckedSub,
}};

use frame_support::dispatch::{DispatchError, DispatchResultWithPostInfo};

use frame_support::traits::{EnsureOrigin};

use frame_support::weights::{Weight};

use codec::{Encode, Decode};
use frame_system::{self as system, ensure_signed};
pub use weights::WeightInfo;

type BalanceOf<T> = pallet_treasury::BalanceOf<T>;

type PositiveImbalanceOf<T> = pallet_treasury::PositiveImbalanceOf<T>;

pub trait Config: frame_system::Config + pallet_treasury::Config {

	/// The amount held on deposit for placing a bounty proposal.
	type BountyDepositBase: Get<BalanceOf<Self>>;

	/// The delay period for which a bounty beneficiary need to wait before claim the payout.
	type BountyDepositPayoutDelay: Get<Self::BlockNumber>;

	/// Bounty duration in blocks.
	type BountyUpdatePeriod: Get<Self::BlockNumber>;

	/// Percentage of the curator fee that will be reserved upfront as deposit for bounty curator.
	type BountyCuratorDeposit: Get<Permill>;

	/// Minimum value for a bounty.
	type BountyValueMinimum: Get<BalanceOf<Self>>;

	/// The amount held on deposit per byte within the tip report reason or bounty description.
	type DataDepositPerByte: Get<BalanceOf<Self>>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Maximum acceptable reason length.
	type MaximumReasonLength: Get<u32>;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;

	/// Maximum number of subbounty that can be added to active bounty.
	type MaxActiveSubBountyCount: Get<u32>;
}

/// An index of a bounty. Just a `u32`.
pub type BountyIndex = u32;

/// A bounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Bounty<AccountId, Balance, BlockNumber> {
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the bounty is rewarded.
	value: Balance,
	/// The curator fee. Included in value.
	fee: Balance,
	/// The deposit of curator.
	curator_deposit: Balance,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,
	/// The status of this bounty.
	status: BountyStatus<AccountId, BlockNumber>,
	/// active Subbounty count
	active_subbounty_count: BountyIndex,
}

/// The status of a bounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum BountyStatus<AccountId, BlockNumber> {
	/// The bounty is proposed and waiting for approval.
	Proposed,
	/// The bounty is approved and waiting to become active at next spend period.
	Approved,
	/// The bounty is funded and waiting for curator assignment.
	Funded,
	/// A curator has been proposed by the `ApproveOrigin`. Waiting for acceptance from the curator.
	CuratorProposed {
		/// The assigned curator of this bounty.
		curator: AccountId,
	},
	/// The bounty is active and waiting to be awarded.
	Active {
		/// The curator of this bounty.
		curator: AccountId,
		/// An update from the curator is due by this block, else they are considered inactive.
		update_due: BlockNumber,
	},
	/// The bounty is awarded and waiting to released after a delay.
	PendingPayout {
		/// The curator of this bounty.
		curator: AccountId,
		/// The beneficiary of the bounty.
		beneficiary: AccountId,
		/// When the bounty can be claimed.
		unlock_at: BlockNumber,
	},
}

/// A Subbounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct SubBounty<AccountId, Balance, BlockNumber> {
	/// The (total) amount that should be paid if the sub-bounty is rewarded.
	value: Balance,
	/// The subcurator fee. Included in value.
	fee: Balance,
	/// The deposit of subcurator.
	curator_deposit: Balance,
	/// The status of this subbounty.
	status: SubBountyStatus<AccountId, BlockNumber>,
}

/// The status of a bounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum SubBountyStatus<AccountId, BlockNumber> {
	/// The Subbounty is added and waiting for curator assignment.
	Added,
	/// A Subcurator has been proposed by the `curator`. Waiting for acceptance from the subcurator.
	SubCuratorProposed {
		/// The assigned subcurator of this bounty.
		subcurator: AccountId,
	},
	/// The subbounty is active and waiting to be awarded.
	Active {
		/// The subcurator of this subbounty.
		subcurator: AccountId,
	},
	/// The subbounty is awarded and waiting to released after a delay.
	PendingPayout {
		/// The subcurator of this subbounty.
		subcurator: AccountId,
		/// The beneficiary of the subbounty.
		beneficiary: AccountId,
		/// When the subbounty can be claimed.
		unlock_at: BlockNumber,
	},
}

// Note :: For backward compatibility reasons,
// pallet-bounties uses Treasury for storage.
// This is temporary solution, soon will get replaced with
// Own storage identifier.
decl_storage! {
	trait Store for Module<T: Config> as Treasury {

		/// Number of bounty proposals that have been made.
		pub BountyCount get(fn bounty_count): BountyIndex;

		/// Bounties that have been made.
		pub Bounties get(fn bounties):
			map hasher(twox_64_concat) BountyIndex
			=> Option<Bounty<T::AccountId, BalanceOf<T>, T::BlockNumber>>;

		/// The description of each bounty.
		pub BountyDescriptions get(fn bounty_descriptions):
			map hasher(twox_64_concat) BountyIndex => Option<Vec<u8>>;

		/// Bounty indices that have been approved but not yet funded.
		pub BountyApprovals get(fn bounty_approvals): Vec<BountyIndex>;

		/// SubBounties that have been made.
		pub SubBounties get(fn subbounties):
			double_map hasher(twox_64_concat) BountyIndex,
			hasher(twox_64_concat) BountyIndex =>
			Option<SubBounty<T::AccountId, BalanceOf<T>, T::BlockNumber>>;
	}
}

decl_event!(
	pub enum Event<T>
	where
		Balance = BalanceOf<T>,
		<T as frame_system::Config>::AccountId,
	{
		/// New bounty proposal. \[index\]
		BountyProposed(BountyIndex),
		/// A bounty proposal was rejected; funds were slashed. \[index, bond\]
		BountyRejected(BountyIndex, Balance),
		/// A bounty proposal is funded and became active. \[index\]
		BountyBecameActive(BountyIndex),
		/// A bounty is awarded to a beneficiary. \[index, beneficiary\]
		BountyAwarded(BountyIndex, AccountId),
		/// A bounty is claimed by beneficiary. \[index, payout, beneficiary\]
		BountyClaimed(BountyIndex, Balance, AccountId),
		/// A bounty is cancelled. \[index\]
		BountyCanceled(BountyIndex),
		/// A bounty expiry is extended. \[index\]
		BountyExtended(BountyIndex),
		/// A subbounty is added. \[index, subbounty index\]
		SubBountyAdded(BountyIndex, BountyIndex),
		/// A subbounty is awarded to a beneficiary. \[index, subbounty index, beneficiary\]
		SubBountyAwarded(BountyIndex, BountyIndex, AccountId),
		/// A Subbounty is claimed by beneficiary. \[index, subbounty index, payout, beneficiary\]
		SubBountyClaimed(BountyIndex, BountyIndex, Balance, AccountId),
		/// A Subbounty proposal was rejected; funds were slashed. \[index, subbounty index, bond\]
		SubBountyRejected(BountyIndex, BountyIndex, Balance),
		/// A Subbounty is cancelled. \[index, subbounty index,\]
		SubBountyCanceled(BountyIndex, BountyIndex),
		/// A subbounty proposal is funded and became active. \[index, subbounty index\]
		SubBountyBecameActive(BountyIndex, BountyIndex),
		/// A Subbounty expiry is extended. \[index, subbounty index,\]
		SubBountyExtended(BountyIndex, BountyIndex),
	}
);

decl_error! {
	/// Error for the treasury module.
	pub enum Error for Module<T: Config> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal or bounty at that index.
		InvalidIndex,
		/// The reason given is just too big.
		ReasonTooBig,
		/// The bounty status is unexpected.
		UnexpectedStatus,
		/// Require bounty curator.
		RequireCurator,
		/// Invalid bounty value.
		InvalidValue,
		/// Invalid bounty fee.
		InvalidFee,
		/// A bounty payout is pending.
		/// To cancel the bounty, you must unassign and slash the curator.
		PendingPayout,
		/// The bounties cannot be claimed/closed because it's still in the countdown period.
		Premature,
		/// The bounty balance is not enough to add new subbounty.
		InsufficientBountyBalance,
		/// Subbounty active
		SubBountyActive,
		/// Number of subbounty exceeds threahold of TooManySubBounties
		TooManySubBounties,
		/// Require subbounty curator.
		RequireSubCurator,
	}
}

decl_module! {
	pub struct Module<T: Config>
		for enum Call
		where origin: T::Origin
	{
		/// The amount held on deposit per byte within bounty description.
		const DataDepositPerByte: BalanceOf<T> = T::DataDepositPerByte::get();

		/// The amount held on deposit for placing a bounty proposal.
		const BountyDepositBase: BalanceOf<T> = T::BountyDepositBase::get();

		/// The delay period for which a bounty beneficiary need to wait before claim the payout.
		const BountyDepositPayoutDelay: T::BlockNumber = T::BountyDepositPayoutDelay::get();

		/// Bounty duration in blocks.
		const BountyUpdatePeriod: T::BlockNumber = T::BountyUpdatePeriod::get();

		/// Percentage of the curator fee that will be reserved upfront as deposit for bounty curator.
		const BountyCuratorDeposit: Permill = T::BountyCuratorDeposit::get();

		/// Minimum value for a bounty.
		const BountyValueMinimum: BalanceOf<T> = T::BountyValueMinimum::get();

		/// Maximum acceptable reason length.
		const MaximumReasonLength: u32 = T::MaximumReasonLength::get();

		type Error = Error<T>;

		fn deposit_event() = default;

		/// Propose a new bounty.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// Payment: `TipReportDepositBase` will be reserved from the origin account, as well as
		/// `DataDepositPerByte` for each byte in `reason`. It will be unreserved upon approval,
		/// or slashed when rejected.
		///
		/// - `curator`: The curator account whom will manage this bounty.
		/// - `fee`: The curator fee.
		/// - `value`: The total payment amount of this bounty, curator fee included.
		/// - `description`: The description of this bounty.
		#[weight = <T as Config>::WeightInfo::propose_bounty(description.len() as u32)]
		fn propose_bounty(
			origin,
			#[compact] value: BalanceOf<T>,
			description: Vec<u8>,
		) {
			let proposer = ensure_signed(origin)?;
			Self::create_bounty(proposer, description, value)?;
		}

		/// Approve a bounty proposal. At a later time, the bounty will be funded and become active
		/// and the original deposit will be returned.
		///
		/// May only be called from `T::ApproveOrigin`.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::approve_bounty()]
		fn approve_bounty(origin, #[compact] bounty_id: BountyIndex) {
			T::ApproveOrigin::ensure_origin(origin)?;

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;
				ensure!(bounty.status == BountyStatus::Proposed, Error::<T>::UnexpectedStatus);

				bounty.status = BountyStatus::Approved;

				BountyApprovals::append(bounty_id);

				Ok(())
			})?;
		}

		/// Assign a curator to a funded bounty.
		///
		/// May only be called from `T::ApproveOrigin`.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::propose_curator()]
		fn propose_curator(
			origin,
			#[compact] bounty_id: BountyIndex,
			curator: <T::Lookup as StaticLookup>::Source,
			#[compact] fee: BalanceOf<T>,
		) {
			T::ApproveOrigin::ensure_origin(origin)?;

			let curator = T::Lookup::lookup(curator)?;
			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {

				let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;
				match bounty.status {
					BountyStatus::Proposed | BountyStatus::Approved | BountyStatus::Funded => {},
					_ => return Err(Error::<T>::UnexpectedStatus.into()),
				};

				ensure!(fee < bounty.value, Error::<T>::InvalidFee);

				bounty.status = BountyStatus::CuratorProposed { curator };
				bounty.fee = fee;

				Ok(())
			})?;
		}

		/// Unassign curator from a bounty.
		///
		/// This function can only be called by the `RejectOrigin` a signed origin.
		///
		/// If this function is called by the `RejectOrigin`, we assume that the curator is malicious
		/// or inactive. As a result, we will slash the curator when possible.
		///
		/// If the origin is the curator, we take this as a sign they are unable to do their job and
		/// they willingly give up. We could slash them, but for now we allow them to recover their
		/// deposit and exit without issue. (We may want to change this if it is abused.)
		///
		/// Finally, the origin can be anyone if and only if the curator is "inactive". This allows
		/// anyone in the community to call out that a curator is not doing their due diligence, and
		/// we should pick a new curator. In this case the curator should also be slashed.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::unassign_curator()]
		fn unassign_curator(origin, #[compact] bounty_id: BountyIndex) {
			let maybe_sender = ensure_signed(origin.clone())
				.map(Some)
				.or_else(|_| T::RejectOrigin::ensure_origin(origin).map(|_| None))?;

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;

				let slash_curator = |curator: &T::AccountId, curator_deposit: &mut BalanceOf<T>| {
					let imbalance = T::Currency::slash_reserved(curator, *curator_deposit).0;
					T::OnSlash::on_unbalanced(imbalance);
					*curator_deposit = Zero::zero();
				};

				match bounty.status {
					BountyStatus::Proposed | BountyStatus::Approved | BountyStatus::Funded => {
						// No curator to unassign at this point.
						return Err(Error::<T>::UnexpectedStatus.into())
					}
					BountyStatus::CuratorProposed { ref curator } => {
						// A curator has been proposed, but not accepted yet.
						// Either `RejectOrigin` or the proposed curator can unassign the curator.
						ensure!(maybe_sender.map_or(true, |sender| sender == *curator), BadOrigin);
					},
					BountyStatus::Active { ref curator, ref update_due } => {
						// The bounty is active.
						match maybe_sender {
							// If the `RejectOrigin` is calling this function, slash the curator.
							None => {
								slash_curator(curator, &mut bounty.curator_deposit);
								// Continue to change bounty status below...
							},
							Some(sender) => {
								// If the sender is not the curator, and the curator is inactive,
								// slash the curator.
								if sender != *curator {
									let block_number = system::Module::<T>::block_number();
									if *update_due < block_number {
										slash_curator(curator, &mut bounty.curator_deposit);
										// Continue to change bounty status below...
									} else {
										// Curator has more time to give an update.
										return Err(Error::<T>::Premature.into())
									}
								} else {
									// Else this is the curator, willingly giving up their role.
									// Give back their deposit.
									let _ = T::Currency::unreserve(&curator, bounty.curator_deposit);
									// Continue to change bounty status below...
								}
							},
						}
					},
					BountyStatus::PendingPayout { ref curator, .. } => {
						// The bounty is pending payout, so only council can unassign a curator.
						// By doing so, they are claiming the curator is acting maliciously, so
						// we slash the curator.
						ensure!(maybe_sender.is_none(), BadOrigin);
						slash_curator(curator, &mut bounty.curator_deposit);
						// Continue to change bounty status below...
					},
				};

				bounty.status = BountyStatus::Funded;
				Ok(())
			})?;
		}

		/// Accept the curator role for a bounty.
		/// A deposit will be reserved from curator and refund upon successful payout.
		///
		/// May only be called from the curator.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::accept_curator()]
		fn accept_curator(origin, #[compact] bounty_id: BountyIndex) {
			let signer = ensure_signed(origin)?;

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;

				match bounty.status {
					BountyStatus::CuratorProposed { ref curator } => {
						ensure!(signer == *curator, Error::<T>::RequireCurator);

						// Reserve the Curator deposit
						let deposit = T::BountyCuratorDeposit::get() * bounty.fee;
						T::Currency::reserve(curator, deposit)?;
						bounty.curator_deposit = deposit;

						let update_due = system::Module::<T>::block_number() + T::BountyUpdatePeriod::get();
						bounty.status = BountyStatus::Active { curator: curator.clone(), update_due };
						Ok(())
					},
					_ => Err(Error::<T>::UnexpectedStatus.into()),
				}
			})?;
		}

		/// Award bounty to a beneficiary account. The beneficiary will be able to claim the funds after a delay.
		///
		/// The dispatch origin for this call must be the curator of this bounty.
		///
		/// - `bounty_id`: Bounty ID to award.
		/// - `beneficiary`: The beneficiary account whom will receive the payout.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::award_bounty()]
		fn award_bounty(origin,
			#[compact] bounty_id: BountyIndex,
			beneficiary: <T::Lookup as StaticLookup>::Source
		) {
			let signer = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;

				// Ensure no active subbounties before processing the call.
				ensure!(bounty.active_subbounty_count == 0, Error::<T>::SubBountyActive);

				match &bounty.status {
					BountyStatus::Active {
						curator,
						..
					} => {
						ensure!(signer == *curator, Error::<T>::RequireCurator);
					},
					_ => return Err(Error::<T>::UnexpectedStatus.into()),
				}
				bounty.status = BountyStatus::PendingPayout {
					curator: signer,
					beneficiary: beneficiary.clone(),
					unlock_at: system::Module::<T>::block_number() + T::BountyDepositPayoutDelay::get(),
				};

				Ok(())
			})?;

			Self::deposit_event(Event::<T>::BountyAwarded(bounty_id, beneficiary));
		}

		/// Claim the payout from an awarded bounty after payout delay.
		///
		/// The dispatch origin for this call must be the beneficiary of this bounty.
		///
		/// - `bounty_id`: Bounty ID to claim.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::claim_bounty()]
		fn claim_bounty(origin, #[compact] bounty_id: BountyIndex) {
			let _ = ensure_signed(origin)?; // anyone can trigger claim

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let bounty = maybe_bounty.take().ok_or(Error::<T>::InvalidIndex)?;
				if let BountyStatus::PendingPayout { curator, beneficiary, unlock_at } = bounty.status {
					ensure!(system::Module::<T>::block_number() >= unlock_at, Error::<T>::Premature);
					// Get bounty account id
					let bounty_account = Self::bounty_account_id(bounty_id);
					let balance = T::Currency::free_balance(&bounty_account);
					let fee = bounty.fee.min(balance); // just to be safe

					// Make curator fee payment & unreserve the deposit
					let _ = T::Currency::unreserve(&curator, bounty.curator_deposit);
					let _ = T::Currency::transfer(
						&bounty_account,
						&curator,
						fee,
						AllowDeath
					); // should not fail

					// Make beneficiary payment
					let payout = balance.saturating_sub(fee);
					let _ = T::Currency::transfer(
						&bounty_account,
						&beneficiary,
						payout,
						AllowDeath
					); // should not fail

					// State Clean-up
					BountyDescriptions::remove(bounty_id);
					*maybe_bounty = None;
					// Trigger Event
					Self::deposit_event(Event::<T>::BountyClaimed(bounty_id, payout, beneficiary));
					Ok(())
				} else {
					Err(Error::<T>::UnexpectedStatus.into())
				}
			})?;
		}

		/// Cancel a proposed or active bounty. All the funds will be sent to treasury and
		/// the curator deposit will be unreserved if possible.
		///
		/// Only `T::RejectOrigin` is able to cancel a bounty.
		///
		/// - `bounty_id`: Bounty ID to cancel.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::close_bounty_proposed().max(<T as Config>::WeightInfo::close_bounty_active())]
		fn close_bounty(origin, #[compact] bounty_id: BountyIndex) -> DispatchResultWithPostInfo {
			T::RejectOrigin::ensure_origin(origin)?;


			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResultWithPostInfo {
				let bounty = maybe_bounty.as_ref().ok_or(Error::<T>::InvalidIndex)?;

				// Ensure no active subbounties before processing the call.
				ensure!(bounty.active_subbounty_count == 0, Error::<T>::SubBountyActive);

				match &bounty.status {
					BountyStatus::Proposed => {
						// The reject origin would like to cancel a proposed bounty.
						BountyDescriptions::remove(bounty_id);
						let value = bounty.bond;
						let imbalance = T::Currency::slash_reserved(&bounty.proposer, value).0;
						T::OnSlash::on_unbalanced(imbalance);
						*maybe_bounty = None;

						Self::deposit_event(Event::<T>::BountyRejected(bounty_id, value));
						// Return early, nothing else to do.
						return Ok(Some(<T as Config>::WeightInfo::close_bounty_proposed()).into())
					},
					BountyStatus::Approved => {
						// For weight reasons, we don't allow a council to cancel in this phase.
						// We ask for them to wait until it is funded before they can cancel.
						return Err(Error::<T>::UnexpectedStatus.into())
					},
					BountyStatus::Funded |
					BountyStatus::CuratorProposed { .. } => {
						// Nothing extra to do besides the removal of the bounty below.
					},
					BountyStatus::Active { curator, .. } => {
						// Cancelled by council, refund deposit of the working curator.
						let _ = T::Currency::unreserve(&curator, bounty.curator_deposit);
						// Then execute removal of the bounty below.
					},
					BountyStatus::PendingPayout { .. } => {
						// Bounty is already pending payout. If council wants to cancel
						// this bounty, it should mean the curator was acting maliciously.
						// So the council should first unassign the curator, slashing their
						// deposit.
						return Err(Error::<T>::PendingPayout.into())
					},
				}

				let bounty_account = Self::bounty_account_id(bounty_id);
				BountyDescriptions::remove(bounty_id);

				let balance = T::Currency::free_balance(&bounty_account);
				let _ = T::Currency::transfer(
					&bounty_account,
					&Self::account_id(),
					balance,
					AllowDeath
				); // should not fail
				*maybe_bounty = None;

				Self::deposit_event(Event::<T>::BountyCanceled(bounty_id));
				Ok(Some(<T as Config>::WeightInfo::close_bounty_active()).into())
			})
		}

		/// Extend the expiry time of an active bounty.
		///
		/// The dispatch origin for this call must be the curator of this bounty.
		///
		/// - `bounty_id`: Bounty ID to extend.
		/// - `remark`: additional information.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::extend_bounty_expiry()]
		fn extend_bounty_expiry(origin, #[compact] bounty_id: BountyIndex, remark: Vec<u8>) {
			let signer = ensure_signed(origin)?;

			Bounties::<T>::try_mutate_exists(bounty_id, |maybe_bounty| -> DispatchResult {
				let bounty = maybe_bounty.as_mut().ok_or(Error::<T>::InvalidIndex)?;

				match bounty.status {
					BountyStatus::Active { ref curator, ref mut update_due } => {
						ensure!(*curator == signer, Error::<T>::RequireCurator);
						*update_due = (system::Module::<T>::block_number() + T::BountyUpdatePeriod::get()).max(*update_due);
					},
					_ => return Err(Error::<T>::UnexpectedStatus.into()),
				}
				Ok(())
			})?;

			Self::deposit_event(Event::<T>::BountyExtended(bounty_id));
		}

		/// Add a new subbounty.
		///
		/// The dispatch origin for this call must be curator.
		/// Bounty must me in "active" state.
		///
		/// Subbouty gets added successfully & fund gets reserved, if bounty has enough fund.
		/// else call get failed.
		///
		/// Upperbount to maximum number of subbounties that can be added is
		/// managed via runtime trait config 'MaxActiveSubBountyCount'.
		///
		/// Payment: `TipReportDepositBase` will be reserved from the origin account, as well as
		/// `DataDepositPerByte` for each byte in `reason`. It will be unreserved upon approval,
		/// or slashed when rejected.
		///
		/// if call is success, state of subbounty is moved to "Approved" state.
		/// And later moved to "Funded" state as part of "spend_fund()" callback.
		///
		/// - `bounty_id`: Bounty ID for which subbounty to be added.
		/// - `value`: Value for executing the proposal.
		/// - `description`: Text description for the subbounty.
		#[weight = 10_000]
		fn add_subbounty(
			origin,
			#[compact] bounty_id: BountyIndex,
			value: BalanceOf<T>,
			description: Vec<u8>,
		) {
			let signer = ensure_signed(origin)?;

			Bounties::<T>::try_mutate_exists(
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let bounty = maybe_bounty
						.as_mut()
						.ok_or(Error::<T>::InvalidIndex)?;

					if let BountyStatus::Active { ref curator, .. } = bounty.status {
						ensure!(signer == *curator, Error::<T>::RequireCurator);

						// Verify the arguments
						ensure!(
							description.len() <= T::MaximumReasonLength::get() as usize,
							Error::<T>::ReasonTooBig,
						);
						ensure!(
							value >= T::BountyValueMinimum::get(),
							Error::<T>::InvalidValue,
						);
						ensure!(
							bounty.active_subbounty_count <
								T::MaxActiveSubBountyCount::get() as u32,
							Error::<T>::TooManySubBounties,
						);

						// Makesure Parent bounty have enough balance to fund Subbounty
						let bounty_account = Self::bounty_account_id(bounty_id);
						let balance = T::Currency::free_balance(&bounty_account);

						// minimum balance expected on bounty account
						// ensure master curator fee is considered in calculation
						let expect_balance = value.saturating_add(bounty.fee);
						let expect_free_balance = balance.checked_sub(&expect_balance)
							.ok_or(Error::<T>::InsufficientBountyBalance)?;

						T::Currency::ensure_can_withdraw(
							&bounty_account,
							expect_balance,
							WithdrawReasons::TRANSFER,
							expect_free_balance,
						).map_err(|_| Error::<T>::InsufficientBountyBalance)?;

						// Use bounty counter to generate
						// subbounty id
						let subbounty_id = Self::bounty_count();

						// Increment the active subbounty count.
						bounty.active_subbounty_count += 1;

						BountyCount::put(subbounty_id + 1);

						// Transfer fund from parent bounty to subbounty.
						let subbounty_account = Self::bounty_account_id(subbounty_id);
						let _ = T::Currency::transfer(
							&bounty_account,
							&subbounty_account,
							value,
							AllowDeath
						); // should not fail

						// Create subbounty instance
						Self::create_subbounty(
							bounty_id,
							subbounty_id,
							description,
							value
						);
						Ok(())
					} else {
						Err(Error::<T>::UnexpectedStatus.into())
					}
				}
			)?;
		}

		/// Propose subcurator for funded subbounty.
		///
		/// The dispatch origin for this call must be master curator.
		///
		/// Parent bounty must be in active state,
		/// for this subbounty call to work.
		///
		/// Proposed subcurator may be "master curator",
		/// and subcurator fee & curator deposit
		/// may be Zero in this case.
		///
		/// Subbounty must be in "Funded" state, for
		/// processing the call. and state of subbounty is
		/// moved to CuratorProposed on successful call
		/// completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		/// - `subcurator`: Address of subcurator.
		/// - `fee`: payment fee to subcurator for execution.
		#[weight = 10_000]
		fn propose_subcurator(origin, #[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
			subcurator: <T::Lookup as StaticLookup>::Source,
			#[compact] fee: BalanceOf<T>,
		) {
			let signer = ensure_signed(origin)?;
			let subcurator = T::Lookup::lookup(subcurator)?;

			// Ensure parent bounty is Active & get status of curator
			let (master_curator, _) = Self::ensure_bounty_active(bounty_id)?;

			// Mutate the Subbounty instance
			SubBounties::<T>::try_mutate_exists(
				bounty_id,
				subbounty_id,
				|maybe_subbounty| -> DispatchResult {

					let mut subbounty = maybe_subbounty
						.as_mut()
						.ok_or(Error::<T>::InvalidIndex)?;

					// Ensure sure caller is curator
					ensure!(signer == master_curator, Error::<T>::RequireCurator);

					// Ensure subbounty is in expected state
					ensure!(
						subbounty.status == SubBountyStatus::Added,
						Error::<T>::UnexpectedStatus
					);

					// Ensure subcurator fee is less than subbounty value.
					ensure!(fee < subbounty.value, Error::<T>::InvalidFee);

					// Update the master curator fee balance.
					Bounties::<T>::mutate_exists(
						bounty_id,
						|maybe_bounty| -> DispatchResult {
							if let Some(bounty) = maybe_bounty.as_mut() {
								// Ensure subcurator fee is less than
								// master curator fee balance
								ensure!(fee < bounty.fee, Error::<T>::InvalidFee);
								// Reduce the master curator fee balance.
								bounty.fee = bounty.fee.saturating_sub(fee);
							}
							Ok(())
						}
					)?;

					// Update the subcurator fee.
					subbounty.fee = fee;

					// update the subbounty state
					subbounty.status = SubBountyStatus::SubCuratorProposed {
						subcurator: subcurator
					};
					Ok(())
				}
			)?;
		}

		/// Accept the subcurator role for the subbounty.
		///
		/// A deposit will be reserved from subcurator and
		/// refund upon successful payout or cancellation.
		/// In case if "master curator" is "subcurator",
		/// curator deposit is Zero or ignored.
		///
		/// The dispatch origin for this call must be
		/// the subcurator of this subbounty.
		///
		/// Parent bounty must be in active state,
		/// for this subbounty call to work.
		///
		/// Subbounty must be in "CuratorProposed" state, for
		/// processing the call. and state of subbounty is
		/// moved to "Active" on successful call
		/// completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		#[weight = 10_000]
		fn accept_subcurator(origin,
			#[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
		) {
			let signer = ensure_signed(origin)?;

			// Ensure parent bounty is Active & get status of curator
			let (_, _) = Self::ensure_bounty_active(bounty_id)?;

			// Mutate Subbounty
			SubBounties::<T>::try_mutate_exists(bounty_id, subbounty_id,
				|maybe_subbounty| -> DispatchResult {

				let mut subbounty = maybe_subbounty
					.as_mut()
					.ok_or(Error::<T>::InvalidIndex)?;

				// Ensure subbounty is in expected state
				match subbounty.status {
					SubBountyStatus::SubCuratorProposed { ref subcurator } => {
						ensure!(signer == *subcurator, Error::<T>::RequireSubCurator);

						// Reserve subcurator deposit
						let deposit = T::BountyCuratorDeposit::get() * subbounty.fee;
						T::Currency::reserve(subcurator, deposit)?;
						subbounty.curator_deposit = deposit;

						subbounty.status = SubBountyStatus::Active {
							subcurator: subcurator.clone(),
						};
					},
					_ => return Err(Error::<T>::UnexpectedStatus.into()),
				};
				Ok(())
			})?;
		}

		/// Unassign subcurator from a subbounty.
		///
		/// The dispatch origin for this call can be
		/// either `RejectOrigin` or any signed origin.
		///
		/// Parent bounty must be in active state,
		/// for this subbounty call to work.
		///
		/// If this function is called by the `RejectOrigin`, we assume that
		/// the curator is malicious or inactive. As a result,
		/// we will slash the curator when possible.
		///
		/// If the origin is the subcurator, we take this as a sign they are
		/// unable to do their job and they willingly give up.
		/// We could slash them, but for now we allow them to recover their
		/// deposit and exit without issue. (We may want to change this
		/// if it is abused.)
		///
		/// Finally, the origin can be anyone if and only if the subcurator
		/// is "inactive". This allows anyone in the community to call out
		/// that a subcurator is not doing their due diligence, and
		/// we should pick a new subcurator. In this case the subcurator
		/// should also be slashed.
		///
		/// State of subbounty is moved to Funded state
		/// on successful call completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		#[weight = 10_000]
		fn unassign_subcurator(
			origin,
			#[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
		) {
			let maybe_sender = ensure_signed(origin.clone())
				.map(Some)
				.or_else(|_| T::RejectOrigin::ensure_origin(origin).map(|_| None))?;

			// Ensure parent bounty is Active & get status of curator
			let (master_curator, update_due) = Self::ensure_bounty_active(bounty_id)?;

			// Ensure subbounty is in expected state
			SubBounties::<T>::try_mutate_exists(
				bounty_id,
				subbounty_id,
				|maybe_subbounty| -> DispatchResult {

					let mut subbounty = maybe_subbounty
						.as_mut()
						.ok_or(Error::<T>::InvalidIndex)?;

					let slash_curator = |arg_curator: &T::AccountId,
						curator_deposit: &mut BalanceOf<T>| {
							let imbalance = T::Currency::slash_reserved(
								arg_curator,
								*curator_deposit,
							).0;
							T::OnSlash::on_unbalanced(imbalance);
							*curator_deposit = Zero::zero();
						};

					match subbounty.status {
						SubBountyStatus::Added => {
							// No curator to unassign at this point.
							return Err(Error::<T>::UnexpectedStatus.into())
						}
						SubBountyStatus::SubCuratorProposed { ref subcurator } => {
							// A subcurator has been proposed, but not accepted yet.
							// Either `RejectOrigin`, curator or the proposed subcurator
							// can unassign the subcurator.
							ensure!(
								maybe_sender.map_or(
									true,
									|sender| sender == *subcurator || sender == master_curator
								),
								BadOrigin,
							);
						},
						SubBountyStatus::Active { ref subcurator } => {
							// The bounty is active.
							match maybe_sender {
								// If the `RejectOrigin` is calling this function,
								// slash the subcurator deposit.
								None => {
									slash_curator(subcurator, &mut subbounty.curator_deposit);
									// Continue to change bounty status below...
								},
								Some(sender) => {
									if sender == *subcurator {
										// This is the subcurator,
										// willingly giving up their role.
										// Give back their deposit.
										T::Currency::unreserve(
											&subcurator,
											subbounty.curator_deposit,
										);
										// Continue to change bounty status below...
									} else if sender == master_curator {
										// looks like subcurator is inactive,
										// slash the subcurator deposit.
										slash_curator(subcurator, &mut subbounty.curator_deposit);
										// Continue to change bounty status below...
									} else {
										// check for expiry
										// looks like subcurator is inactive,
										// slash the subcurator deposit.
										let block_number = system::Module::<T>::block_number();
										if update_due < block_number {
											slash_curator(
												subcurator,
												&mut subbounty.curator_deposit,
											);
											// Continue to change bounty status below...
										} else {
											// Curator has more time to give an update.
											return Err(Error::<T>::Premature.into())
										}
									}
								},
							}
						},
						SubBountyStatus::PendingPayout { ref subcurator, .. } => {
							// TODO :: Have to recheck
							// The subbounty is in pending payout state,
							// only Root or Master curator origin can unassign.
							// By doing so, they are claiming the subcurator
							// is acting maliciously, so We slash the subcurator.
							ensure!(
								maybe_sender.map_or(
									true,
									|sender| sender == master_curator
								),
								BadOrigin,
							);
							slash_curator(subcurator, &mut subbounty.curator_deposit);
							// Continue to change bounty status below...
						},
					};
					// Move the subbounty state to Added.
					subbounty.status = SubBountyStatus::Added;
					Ok(())
				}
			)?;
		}

		/// Award subbounty to a beneficiary.
		///
		/// The beneficiary will be able to claim the
		/// funds after a delay.
		///
		/// The dispatch origin for this call must be
		/// the master curator or subcurator of this subbounty.
		///
		/// Parent bounty must be in active state,
		/// for this subbounty call to work.
		///
		/// Subbounty must be in active state, for
		/// processing the call. and state of subbounty is
		/// moved to PendingPayout on successful call
		/// completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		/// - `beneficiary`: Beneficiary account.
		#[weight = 10_000]
		fn award_subbounty(origin,
			#[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
			beneficiary: <T::Lookup as StaticLookup>::Source
		) {
			let signer = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			// Ensure parent bounty is Active
			let (_master_curator, _) = Self::ensure_bounty_active(bounty_id)?;

			// Ensure subbounty is in expected state
			SubBounties::<T>::try_mutate_exists(
				bounty_id,
				subbounty_id,
				|maybe_subbounty| -> DispatchResult {
					let mut subbounty = maybe_subbounty
						.as_mut()
						.ok_or(Error::<T>::InvalidIndex)?;

					// Ensure Subbounty is in active state
					match &subbounty.status {
						SubBountyStatus::Active {
							subcurator,
							..
						} => {
							// Only Subcurator can award the subbounty.
							ensure!(
								signer == *subcurator,
								Error::<T>::RequireSubCurator,
							);
						},
						_ => return Err(Error::<T>::UnexpectedStatus.into()),
					}
					// Move the subbounty state to Pending payout.
					subbounty.status = SubBountyStatus::PendingPayout {
						subcurator: signer,
						beneficiary: beneficiary.clone(),
						unlock_at: system::Module::<T>::block_number() +
							T::BountyDepositPayoutDelay::get(),
					};
					Ok(())
				}
			)?;
			// Trigger the event SubBountyAwarded
			Self::deposit_event(Event::<T>::SubBountyAwarded(bounty_id, subbounty_id, beneficiary));
		}

		/// Claim the payout from an awarded subbounty after payout delay.
		///
		/// The dispatch origin for this call may be any signed origin.
		///
		/// Call works independent of parent bounty state,
		/// No need for parent bounty must be in active state.
		///
		/// Beneficiary is paid out with with agreed bounty value.
		/// SubCurator fee is paid & bond deposit is unreserved.
		///
		/// Subbounty must be in PendingPayout state, for
		/// processing the call. and instance of subbounty is
		/// deallocated from DB on successful call completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		#[weight = 10_000]
		fn claim_subbounty(origin,
			#[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
		) {
			let _ = ensure_signed(origin)?;

			// TODO :: Have to recheck
			// ignoring the requirement of parent bounty should be active
			// for claiming the subbounty.
			// Since subbounty is executed & in waiting period of PendingPayout.
			// We can gracefully execute this call, without having dependency on
			// state of parent bounty, This enables to call close_subbounty()
			// recursively from close_bounty() without any issue.
			// // Ensure parent bounty is Active
			// let master_curator = Self::ensure_bounty_active(bounty_id)?;

			// Ensure subbounty is in expected state
			SubBounties::<T>::try_mutate_exists(
				bounty_id,
				subbounty_id,
				|maybe_subbounty| -> DispatchResult {
					let subbounty = maybe_subbounty
						.as_mut()
						.ok_or(Error::<T>::InvalidIndex)?;

					if let SubBountyStatus::PendingPayout {
						ref subcurator, ref beneficiary, ref unlock_at
					} = subbounty.status {
						// Ensure block number is elapsed for
						// processing the claim.
						ensure!(
							system::Module::<T>::block_number() >= *unlock_at,
							Error::<T>::Premature,
						);

						// Make curator fee payment
						let subbounty_account = Self::bounty_account_id(subbounty_id);
						let balance = T::Currency::free_balance(&subbounty_account);
						let fee = subbounty.fee.min(balance); // just to be safe
						let payout = balance.saturating_sub(fee);

						// unreserve the subcurator deposit
						let _ = T::Currency::unreserve(
							&subcurator,
							subbounty.curator_deposit,
						); // should not fail

						// Make payout to subcurator
						let _ = T::Currency::transfer(
							&subbounty_account,
							&subcurator,
							fee,
							AllowDeath,
						); // should not fail

						// Make payout to beneficiary
						let _ = T::Currency::transfer(
							&subbounty_account,
							beneficiary,
							payout,
							AllowDeath,
						); // should not fail

						// Trigger the SubBountyClaimed event
						Self::deposit_event(
							Event::<T>::SubBountyClaimed(
								bounty_id,
								subbounty_id,
								payout,
								beneficiary.clone(),
							)
						);

						// Remove the subbounty from bounty active subbouty list
						Bounties::<T>::mutate_exists(
							bounty_id,
							|maybe_bounty| -> DispatchResult {
								// Remove the subbounty index from parent bounty
								// active list.
								if let Some(bounty) = maybe_bounty.as_mut() {
									bounty.active_subbounty_count -= 1;
								}
								Ok(())
							}
						)?;
						// Remove the subbounty description
						BountyDescriptions::remove(subbounty_id);
						// Remove the subbounty instance
						// from DB
						*maybe_subbounty = None;
						Ok(())
					} else {
						Err(Error::<T>::UnexpectedStatus.into())
					}
				}
			)?;
		}

		/// Cancel a proposed or active subbounty. All the reserved funds
		/// gets unreserved to parent bounty account. the curator deposit
		/// will be unreserved if possible.
		///
		/// The dispatch origin for this call must be
		/// either `T::RejectOrigin` or master curator of this subbounty.
		///
		/// If state of subbounty is `Active`,
		/// subcurator deposit is unreserved.
		///
		/// If state of subbounty is `PendingPayout`,
		/// call fails & returns PendingPayout error.
		///
		/// Parent bounty must be in active state,
		/// for this subbounty call to work.
		///
		/// Instance of subbounty is deallocated from DB
		/// on successful call completion.
		///
		/// - `bounty_id`: ID pair Bounty ID.
		/// - `subbounty_id`: ID pair SubBounty ID to cancel.
		#[weight = 10_000]
		fn close_subbounty(origin,
			#[compact] bounty_id: BountyIndex,
			#[compact] subbounty_id: BountyIndex,
		) -> DispatchResultWithPostInfo {
			let maybe_sender = ensure_signed(origin.clone())
				.map(Some)
				.or_else(|_| T::RejectOrigin::ensure_origin(origin).map(|_| None))?;

			// Ensure parent bounty is Active
			let (master_curator, _) = Self::ensure_bounty_active(bounty_id)?;

			// Either `RejectOrigin` or the master curator can close subbounty.
			ensure!(
				maybe_sender.map_or(true, |sender| sender == master_curator),
				BadOrigin
			);
			// Call the internal implementation.
			Self::impl_close_subbounty(bounty_id, subbounty_id)?;

			Ok(Some(<T as Config>::WeightInfo::close_bounty_active()).into())
		}
	}
}

impl<T: Config> Module<T> {
	// Add public immutables and private mutables.

	/// The account ID of the treasury pot.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	/// The account ID of a bounty account
	pub fn bounty_account_id(id: BountyIndex) -> T::AccountId {
		// only use two byte prefix to support 16 byte account id (used by test)
		// "modl" ++ "py/trsry" ++ "bt" is 14 bytes, and two bytes remaining for bounty index
		T::ModuleId::get().into_sub_account(("bt", id))
	}

	fn create_bounty(
		proposer: T::AccountId,
		description: Vec<u8>,
		value: BalanceOf<T>,
	) -> DispatchResult {
		ensure!(description.len() <= T::MaximumReasonLength::get() as usize, Error::<T>::ReasonTooBig);
		ensure!(value >= T::BountyValueMinimum::get(), Error::<T>::InvalidValue);

		let index = Self::bounty_count();

		// reserve deposit for new bounty
		let bond = T::BountyDepositBase::get()
			+ T::DataDepositPerByte::get() * (description.len() as u32).into();
		T::Currency::reserve(&proposer, bond)
			.map_err(|_| Error::<T>::InsufficientProposersBalance)?;

		BountyCount::put(index + 1);

		let bounty = Bounty {
			proposer,
			value,
			fee: 0u32.into(),
			curator_deposit: 0u32.into(),
			bond,
			status: BountyStatus::Proposed,
			active_subbounty_count: 0u32.into(),
		};

		Bounties::<T>::insert(index, &bounty);
		BountyDescriptions::insert(index, description);

		Self::deposit_event(RawEvent::BountyProposed(index));

		Ok(())
	}

	fn ensure_bounty_active(
		bounty_id: BountyIndex,
	) -> Result<(T::AccountId, T::BlockNumber), DispatchError> {
		let bounty = Self::bounties(&bounty_id).ok_or(Error::<T>::InvalidIndex)?;
		if let BountyStatus::Active { curator, update_due } = bounty.status {
			Ok((curator, update_due))
		} else {
			Err(Error::<T>::UnexpectedStatus.into())
		}
	}

	fn create_subbounty(
		bounty_id: BountyIndex,
		subbounty_id: BountyIndex,
		description: Vec<u8>,
		value: BalanceOf<T>,
	) {

		let subbounty = SubBounty {
			value,
			fee: 0u32.into(),
			curator_deposit: 0u32.into(),
			status: SubBountyStatus::Added,
		};

		SubBounties::<T>::insert(bounty_id, subbounty_id, &subbounty);
		BountyDescriptions::insert(subbounty_id, description);
		Self::deposit_event(RawEvent::SubBountyAdded(bounty_id, subbounty_id));
	}

	fn impl_close_subbounty(
		bounty_id: BountyIndex,
		subbounty_id: BountyIndex,
	) -> DispatchResult {
		SubBounties::<T>::try_mutate_exists(
			bounty_id,
			subbounty_id,
			|maybe_subbounty| -> DispatchResult {

				let subbounty = maybe_subbounty
					.as_mut()
					.ok_or(Error::<T>::InvalidIndex)?;

				match &subbounty.status {
					SubBountyStatus::Added |
					SubBountyStatus::SubCuratorProposed { .. } => {
						// Nothing extra to do besides the removal of the bounty below.
					},
					SubBountyStatus::Active { subcurator } => {
						// Cancelled by council, refund deposit of the working curator.
						let _ = T::Currency::unreserve(subcurator, subbounty.curator_deposit);
						// Then execute removal of the bounty below.
					},
					SubBountyStatus::PendingPayout { .. } => {
						// Bounty is already pending payout. If council wants to cancel
						// this bounty, it should mean the curator was acting maliciously.
						// So the council should first unassign the curator, slashing their
						// deposit.
						return Err(Error::<T>::PendingPayout.into())
					},
				}

				// Update the master curator fee &
				// Reduce the active subbounty count.
				Bounties::<T>::mutate_exists(
					bounty_id,
					|maybe_bounty| {
						if let Some(bounty) = maybe_bounty.as_mut() {
							bounty.fee = bounty
								.fee
								.saturating_add(subbounty.fee);
							bounty.active_subbounty_count -= 1;
						}
					}
				);

				// Transfer fund from subbounty to parent bounty.
				let bounty_account = Self::bounty_account_id(bounty_id);
				let subbounty_account = Self::bounty_account_id(subbounty_id);
				let balance = T::Currency::free_balance(&subbounty_account);
				let _ = T::Currency::transfer(
					&subbounty_account,
					&bounty_account,
					balance,
					AllowDeath
				); // should not fail

				// Remove the subbounty description
				BountyDescriptions::remove(subbounty_id);
				*maybe_subbounty = None;

				Self::deposit_event(
					Event::<T>::SubBountyCanceled(bounty_id, subbounty_id),
				);
				Ok(())
			}
		)
	}
}

impl<T: Config> pallet_treasury::SpendFunds<T> for Module<T> {
	fn spend_funds(
		budget_remaining: &mut BalanceOf<T>,
		imbalance: &mut PositiveImbalanceOf<T>,
		total_weight: &mut Weight,
		missed_any: &mut bool
	) {
		let bounties_len = BountyApprovals::mutate(|v| {
			let bounties_approval_len = v.len() as u32;
			v.retain(|&index| {
				Bounties::<T>::mutate(index, |bounty| {
					// Should always be true, but shouldn't panic if false or we're screwed.
					if let Some(bounty) = bounty {
						if bounty.value <= *budget_remaining {
							*budget_remaining -= bounty.value;

							bounty.status = BountyStatus::Funded;

							// return their deposit.
							let _ = T::Currency::unreserve(&bounty.proposer, bounty.bond);

							// fund the bounty account
							imbalance.subsume(
								T::Currency::deposit_creating(
									&Self::bounty_account_id(index),
									bounty.value
								)
							);

							Self::deposit_event(RawEvent::BountyBecameActive(index));
							false
						} else {
							*missed_any = true;
							true
						}
					} else {
						false
					}
				})
			});
			bounties_approval_len
		});

		*total_weight += <T as Config>::WeightInfo::spend_funds(bounties_len);
	}
}
