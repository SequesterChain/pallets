use crate::{self as sequester_pallet, FeeCalculator};
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, EnsureOrigin, Everything, OriginTrait},
	weights::Weight,
	PalletId,
};
use frame_system as system;

use sp_runtime::traits::{AccountIdConversion, Convert};
use xcm_builder::{AllowUnpaidExecutionFrom, FixedWeightBounds};
use xcm_executor::{
	traits::{InvertLocation, TransactAsset, WeightTrader},
	Assets,
};

use sp_runtime::{traits::IdentityLookup, Percent};
use xcm::latest::{
	Error as XcmError, Junctions::X1, MultiAsset, MultiLocation, Result as XcmResult, SendResult,
	SendXcm, Xcm,
};

type AccountId = u64;
type AccountIndex = u32;
type BlockNumber = u64;
type Balance = u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>},
		TemplateModule: sequester_pallet::{Pallet, Call, Storage, Event<T>},
	}
);

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = AccountIndex;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = sp_core::H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = sp_runtime::testing::Header;
	type Event = Event;
	type BlockHashCount = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const UnsignedPriority: u64 = 99999999;
	pub const SendInterval: BlockNumber = 9;

	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 = 100;

	pub const BaseXcmWeight: Weight = 1000;
	pub const MaxInstructions: u32 = 100;

	pub const SequesterPalletId: PalletId = PalletId(*b"py/sqstr");
	pub const TxnFeePercentage: Percent = Percent::from_percent(10);
	pub DonationsXCMAccount: AccountId = SequesterPalletId::get().into_account();
	pub SequesterTransferWeight: Weight = 100000000000;
	pub SequesterTransferFee: Balance = 10000000;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<10>;
	type AccountStore = System;
	type WeightInfo = ();
}

pub type Extrinsic = sp_runtime::testing::TestXt<Call, ()>;

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	Call: From<C>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

impl pallet_treasury::Config for Test {
	type Currency = pallet_balances::Pallet<Test>;
	type ApproveOrigin = frame_system::EnsureRoot<AccountId>;
	type RejectOrigin = frame_system::EnsureRoot<AccountId>;
	type Event = Event;
	type OnSlash = ();
	type ProposalBond = ();
	type ProposalBondMinimum = ();
	type ProposalBondMaximum = ();
	type SpendPeriod = ();
	type Burn = ();
	type BurnDestination = ();
	type PalletId = TreasuryPalletId;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type WeightInfo = ();
}

pub struct ConvertOriginToLocal;
impl<Origin: OriginTrait> EnsureOrigin<Origin> for ConvertOriginToLocal {
	type Success = MultiLocation;

	fn try_origin(_: Origin) -> Result<MultiLocation, Origin> {
		Ok(MultiLocation::here())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> Origin {
		Origin::root()
	}
}

pub struct InvertNothing;
impl InvertLocation for InvertNothing {
	fn invert_location(_: &MultiLocation) -> sp_std::result::Result<MultiLocation, ()> {
		Ok(MultiLocation::here())
	}

	fn ancestry() -> MultiLocation {
		todo!()
	}
}

pub struct DoNothingRouter;
impl SendXcm for DoNothingRouter {
	fn send_xcm(_dest: impl Into<MultiLocation>, _msg: Xcm<()>) -> SendResult {
		Ok(())
	}
}

pub struct DummyAssetTransactor;
impl TransactAsset for DummyAssetTransactor {
	fn deposit_asset(_what: &MultiAsset, _who: &MultiLocation) -> XcmResult {
		Ok(())
	}

	fn withdraw_asset(_what: &MultiAsset, _who: &MultiLocation) -> Result<Assets, XcmError> {
		Ok(Assets::default())
	}
}

pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

pub struct DummyWeightTrader;
impl WeightTrader for DummyWeightTrader {
	fn new() -> Self {
		DummyWeightTrader
	}

	fn buy_weight(&mut self, _weight: Weight, _payment: Assets) -> Result<Assets, XcmError> {
		Ok(Assets::default())
	}
}

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type Call = Call;
	type XcmSender = DoNothingRouter;
	type AssetTransactor = DummyAssetTransactor;
	type OriginConverter = pallet_xcm::XcmPassthrough<Origin>;
	type IsReserve = ();
	type IsTeleporter = ();
	type LocationInverter = InvertNothing;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	type Trader = DummyWeightTrader;
	type ResponseHandler = ();
	type SubscriptionService = ();
	type AssetTrap = PolkadotXcm;
	type AssetClaims = PolkadotXcm;
}

impl pallet_xcm::Config for Test {
	// The config types here are entirely configurable, since the only one that is sorely needed
	// is `XcmExecutor`, which will be used in unit tests located in xcm-executor.
	type Event = Event;
	type SendXcmOrigin = ConvertOriginToLocal;
	type XcmRouter = DoNothingRouter;
	type ExecuteXcmOrigin = ConvertOriginToLocal;
	type XcmExecuteFilter = frame_support::traits::Everything;
	type XcmExecutor = xcm_executor::XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = frame_support::traits::Everything;
	type XcmReserveTransferFilter = frame_support::traits::Everything;
	type Weigher = xcm_builder::FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	type LocationInverter = InvertNothing;
	type Origin = Origin;
	type Call = Call;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

pub struct TransactionFeeCalculator<S>(sp_std::marker::PhantomData<S>);
impl<S> FeeCalculator<S> for TransactionFeeCalculator<S>
where
	S: pallet_balances::Config + sequester_pallet::Config,
	<S as frame_system::Config>::AccountId: From<AccountId>,
	<S as frame_system::Config>::AccountId: Into<AccountId>,
{
	fn match_event(
		event: pallet_balances::Event<S>,
		curr_block_fee_sum: &mut <S as sequester_pallet::Config>::Balance,
	) {
		todo!()
	}
}

pub struct SequesterAccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for SequesterAccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		todo!()
	}
}

impl sequester_pallet::Config for Test {
	type Event = Event;
	type BalancesEvent = Event;
	type Balance = Balance;
	type UnsignedPriority = UnsignedPriority;
	type SendInterval = SendInterval;

	type DonationsXCMAccount = DonationsXCMAccount;
	type TxnFeePercentage = TxnFeePercentage;
	type FeeCalculator = TransactionFeeCalculator<Self>;
	type AccountIdToMultiLocation = SequesterAccountIdToMultiLocation;
	type SequesterTransferFee = SequesterTransferFee;
	type SequesterTransferWeight = SequesterTransferWeight;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
