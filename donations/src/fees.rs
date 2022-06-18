// ! Traits and default implementation for paying transaction fees.
//
// Example implementation for chain sending fees to treasury may look as follows:
//
// pub struct TransactionFeeCalculator<S>(sp_std::marker::PhantomData<S>);
// impl<S> FeeCalculator<S> for TransactionFeeCalculator<S>
// where
// 	S: pallet_balances::Config + pallet_donations::Config,
// 	<S as frame_system::Config>::AccountId: From<AccountId>,
// 	<S as frame_system::Config>::AccountId: Into<AccountId>,
// 	BalanceOf<S>: From<<S as pallet_balances::Config>::Balance>,
// 	BalanceOf<S>: Into<<S as pallet_balances::Config>::Balance>,
// {
// 	fn match_events(
// 		events: Vec<
// 			EventRecord<<S as frame_system::Config>::Event, <S as frame_system::Config>::Hash>,
// 		>,
// 	) -> BalanceOf<S> {
// 		let mut curr_block_fee_sum: BalanceOf<S> = Zero::zero();

// 		let filtered_events = events.into_iter().filter_map(|event_record| {
// 			let balances_event =
// 				<S as pallet_donations::Config>::BalancesEvent::from(event_record.event);
// 			balances_event.try_into().ok()
// 		});

// 		for filtered_event in filtered_events {
// 			let treasury_id: AccountId = TreasuryPalletId::get().into_account();
// 			match filtered_event {
// 				<pallet_balances::Event<S>>::Deposit { who, amount } => {
// 					// If amount is deposited back into the account that paid for the transaction
// 					// fees during the same transaction, then deduct it from the txn fee counter as
// 					// a refund
// 					if who == treasury_id.into() {
// 						curr_block_fee_sum = (curr_block_fee_sum).saturating_add(amount.into());
// 					}
// 				},
// 				_ => {},
// 			}
// 		}
// 		curr_block_fee_sum
// 	}
// }

use crate::Config;
use frame_system::EventRecord;
use pallet_treasury::BalanceOf;
use sp_std::vec::Vec;
pub trait FeeCalculator<T: Config> {
    fn match_events(
        events: Vec<
            EventRecord<<T as frame_system::Config>::Event, <T as frame_system::Config>::Hash>,
        >,
    ) -> BalanceOf<T>;
}
