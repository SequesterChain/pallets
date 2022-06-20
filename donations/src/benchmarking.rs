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

#[allow(unused)]
use crate::Pallet as Donations;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::traits::Currency;
use frame_system::EventRecord;
use frame_system::RawOrigin;

use sp_runtime::traits::UniqueSaturatedInto;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    submit_unsigned {
        let s in 0 .. 100_000_000;
        let block = T::BlockNumber::from(0u32);
    }: _(RawOrigin::None, s.into(), block)
    verify {
        assert_last_event::<T>(Event::TxnFeeQueued(s.into()).into())
    }

    xcm_transfer_to_sequester {
        let s in 10_000_000 .. 100_000_000;
        let caller: T::AccountId = whitelisted_caller();
        T::Currency::make_free_balance_be(&caller, s.unique_saturated_into());
    }: _(RawOrigin::Signed(caller), s.into())
    verify {
        assert_last_event::<T>(Event::SequesterTransferSuccess(s.into()).into())
    }

    impl_benchmark_test_suite!(Donations, crate::mock::new_test_ext(), crate::mock::Test);
}
