/// ! Traits and default implementation for paying transaction fees.
use crate::Config;
use frame_system::EventRecord;
use pallet_treasury::BalanceOf;

pub trait FeeCalculator<T: Config> {
    fn match_events(
        events: Vec<
            EventRecord<<T as frame_system::Config>::Event, <T as frame_system::Config>::Hash>,
        >,
    ) -> BalanceOf<T>;
}
