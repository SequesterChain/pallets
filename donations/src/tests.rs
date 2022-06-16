use core::marker::PhantomData;

use crate::mock::*;
use frame_support::assert_ok;
use frame_support::traits::OffchainWorker;
use frame_support::traits::{OnFinalize, OnInitialize};
use pallet_transaction_payment::{ChargeTransactionPayment, CurrencyAdapter, Multiplier};
use sp_runtime::traits::SignedExtension;

use pallet_treasury::BalanceOf;
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
        assert_eq!(Balances::free_balance(1), ACC_BAL_1);
        assert_eq!(Balances::free_balance(2), ACC_BAL_2);

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        let pre = ChargeTransactionPayment::<Test>::from(0u64.into())
            .pre_dispatch(&FROM_ACCOUNT, CALL, &dispatch_info, len)
            .unwrap();

        assert!(ChargeTransactionPayment::<Test>::post_dispatch(
            Some(pre),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        let TXN_AMOUNT = 50;

        assert_ok!(Balances::transfer(Origin::signed(1), 2, TXN_AMOUNT));

        let new_bal_1 = ACC_BAL_1 - TXN_AMOUNT - TXN_FEE;
        let new_bal_2 = ACC_BAL_2 + TXN_AMOUNT;

        assert_eq!(Balances::free_balance(FROM_ACCOUNT), new_bal_1);
        assert_eq!(Balances::free_balance(TO_ACCOUNT), new_bal_2);

        run_to_block(5);

        let val = StorageValue::persistent(&DB_KEY_SUM);
        let sum = val.get::<BalanceOf<Test>>();

        assert_eq!(sum, Ok(Some(16_u64)));
    })
}
