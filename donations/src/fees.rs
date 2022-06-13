/// ! Traits and default implementation for paying transaction fees.
use crate::Config;

pub trait FeeCalculator<T: Config> {
	fn match_event(
		event: pallet_balances::Event<T>,
		curr_block_fee_sum: &mut <T as Config>::Balance,
	);
}
