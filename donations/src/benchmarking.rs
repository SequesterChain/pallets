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

//! Benchmarking setup for pallet-donations

use super::*;

use crate::Pallet as Donations;
use frame_benchmarking::{benchmarks, whitelisted_caller, Zero};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::{Currency, Imbalance};
use frame_system::EventRecord;
use frame_system::RawOrigin;

use pallet_treasury::BalanceOf;

use sp_runtime::traits::UniqueSaturatedInto;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn setup_pot_account<T: Config>() {
    let treasury_acc = pallet_treasury::Pallet::<T>::account_id();
    let _ =
        T::Currency::make_free_balance_be(&treasury_acc, 100_000_000u64.unique_saturated_into());

    let fees_to_send: BalanceOf<T> = 100_000u64.unique_saturated_into();
    FeesToSend::<T>::set(fees_to_send);
}

benchmarks! {
    submit_unsigned {
        let block = T::BlockNumber::from(0u32);
        let amount = 100_000_000u32.unique_saturated_into();
    }: _(RawOrigin::None, amount, block)
    verify {
        assert_last_event::<T>(Event::TxnFeeQueued(amount).into())
    }

    xcm_transfer_to_sequester {
        let caller: T::AccountId = whitelisted_caller();
        let amount = 100_000_000u32.unique_saturated_into();
        T::Currency::make_free_balance_be(&caller, amount);
    }: _(RawOrigin::Signed(caller), amount)
    verify {
        assert_last_event::<T>(Event::SequesterTransferSuccess(amount).into())
    }

    spend_funds {
        setup_pot_account::<T>();

        let mut budget_remaining = 100_000_000u64.unique_saturated_into();
        let mut imbalance = pallet_treasury::PositiveImbalanceOf::<T>::zero();
        let mut total_weight = Weight::zero();
        let mut missed_any = false;
    }: {
        <Donations<T> as pallet_treasury::SpendFunds<T>>::spend_funds(
            &mut budget_remaining,
            &mut imbalance,
            &mut total_weight,
            &mut missed_any,
        );
    }

    impl_benchmark_test_suite!(Donations, crate::mock::new_test_ext(), crate::mock::Test);
}
