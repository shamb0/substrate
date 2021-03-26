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

//! # Tipping Module ( pallet-tips )
//!
//! > NOTE: This pallet is tightly coupled with pallet-treasury.
//!
//! A subsystem to allow for an agile "tipping" process, whereby a reward may be given without first
//! having a pre-determined stakeholder group come to consensus on how much should be paid.
//!
//! A group of `Tippers` is determined through the config `Config`. After half of these have declared
//! some amount that they believe a particular reported reason deserves, then a countdown period is
//! entered where any remaining members can declare their tip amounts also. After the close of the
//! countdown period, the median of all declared tips is paid to the reported beneficiary, along
//! with any finders fee, in case of a public (and bonded) original report.
//!
//!
//! ### Terminology
//!
//! Tipping protocol:
//! - **Tipping:** The process of gathering declarations of amounts to tip and taking the median
//!   amount to be transferred from the treasury to a beneficiary account.
//! - **Tip Reason:** The reason for a tip; generally a URL which embodies or explains why a
//!   particular individual (identified by an account ID) is worthy of a recognition by the
//!   treasury.
//! - **Finder:** The original public reporter of some reason for tipping.
//! - **Finders Fee:** Some proportion of the tip amount that is paid to the reporter of the tip,
//!   rather than the main beneficiary.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! Tipping protocol:
//! - `report_awesome` - Report something worthy of a tip and register for a finders fee.
//! - `retract_tip` - Retract a previous (finders fee registered) report.
//! - `tip_new` - Report an item worthy of a tip and declare a specific amount to tip.
//! - `tip` - Declare or redeclare an amount to tip for a particular reason.
//! - `close_tip` - Close and pay out a tip.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;
mod benchmarking;
pub mod weights;

use sp_std::prelude::*;

use frame_support::{
    Parameter,
    storage::migration::{move_storage_from_pallet},
    traits::{
        Currency, Get, ExistenceRequirement::{KeepAlive},
        ReservableCurrency, Contains, ContainsLengthBound,
        OnUnbalanced, PalletInfo,
    },
};
use frame_system::{self as system};
use sp_runtime::{ Percent, RuntimeDebug, traits::{
    Zero, AccountIdConversion, Hash, BadOrigin
}};
#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;

use codec::{Encode, Decode};
pub use weights::WeightInfo;

pub use pallet::*;

pub type BalanceOf<T> = pallet_treasury::BalanceOf<T>;
pub type NegativeImbalanceOf<T> = pallet_treasury::NegativeImbalanceOf<T>;

/// An open tipping "motion". Retains all details of a tip including information on the finder
/// and the members who have voted.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub struct OpenTip<
    AccountId: Parameter,
    Balance: Parameter,
    BlockNumber: Parameter,
    Hash: Parameter,
