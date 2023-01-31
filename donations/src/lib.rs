// Copyright 2022 Sequester Developer.
// This file is part of Sequester.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// -------------------------------------------------------------------------------------------
//                                    Donations Pallet
// -------------------------------------------------------------------------------------------
// This pallet calculates transaction fees using an off-chain worker in order
// to send a percentage of them to Sequester, where they will be donated.
//
// This pallet uses an off-chain worker to calculate these fees in order to minimize
// the on-chain computation to accomplish this task. An alternative technical implementation
// would be to allocate funds to a "donations" account using a DealWithFees struct
// passed into pallet_transaction_payment. However, this approach would use on-chain
// computation for every transaction on the chain, which is unneccessary, since all
// transaction fees donated will be going to a single location.
//
// In order to calculate txn fees, we use a multi step process:
//
// 1) An off-chain worker will run each block. Since fees are customizable on substrate chains,
// each chain will need custom logic to sum the txn fees. Thus, each block, an offchain
// worker will iterate through on-chain events, where txn fees will be parsed on a per-event basis. At the end of each block,
// the sum of txn fees caculated will be added to an off-chain variable storing cumulative txn
// fees on-chain
//
// 2) Every OnChainUpdateInterval blocks, the offchain worker will submit an unsigned transaction to
// record the pending txn fees in on-chain storage, reset the off-chain cumulative fee variable,
// and send a TxnFeeQueued event.
//
// 3) Every SpendPeriod (var for pallet_treasury) blocks, the treasury will call the SpendFunds
// trait, which will check the on-chain storage for queued txn fees. If txn fees are queued,
// they will be subsumed into a special Sequester account, and an XCM will be constructed sending
// the queued funds to the Sequester chain.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get, Imbalance},
        weights::{PostDispatchInfo, Weight},
        PalletId,
    };
    use frame_system::{
        offchain::{SendTransactionTypes, SubmitTransaction},
        pallet_prelude::*,
        RawOrigin,
    };
    use pallet_transaction_payment::{Event::TransactionFeePaid, OnChargeTransaction};
    use pallet_treasury::{BalanceOf, PositiveImbalanceOf};
    use sp_runtime::{
        offchain::{
            storage::StorageValueRef,
            storage_lock::{StorageLock, Time},
        },
        traits::{AccountIdConversion, Convert, Saturating, Zero},
        Percent,
    };

    use sp_std::{boxed::Box, vec};
    use xcm::latest::prelude::*;
    use xcm_executor::traits::WeightBounds;

    const DB_KEY_SUM: &[u8] = b"donations/txn-fee-sum";
    const DB_LOCK: &[u8] = b"donations/txn-sum-lock";

    pub const BASE_CREATE_GAS: Weight = Weight::zero();

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_transaction_payment::Config
        + pallet_treasury::Config
        + pallet_xcm::Config
        + SendTransactionTypes<Call<Self>>
    {
        // The standard event type
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        // Type used to convert generic frame events into the
        // event type specifically emitted by the balances pallet
        type TransactionFeeEvent: From<<Self as frame_system::Config>::Event>
            + TryInto<pallet_transaction_payment::Event<Self>>;

        type BalanceConverter: From<<<Self as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<Self>>::Balance>
			+ Into<BalanceOf<Self>>;

        // A standard AccountIdToMultiLocation converter
        type AccountIdToMultiLocation: Convert<Self::AccountId, MultiLocation>;

        // Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        // weight of an xcm transaction to send to sequester
        #[pallet::constant]
        type SequesterTransferMinimum: Get<BalanceOf<Self>>;

        // The priority of the unsigned transactions submitted by
        // the Sequester pallet
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        // Interval (in blocks) at which we store the pending fees and reset the
        // txn-fee-sum variable
        #[pallet::constant]
        type OnChainUpdateInterval: Get<Self::BlockNumber>;

        // The percentage of transaction fees that you would like to send
        // from your chain’s treasury to the Sequester chain
        #[pallet::constant]
        type TxnFeePercentage: Get<Percent>;

        // The MultiLocation representing where the funds will be going
        // e.g (sequester kusama chain or sequester polkadot chain)
        #[pallet::constant]
        type SequesterMultiLocation: Get<MultiLocation>;

        // The MultiLocation representing what funds you are sending
        // from your treasury
        #[pallet::constant]
        type TokenToSendMultiLocation: Get<MultiLocation>;
    }

    // The next block where an unsigned transaction will be considered valid
    #[pallet::type_value]
    pub(super) fn DefaultNextUnsigned<T: Config>() -> T::BlockNumber {
        T::BlockNumber::from(0u32)
    }
    #[pallet::storage]
    #[pallet::getter(fn next_unsigned_at)]
    pub(super) type NextUnsignedAt<T: Config> =
        StorageValue<_, T::BlockNumber, ValueQuery, DefaultNextUnsigned<T>>;

    #[pallet::storage]
    #[pallet::getter(fn fees_to_send)]
    pub(super) type FeesToSend<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Transaction fee sum from offchain worker has been processed
        /// and stored on chain
        TxnFeeQueued(BalanceOf<T>),
        /// Transaction fee sum has been withdrawn from treasury account
        /// to Sequester account
        TxnFeeSubsumed(BalanceOf<T>),
        /// Transaction fee has successfully been sent from chain
        /// to Sequester
        SequesterTransferSuccess(BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidXCMExtrinsicCall,
        FeeConvertFailed,
        XcmExecutionFailed,
        UnweighableMessage,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Each block, initiate an offchain worker to summarize the txn fees for that block,
        // and append that amount to a counter in local storage, which we will empty
        // when it is time to send the txn fees to Sequester.
        fn offchain_worker(block_number: T::BlockNumber) {
            let mut block_fee_sum = Self::calculate_fees_for_block();

            let percent_to_send = T::TxnFeePercentage::get();

            block_fee_sum = percent_to_send * block_fee_sum;

            Self::update_offchain_storage(block_fee_sum);

            // send fees to sequester
            if (block_number % T::OnChainUpdateInterval::get()).is_zero() {
                Self::queue_pending_txn_fees_onchain(block_number);
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get the account ID of the Sequester account.
        /// Treasury funds that will be sent to Sequester will be
        /// temporarily stored in this account
        pub fn get_sequester_account_id() -> T::AccountId {
            let seq_pallet_id: PalletId = PalletId(*b"py/sqstr");
            seq_pallet_id.into_account_truncating()
        }

        /// Calculate the fees for a given block
        fn calculate_fees_for_block() -> BalanceOf<T> {
            let events = <frame_system::Pallet<T>>::read_events_no_consensus();

            let filtered_events = events.into_iter().filter_map(|event_record| {
                let balances_event = <T as Config>::TransactionFeeEvent::from(event_record.event);
                balances_event.try_into().ok()
            });

            let mut curr_block_fee_sum: BalanceOf<T> = Zero::zero();
            for filtered_event in filtered_events {
                match filtered_event {
                    TransactionFeePaid {
                        who: _,
                        actual_fee,
                        tip: _,
                    } => {
                        let converted_fee = <T as Config>::BalanceConverter::from(actual_fee);
                        curr_block_fee_sum =
                            (curr_block_fee_sum).saturating_add(converted_fee.into());
                    }
                    _ => {}
                }
            }

            curr_block_fee_sum
        }

        /// Updates the offchain storage variable tracking cumulative txn fees by safely adding
        /// the fees calculated in a given block to the running amount
        fn update_offchain_storage(block_fee_sum: BalanceOf<T>) {
            // Use get/set instead of mutation to guarantee that we don't
            // hit any MutateStorageError::ConcurrentModification errors
            let mut lock = StorageLock::<Time>::new(&DB_LOCK);
            {
                let _guard = lock.lock();
                let val = StorageValueRef::persistent(&DB_KEY_SUM);
                match val.get::<BalanceOf<T>>() {
                    // initialize value
                    Ok(None) => {
                        val.set(&block_fee_sum);
                    }
                    // update value
                    Ok(Some(fetched_txn_fee_sum)) => {
                        val.set(&fetched_txn_fee_sum.saturating_add(block_fee_sum));
                    }
                    _ => {}
                };
            }
        }

        /// Takes all of the pending transaction fees which have been stored in the offchain
        /// variable and submits an unsigned transaction to store that queue on chain. Once the
        /// value is queued, it will be sent the the Sequester chain the next time that the
        /// SpendFunds callback is initiated in the Treasury pallet
        fn queue_pending_txn_fees_onchain(block_num: T::BlockNumber) {
            // get lock so that another ocw doesn't modify the value mid-send
            let mut lock = StorageLock::<Time>::new(&DB_LOCK);
            {
                let _guard = lock.lock();
                let val = StorageValueRef::persistent(&DB_KEY_SUM);
                let fees_to_send = val.get::<BalanceOf<T>>();
                match fees_to_send {
                    Ok(Some(fetched_fees)) => {
                        let call = Call::<T>::submit_unsigned {
                            amount: fetched_fees,
                            block_num,
                        };
                        let txn_res = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
                            call.into(),
                        );
                        match txn_res {
                            Ok(_) => {
                                let zero_bal: BalanceOf<T> = Zero::zero();
                                val.set(&zero_bal);
                            }
                            Err(_) => {}
                        }
                    }
                    _ => {}
                };
            }
        }
    }

    /// Validation logic from unsigned transactions. In order to make sure that malicious txns
    /// can't get through, we discard any Transactions not from a local OCW and make sure that only
    /// 1 txn can be submitted every OnChainUpdateInterval blocks
    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_unsigned { block_num, .. } = call {
                // Discard solution not coming from the local OCW.
                match source {
                    TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ }
                    _ => return InvalidTransaction::Call.into(),
                }

                // Reject outdated txns
                let next_unsigned_at = Self::next_unsigned_at();
                if &next_unsigned_at > block_num {
                    return InvalidTransaction::Stale.into();
                }
                // Reject txns from the future
                let current_block = <frame_system::Pallet<T>>::block_number();
                if &current_block < block_num {
                    return InvalidTransaction::Future.into();
                }

                ValidTransaction::with_tag_prefix("Donations")
                    .priority(T::UnsignedPriority::get())
                    // We don't propagate this. This can never be validated at a remote node.
                    .propagate(false)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }

    /// SpendFunds implementation. Since the transaction fees we will be sending were originally sent to the
    /// treasury, we use the SpendFunds callback to pull funds out of the treasury and into the Sequester account,
    /// which we will use to send funds via XCM
    impl<T: Config> pallet_treasury::SpendFunds<T> for Pallet<T> {
        fn spend_funds(
            budget_remaining: &mut BalanceOf<T>,
            imbalance: &mut PositiveImbalanceOf<T>,
            total_weight: &mut Weight,
            _missed_any: &mut bool,
        ) {
            let fees_to_send = Self::fees_to_send();

            let transfer_fee = T::SequesterTransferMinimum::get();

            // valid fees to send
            if fees_to_send > transfer_fee && *budget_remaining >= fees_to_send {
                *budget_remaining -= fees_to_send;

                let sequester_acc = Self::get_sequester_account_id();

                imbalance.subsume(T::Currency::deposit_creating(&sequester_acc, fees_to_send));
                // reset fee counter
                let zero_bal: BalanceOf<T> = Zero::zero();
                FeesToSend::<T>::set(zero_bal);
                Self::deposit_event(Event::TxnFeeSubsumed(fees_to_send));

                let sequester_bal = T::Currency::free_balance(&Self::get_sequester_account_id());

                let _ = Self::xcm_transfer_to_sequester(
                    RawOrigin::Signed(sequester_acc).into(),
                    sequester_bal,
                );
            }
            *total_weight += <T as Config>::WeightInfo::spend_funds();
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// The extrinsic for submitting an unsigned transaction. Stores the fees_to_send on-chain
        /// and emits a TxnFeeQueued event
        #[pallet::weight(<T as Config>::WeightInfo::submit_unsigned())]
        pub fn submit_unsigned(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            block_num: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            // update storage to reject unsigned transactions until OnChainUpdateInterval blocks pass
            <NextUnsignedAt<T>>::put(block_num + T::OnChainUpdateInterval::get());

            let pending_fees = Self::fees_to_send();

            // add pending XCM transfer to Sequester here
            FeesToSend::<T>::set(pending_fees.saturating_add(amount.into()));

            Self::deposit_event(Event::TxnFeeQueued(amount));
            Ok(PostDispatchInfo {
                actual_weight: Some(BASE_CREATE_GAS),
                pays_fee: Pays::No,
            })
        }

        /// The extrinsic for sending funds to sequester via an XCM call. The present XCM call is a placeholder
        /// until the Sequester chains have been built and design architecture has been finalized.
        #[pallet::weight(<T as Config>::WeightInfo::xcm_transfer_to_sequester())]
        pub fn xcm_transfer_to_sequester(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin.clone())?;

            let fee = T::SequesterTransferMinimum::get();

            frame_support::ensure!(amount >= fee, Error::<T>::InvalidXCMExtrinsicCall);

            let send_amount_u128 =
                TryInto::<u128>::try_into(amount).map_err(|_| Error::<T>::FeeConvertFailed)?;

            let dest = Box::new(xcm::VersionedMultiLocation::V1(
                T::SequesterMultiLocation::get(),
            ));

            let sequester_acc = Self::get_sequester_account_id();

            let beneficiary = Box::new(xcm::VersionedMultiLocation::V1(
                T::AccountIdToMultiLocation::convert(sequester_acc.clone()),
            ));

            let assets = Box::new(
                vec![MultiAsset {
                    id: AssetId::Concrete(T::TokenToSendMultiLocation::get()),
                    fun: Fungibility::Fungible(send_amount_u128),
                }]
                .into(),
            );

            let result = 
                <pallet_xcm::Pallet<T>>::reserve_transfer_assets(dest, beneficiary, assets, 0);

            match result {
                Err(err) => log::warn!("Failed to teleport assets: {:?}", err),
                _ => Self::deposit_event(Event::SequesterTransferSuccess(amount)),
            }

            Ok(PostDispatchInfo {
                actual_weight: Some(BASE_CREATE_GAS),
                pays_fee: Pays::No,
            })
        }
    }
}
