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

use crate::mock::*;

use frame_support::error::BadOrigin;
use frame_support::traits::OffchainWorker;
use frame_support::traits::{Currency, OnFinalize, OnInitialize};
use frame_support::{assert_err, assert_ok};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

use pallet_treasury::BalanceOf;
use sp_runtime::offchain::storage::StorageValue;

const DB_KEY_SUM: &[u8] = b"donations/txn-fee-sum";

fn run_to_block(n: u64) {
    while System::block_number() < n {
        Donations::on_finalize(System::block_number());
        Donations::offchain_worker(System::block_number());
        System::on_finalize(System::block_number());
        System::on_initialize(System::block_number());
        Donations::on_initialize(System::block_number());
        Treasury::on_initialize(System::block_number());
        System::set_block_number(System::block_number() + 1);
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

        assert_ok!(Balances::transfer(Origin::signed(1), 2, TXN_AMOUNT));
        assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| a.borrow().clone()), TXN_FEE);

        run_to_block(2);

        let new_bal_1 = ACC_BAL_1 - TXN_AMOUNT - TXN_FEE;
        let new_bal_2 = ACC_BAL_2 + TXN_AMOUNT;

        assert_eq!(Balances::free_balance(FROM_ACCOUNT), new_bal_1);
        assert_eq!(Balances::free_balance(TO_ACCOUNT), new_bal_2);

        let val = StorageValue::persistent(&DB_KEY_SUM);
        let sum = val.get::<BalanceOf<Test>>();

        // multiply by 10% to get "fees_to_send"
        let fees_to_send = TXN_FEE as f64 * SEND_PERCENTAGE;

        assert_eq!(sum, Ok(Some(fees_to_send as u64)));

        run_to_block(10);

        // offchain variable should be reset to 0 after
        // send interval blocks

        let reset_sum = val.get::<BalanceOf<Test>>();
        assert_eq!(reset_sum, Ok(Some(0 as u64)));
    })
}

#[test]
fn test_submit_unsigned_updates_on_chain_vars() {
    let (t, _) = &mut new_test_ext_with_offchain_worker();
    t.execute_with(|| {
        assert_eq!(Donations::next_unsigned_at(), 0);
        assert_eq!(Donations::fees_to_send(), 0);

        let collected_fees = (TXN_FEE as f64 * SEND_PERCENTAGE) as u64;

        assert_ok!(Donations::submit_unsigned(
            Origin::none(),
            collected_fees,
            1
        ));

        run_to_block(2);

        // make sure on chain vars are properly updated
        assert_eq!(Donations::next_unsigned_at(), 1 + SEND_INTERVAL);
        assert_eq!(Donations::fees_to_send(), collected_fees);

        let seq_account_id = Donations::get_sequester_account_id();

        Balances::make_free_balance_be(&Treasury::account_id(), collected_fees + 10);
        Balances::make_free_balance_be(&seq_account_id, 0);

        // collected_fees + 10 to set Treasury::pot equal to collected_fees
        assert_eq!(
            Balances::free_balance(&Treasury::account_id()),
            collected_fees + 10
        );
        assert_eq!(Treasury::pot(), collected_fees);
        assert_eq!(Balances::free_balance(&seq_account_id), 0);

        // make sure treasury spend_funds is correctly called and desired effect happens
        run_to_block(4);

        assert_eq!(Treasury::pot(), 0);

        assert_eq!(Balances::free_balance(&seq_account_id), collected_fees);
    })
}

#[test]
fn test_invalid_unsigned_origin() {
    assert_err!(
        Donations::submit_unsigned(Origin::signed(1), TXN_FEE, 1),
        BadOrigin
    );
    assert_err!(
        Donations::submit_unsigned(Origin::root(), TXN_FEE, 1),
        BadOrigin
    );
}

#[test]
fn test_send_zero_doesnt_fail() {
    let (t, _) = &mut new_test_ext_with_offchain_worker();

    t.execute_with(|| {
        let val = StorageValue::persistent(&DB_KEY_SUM);
        let start_storage_var = val.get::<BalanceOf<Test>>();

        assert_eq!(start_storage_var, Ok(None));

        run_to_block(2);

        let middle_storage_var = val.get::<BalanceOf<Test>>();
        assert_eq!(middle_storage_var, Ok(Some(0 as u64)));

        run_to_block(10);

        // offchain variable should be reset to 0 after
        // send interval blocks

        let reset_sum = val.get::<BalanceOf<Test>>();
        assert_eq!(reset_sum, Ok(Some(0 as u64)));
    })
}

#[test]
fn test_multiple_txns_single_block() {
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

        assert_ok!(Balances::transfer(Origin::signed(1), 2, TXN_AMOUNT));
        assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| a.borrow().clone()), TXN_FEE);

        let pre_2 = ChargeTransactionPayment::<Test>::from(0u64.into())
            .pre_dispatch(&FROM_ACCOUNT, CALL, &dispatch_info, len)
            .unwrap();

        assert!(ChargeTransactionPayment::<Test>::post_dispatch(
            Some(pre_2),
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());

        assert_ok!(Balances::transfer(Origin::signed(1), 2, TXN_AMOUNT));
        assert_eq!(
            FEE_UNBALANCED_AMOUNT.with(|a| a.borrow().clone()),
            2 * TXN_FEE
        );

        run_to_block(2);

        let new_bal_1 = ACC_BAL_1 - (2 * (TXN_AMOUNT + TXN_FEE));
        let new_bal_2 = ACC_BAL_2 + (2 * TXN_AMOUNT);

        assert_eq!(Balances::free_balance(FROM_ACCOUNT), new_bal_1);
        assert_eq!(Balances::free_balance(TO_ACCOUNT), new_bal_2);

        let val = StorageValue::persistent(&DB_KEY_SUM);
        let sum = val.get::<BalanceOf<Test>>();

        // multiply by 10% to get "fees_to_send"
        let fees_to_send = TXN_FEE as f64 * SEND_PERCENTAGE;

        assert_eq!(sum, Ok(Some(2 * fees_to_send as u64)));

        run_to_block(10);

        // offchain variable should be reset to 0 after
        // send interval blocks

        let reset_sum = val.get::<BalanceOf<Test>>();
        assert_eq!(reset_sum, Ok(Some(0 as u64)));
    })
}