> {
    /// The hash of the reason for the tip. The reason should be a human-readable UTF-8 encoded string. A URL would be
    /// sensible.
    reason: Hash,
    /// The account to be tipped.
    who: AccountId,
    /// The account who began this tip.
    finder: AccountId,
    /// The amount held on deposit for this tip.
    deposit: Balance,
    /// The block number at which this tip will close if `Some`. If `None`, then no closing is
    /// scheduled.
    closes: Option<BlockNumber>,
    /// The members who have voted for this tip. Sorted by AccountId.
    tips: Vec<(AccountId, Balance)>,
    /// Whether this tip should result in the finder taking a fee.
    finders_fee: bool,
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_treasury::Config {

        /// Maximum acceptable reason length.
        type MaximumReasonLength: Get<u32>;

        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The amount held on deposit per byte within the tip report reason or bounty description.
        type DataDepositPerByte: Get<BalanceOf<Self>>;

        /// Origin from which tippers must come.
        ///
        /// `ContainsLengthBound::max_len` must be cost free (i.e. no storage read or heavy operation).
        type Tippers: Contains<Self::AccountId> + ContainsLengthBound;

        /// The period for which a tip remains open after is has achieved threshold tippers.
        type TipCountdown: Get<Self::BlockNumber>;

        /// The percent of the final tip which goes to the original reporter of the tip.
        type TipFindersFee: Get<Percent>;

        /// The amount held on deposit for placing a tip report.
        type TipReportDepositBase: Get<BalanceOf<Self>>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {

        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if !UpgradedToTripleRefCount::<T>::get() {
                UpgradedToTripleRefCount::<T>::put(true);
                migrations::migrate_to_triple_ref_count::<T>()
            } else {
                0
            }
        }

        fn integrity_test() {
            T::BlockWeights::get()
                .validate()
                .expect("The weights are invalid.");
        }

    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Report something `reason` that deserves a tip and claim any eventual the finder's fee.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// Payment: `TipReportDepositBase` will be reserved from the origin account, as well as
        /// `DataDepositPerByte` for each byte in `reason`.
        ///
        /// - `reason`: The reason for, or the thing that deserves, the tip; generally this will be
        ///   a UTF-8-encoded URL.
        /// - `who`: The account which should be credited for the tip.
        ///
        /// Emits `NewTip` if successful.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::report_awesome(reason.len() as u32))]
        pub fn report_awesome(
            origin: OriginFor<T>,
            reason: Vec<u8>,
            who: T::AccountId
        ) -> DispatchResultWithPostInfo {
            let finder = ensure_signed(origin)?;

            ensure!(
                reason.len() <= T::MaximumReasonLength::get() as usize,
                Error::<T>::ReasonTooBig
            );

            let reason_hash = T::Hashing::hash(&reason[..]);
            ensure!(
                !<Reasons<T>>::contains_key(&reason_hash),
                Error::<T>::AlreadyKnown
            );

            let hash = T::Hashing::hash_of(&(&reason_hash, &who));
            ensure!(
                !<Tips<T>>::contains_key(&hash),
                Error::<T>::AlreadyKnown
            );

            let deposit = T::TipReportDepositBase::get()
                + T::DataDepositPerByte::get() * (reason.len() as u32).into();
            T::Currency::reserve(&finder, deposit)?;

            <Reasons<T>>::insert(&reason_hash, reason);
            let tip = OpenTip {
                reason: reason_hash,
                who,
                finder,
                deposit,
                closes: None,
                tips: vec![],
                finders_fee: true
            };
            <Tips<T>>::insert(&hash, tip);
            Self::deposit_event(Event::NewTip(hash));
            Ok(().into())
        }

        /// Retract a prior tip-report from `report_awesome`, and cancel the process of tipping.
        ///
        /// If successful, the original deposit will be unreserved.
        ///
        /// The dispatch origin for this call must be _Signed_ and the tip identified by `hash`
        /// must have been reported by the signing account through `report_awesome` (and not
        /// through `tip_new`).
        ///
        /// - `hash`: The identity of the open tip for which a tip value is declared. This is formed
        ///   as the hash of the tuple of the original tip `reason` and the beneficiary account ID.
        ///
        /// Emits `TipRetracted` if successful.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::retract_tip())]
        pub fn retract_tip(
            origin: OriginFor<T>,
            hash: T::Hash
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let tip = <Tips<T>>::get(&hash).ok_or(Error::<T>::UnknownTip)?;
            ensure!(tip.finder == who, Error::<T>::NotFinder);

            <Reasons<T>>::remove(&tip.reason);
            <Tips<T>>::remove(&hash);
            if !tip.deposit.is_zero() {
                let err_amount = T::Currency::unreserve(&who, tip.deposit);
                debug_assert!(err_amount.is_zero());
            }
            Self::deposit_event(Event::TipRetracted(hash));
            Ok(().into())
        }

        /// Give a tip for something new; no finder's fee will be taken.
        ///
        /// The dispatch origin for this call must be _Signed_ and the signing account must be a
        /// member of the `Tippers` set.
        ///
        /// - `reason`: The reason for, or the thing that deserves, the tip; generally this will be
        ///   a UTF-8-encoded URL.
        /// - `who`: The account which should be credited for the tip.
        /// - `tip_value`: The amount of tip that the sender would like to give. The median tip
        ///   value of active tippers will be given to the `who`.
        ///
        /// Emits `NewTip` if successful.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::tip_new(reason.len() as u32, T::Tippers::max_len() as u32))]
        pub fn tip_new(
            origin: OriginFor<T>,
            reason: Vec<u8>,
            who: T::AccountId,
            #[pallet::compact] tip_value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let tipper = ensure_signed(origin)?;
            ensure!(T::Tippers::contains(&tipper), BadOrigin);

            let reason_hash = T::Hashing::hash(&reason[..]);
            ensure!(
                !Reasons::<T>::contains_key(&reason_hash),
                Error::<T>::AlreadyKnown,
            );
            let hash = T::Hashing::hash_of(&(&reason_hash, &who));

            <Reasons<T>>::insert(&reason_hash, reason);
            Self::deposit_event(Event::NewTip(hash.clone()));
            let tips = vec![(tipper.clone(), tip_value)];
            let tip = OpenTip {
                reason: reason_hash,
                who,
                finder: tipper,
                deposit: Zero::zero(),
                closes: None,
                tips,
                finders_fee: false,
            };
            <Tips<T>>::insert(&hash, tip);
            Ok(().into())
        }

        /// Declare a tip value for an already-open tip.
        ///
        /// The dispatch origin for this call must be _Signed_ and the signing account must be a
        /// member of the `Tippers` set.
        ///
        /// - `hash`: The identity of the open tip for which a tip value is declared. This is formed
        ///   as the hash of the tuple of the hash of the original tip `reason` and the beneficiary
        ///   account ID.
        /// - `tip_value`: The amount of tip that the sender would like to give. The median tip
        ///   value of active tippers will be given to the `who`.
        ///
        /// Emits `TipClosing` if the threshold of tippers has been reached and the countdown period
        /// has started.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::tip(T::Tippers::max_len() as u32))]
        pub fn tip(
            origin: OriginFor<T>,
            hash: T::Hash,
            #[pallet::compact] tip_value: BalanceOf<T>
        ) -> DispatchResultWithPostInfo {
            let tipper = ensure_signed(origin)?;
            ensure!(T::Tippers::contains(&tipper), BadOrigin);

            let mut tip = <Tips<T>>::get(hash).ok_or(Error::<T>::UnknownTip)?;
            if Self::insert_tip_and_check_closing(&mut tip, tipper, tip_value) {
                Self::deposit_event(Event::TipClosing(hash.clone()));
            }
            <Tips<T>>::insert(&hash, tip);
            Ok(().into())
        }

        /// Close and payout a tip.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// The tip identified by `hash` must have finished its countdown period.
        ///
        /// - `hash`: The identity of the open tip for which a tip value is declared. This is formed
        ///   as the hash of the tuple of the original tip `reason` and the beneficiary account ID.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::close_tip(T::Tippers::max_len() as u32))]
        pub fn close_tip(
            origin: OriginFor<T>,
            hash: T::Hash
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            let tip = <Tips<T>>::get(hash).ok_or(Error::<T>::UnknownTip)?;
            let n = tip.closes.as_ref().ok_or(Error::<T>::StillOpen)?;
            ensure!(
                // system::Module::<T>::block_number() >= *n,
                system::Pallet::<T>::block_number() >= *n,
                Error::<T>::Premature
            );
            // closed.
            <Reasons<T>>::remove(&tip.reason);
            <Tips<T>>::remove(hash);
            Self::payout_tip(hash, tip);
            Ok(().into())
        }

        /// Remove and slash an already-open tip.
        ///
        /// May only be called from `T::RejectOrigin`.
        ///
        /// As a result, the finder is slashed and the deposits are lost.
        ///
        /// Emits `TipSlashed` if successful.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::slash_tip(T::Tippers::max_len() as u32))]
        pub fn slash_tip(
            origin: OriginFor<T>,
            hash: T::Hash
        ) -> DispatchResultWithPostInfo {
            T::RejectOrigin::ensure_origin(origin)?;

            let tip = <Tips<T>>::take(hash).ok_or(Error::<T>::UnknownTip)?;

            if !tip.deposit.is_zero() {
                let imbalance = T::Currency::slash_reserved(&tip.finder, tip.deposit).0;
                T::OnSlash::on_unbalanced(imbalance);
            }
            <Reasons<T>>::remove(&tip.reason);
            Self::deposit_event(Event::TipSlashed(hash, tip.finder, tip.deposit));
            Ok(().into())
        }

    }

    #[pallet::error]
    pub enum Error<T> {
        /// The reason given is just too big.
        ReasonTooBig,
        /// The tip was already found/started.
        AlreadyKnown,
        /// The tip hash is unknown.
        UnknownTip,
        /// The account attempting to retract the tip is not the finder of the tip.
        NotFinder,
        /// The tip cannot be claimed/closed because there are not enough tippers yet.
        StillOpen,
        /// The tip cannot be claimed/closed because it's still in the countdown period.
        Premature,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance", T::Hash = "Hash")]
    pub enum Event<T: Config> {
        /// A new tip suggestion has been opened. \[tip_hash\]
        NewTip(T::Hash),
        /// A tip suggestion has reached threshold and is closing. \[tip_hash\]
        TipClosing(T::Hash),
        /// A tip suggestion has been closed. \[tip_hash, who, payout\]
        TipClosed(T::Hash, T::AccountId, BalanceOf<T>),
        /// A tip suggestion has been retracted. \[tip_hash\]
        TipRetracted(T::Hash),
        /// A tip suggestion has been slashed. \[tip_hash, finder, deposit\]
        TipSlashed(T::Hash, T::AccountId, BalanceOf<T>),
    }

    #[pallet::storage]
    #[pallet::getter(fn tips)]
    pub type Tips<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::Hash,
        OpenTip<T::AccountId, BalanceOf<T>, T::BlockNumber, T::Hash>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn reasons)]
    pub type Reasons<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::Hash,
        Vec<u8>,
        OptionQuery,
    >;

    /// True if we have upgraded so that AccountInfo contains three types of `RefCount`. False
    /// (default) if not.
    #[pallet::storage]
    pub(super) type UpgradedToTripleRefCount<T: Config> = StorageValue<_, bool, ValueQuery>;

    // TODO :: Have to recheck
    // Since each pallet is has own storage
    // Tips is expected to have own storage & not
    // Share with Treasury.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config>{
        pub phantom: sp_std::marker::PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <UpgradedToTripleRefCount<T>>::put(false);
        }
    }
}

