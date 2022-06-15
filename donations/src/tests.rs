use crate::mock::*;
use frame_support::assert_ok;
use frame_support::traits::OffchainWorker;
use frame_support::traits::{OnFinalize, OnInitialize};

use sp_runtime::offchain::storage::StorageValue;

const DB_KEY_SUM: &[u8] = b"donations/txn-fee-sum";

fn run_to_block(n: u64) {
    while System::block_number() < n {
        Donations::on_finalize(System::block_number());
        Donations::offchain_worker(System::block_number());
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        Donations::on_initialize(System::block_number());
    }
}

#[test]
fn test_transfer_txn_updates_offchain_variable() {
    let (t, _) = &mut new_test_ext_with_offchain_worker();

    t.execute_with(|| {
        assert_eq!(Balances::free_balance(1), 100);
        assert_eq!(Balances::free_balance(2), 200);

        assert_ok!(Balances::transfer(Origin::signed(1), 2, 50));

        assert_eq!(Balances::free_balance(1), 50);
        assert_eq!(Balances::free_balance(2), 250);

        run_to_block(2);

        let val = StorageValue::persistent(&DB_KEY_SUM);
        let sum = val.get::<<Test as crate::Config>::Balance>();

        assert_eq!(sum, Ok(Some(16_u64)));
    })
}
