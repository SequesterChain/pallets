//! Benchmarking setup for pallet-donations

use super::*;

#[allow(unused)]
use crate::Pallet as Donations;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::EventRecord;
use frame_system::RawOrigin;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    submit_unsigned {
        let s in 0 .. 100;
        let block = T::BlockNumber::from(1u32);
        let caller: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Signed(caller), s.into(), block)
    verify {
        assert_last_event::<T>(Event::TxnFeeQueued(s.into()).into())
    }

    xcm_transfer_to_sequester {
        let s in 0 .. 100;
        let caller: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Signed(caller), s.into())
    verify {
        // todo
    }

    impl_benchmark_test_suite!(Donations, crate::mock::new_test_ext(), crate::mock::Test);
}