mod migrations {
    use super::*;

    /// Migrate from dual `u32` reference counting to triple `u32` reference counting.
    pub fn migrate_to_triple_ref_count<T: Config>() -> frame_support::weights::Weight {

        let new_name = T::PalletInfo::name::<Pallet<T>>()
            .expect("Fatal Error Invalid PalletInfo name");
        move_storage_from_pallet(b"Tips", b"Treasury", new_name.as_bytes());
        move_storage_from_pallet(b"Reasons", b"Treasury", new_name.as_bytes());

        T::BlockWeights::get().max_block
    }
}

#[cfg(feature = "std")]
impl<T: Config> GenesisConfig<T> {
    /// Direct implementation of `GenesisBuild::build_storage`.
    ///
    /// Kept in order not to break dependency.
    pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
        <Self as GenesisBuild<T>>::build_storage(self)
    }

    /// Direct implementation of `GenesisBuild::assimilate_storage`.
    ///
    /// Kept in order not to break dependency.
    pub fn assimilate_storage(
        &self,
        storage: &mut sp_runtime::Storage
    ) -> Result<(), String> {
        <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
    }
}

impl<T: Config>  Pallet<T> {
    /// The account ID of the treasury pot.
    ///
    /// This actually does computation. If you need to keep using it, then make sure you cache the
    /// value and only call this once.
    pub fn account_id() -> T::AccountId {
        T::ModuleId::get().into_account()
    }

    /// Given a mutable reference to an `OpenTip`, insert the tip into it and check whether it
    /// closes, if so, then deposit the relevant event and set closing accordingly.
    ///
    /// `O(T)` and one storage access.
    fn insert_tip_and_check_closing(
        tip: &mut OpenTip<T::AccountId, BalanceOf<T>, T::BlockNumber, T::Hash>,
        tipper: T::AccountId,
        tip_value: BalanceOf<T>,
    ) -> bool {
        match tip.tips.binary_search_by_key(&&tipper, |x| &x.0) {
            Ok(pos) => tip.tips[pos] = (tipper, tip_value),
            Err(pos) => tip.tips.insert(pos, (tipper, tip_value)),
        }
        Self::retain_active_tips(&mut tip.tips);
        let threshold = (T::Tippers::count() + 1) / 2;
        if tip.tips.len() >= threshold && tip.closes.is_none() {
            tip.closes = Some(system::Pallet::<T>::block_number() + T::TipCountdown::get());
            true
        } else {
            false
        }
    }

    /// Remove any non-members of `Tippers` from a `tips` vector. `O(T)`.
    fn retain_active_tips(tips: &mut Vec<(T::AccountId, BalanceOf<T>)>) {
        let members = T::Tippers::sorted_members();
        let mut members_iter = members.iter();
        let mut member = members_iter.next();
        tips.retain(|(ref a, _)| loop {
            match member {
                None => break false,
                Some(m) if m > a => break false,
                Some(m) => {
                    member = members_iter.next();
                    if m < a {
                        continue
                    } else {
                        break true;
                    }
                }
            }
        });
    }

    /// Execute the payout of a tip.
    ///
    /// Up to three balance operations.
    /// Plus `O(T)` (`T` is Tippers length).
    fn payout_tip(hash: T::Hash, tip: OpenTip<T::AccountId, BalanceOf<T>, T::BlockNumber, T::Hash>) {
        let mut tips = tip.tips;
        Self::retain_active_tips(&mut tips);
        tips.sort_by_key(|i| i.1);

        let treasury = Self::account_id();
        // let max_payout = pallet_treasury::Module::<T>::pot();
        let max_payout = pallet_treasury::Pallet::<T>::pot();

        let mut payout = tips[tips.len() / 2].1.min(max_payout);
        if !tip.deposit.is_zero() {
            let err_amount = T::Currency::unreserve(&tip.finder, tip.deposit);
            debug_assert!(err_amount.is_zero());
        }

        if tip.finders_fee && tip.finder != tip.who {
            // pay out the finder's fee.
            let finders_fee = T::TipFindersFee::get() * payout;
            payout -= finders_fee;
            // this should go through given we checked it's at most the free balance, but still
            // we only make a best-effort.
            let res = T::Currency::transfer(&treasury, &tip.finder, finders_fee, KeepAlive);
            debug_assert!(res.is_ok());
        }

        // same as above: best-effort only.
        let res = T::Currency::transfer(&treasury, &tip.who, payout, KeepAlive);
        debug_assert!(res.is_ok());
        Self::deposit_event(Event::TipClosed(hash, tip.who, payout));
    }

    pub fn migrate_retract_tip_for_tip_new() {
        /// An open tipping "motion". Retains all details of a tip including information on the finder
        /// and the members who have voted.
        #[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
        pub struct OldOpenTip<
            AccountId: Parameter,
            Balance: Parameter,
            BlockNumber: Parameter,
            Hash: Parameter,
        > {
            /// The hash of the reason for the tip. The reason should be a human-readable UTF-8 encoded string. A URL would be
            /// sensible.
            reason: Hash,
            /// The account to be tipped.
            who: AccountId,
            /// The account who began this tip and the amount held on deposit.
            finder: Option<(AccountId, Balance)>,
            /// The block number at which this tip will close if `Some`. If `None`, then no closing is
            /// scheduled.
            closes: Option<BlockNumber>,
            /// The members who have voted for this tip. Sorted by AccountId.
            tips: Vec<(AccountId, Balance)>,
        }

        use frame_support::{Twox64Concat, migration::StorageKeyIterator};

        for (hash, old_tip) in StorageKeyIterator::<
            T::Hash,
            OldOpenTip<T::AccountId, BalanceOf<T>, T::BlockNumber, T::Hash>,
            Twox64Concat,
        >::new(b"Tips", b"Tips").drain()
        {
            let (finder, deposit, finders_fee) = match old_tip.finder {
                Some((finder, deposit)) => {
                    (finder, deposit, true)
                },
                None => {
                    (T::AccountId::default(), Zero::zero(), false)
                },
            };
            let new_tip = OpenTip {
                reason: old_tip.reason,
                who: old_tip.who,
                finder,
                deposit,
                closes: old_tip.closes,
                tips: old_tip.tips,
                finders_fee
            };
            Tips::<T>::insert(hash, new_tip)
        }
    }
}
