use crate::{self as donations_pallet, FeeCalculator, Pallet};

use core::ops::AddAssign;
use frame_support::{
    parameter_types,
    traits::{
        ConstU32, ConstU64, ConstU8, Currency, EnsureOrigin, Everything, Imbalance, OnUnbalanced,
        OriginTrait,
    },
    weights::{DispatchInfo, IdentityFee, PostDispatchInfo, Weight},
    PalletId,
};
use frame_system as system;
use std::cell::RefCell;

use pallet_transaction_payment::{ChargeTransactionPayment, CurrencyAdapter, Multiplier};
use pallet_treasury::BalanceOf;
use sp_runtime::{
    offchain::{
        testing::{self},
        OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
    },
    traits::{AccountIdConversion, Convert, Saturating},
};
use xcm_builder::{AllowUnpaidExecutionFrom, FixedWeightBounds};
use xcm_executor::{
    traits::{InvertLocation, TransactAsset, WeightTrader},
    Assets,
};

use sp_runtime::{traits::IdentityLookup, Percent};
use xcm::latest::prelude::*;
use xcm::latest::{
    Error as XcmError, MultiAsset, MultiLocation, Result as XcmResult, SendResult, SendXcm, Xcm,
};

type AccountId = u64;
type AccountIndex = u32;
type BlockNumber = u64;
type Balance = u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type NegativeImbalance<T> = <pallet_balances::Pallet<T> as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub const CALL: &<Test as frame_system::Config>::Call =
    &Call::Balances(pallet_balances::Call::transfer { dest: 2, value: 50 });

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
pub fn info_from_weight(w: Weight) -> DispatchInfo {
    DispatchInfo {
        weight: w,
        ..Default::default()
    }
}

pub fn default_post_info() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Default::default(),
    }
}

pub const MOCK_WEIGHT: Weight = 600_000_000;

pub const ACC_BAL_1: Balance = 100000000000000;
pub const ACC_BAL_2: Balance = 200000000000000;
pub const ACC_BAL_3: Balance = 300000000000000;

pub const TXN_FEE: Balance = 725000010;

pub const FROM_ACCOUNT: u64 = 1;
pub const TO_ACCOUNT: u64 = 2;

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
        Treasury: pallet_treasury::{Pallet, Call, Storage, Event<T>},
        Donations: donations_pallet::{Pallet, Call, Storage, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage},
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

    pub const TxnFeePercentage: Percent = Percent::from_percent(10);
    pub SequesterTransferWeight: Weight = 100000000000;
    pub SequesterTransferFee: Balance = 10000000;

    pub ReserveMultiLocation: MultiLocation = MultiLocation::new(
        1,
        Junctions::X1(Junction::Parachain(1000)),
    );
    pub SequesterMultiLocation: MultiLocation = MultiLocation::new(
        1,
        Junctions::X1(Junction::Parachain(9999)),
    );
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
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
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

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
}

impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Self>>;
    type TransactionByteFee = TransactionByteFee;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
}

pub struct TransactionFeeCalculator<S>(sp_std::marker::PhantomData<S>);
impl<S> FeeCalculator<S> for TransactionFeeCalculator<S>
where
    S: pallet_balances::Config + donations_pallet::Config,
    <S as frame_system::Config>::AccountId: From<AccountId>,
    <S as frame_system::Config>::AccountId: Into<AccountId>,
    u64: From<
        <<S as pallet_treasury::Config>::Currency as Currency<
            <S as frame_system::Config>::AccountId,
        >>::Balance,
    >,
    u64: Into<
        <<S as pallet_treasury::Config>::Currency as Currency<
            <S as frame_system::Config>::AccountId,
        >>::Balance,
    >,
{
    fn match_event(event: pallet_balances::Event<S>, curr_block_fee_sum: &mut BalanceOf<S>) {
        let fee = FEE_UNBALANCED_AMOUNT.with(|a| a.borrow().clone());
        *curr_block_fee_sum = (*curr_block_fee_sum).saturating_add(fee.into());
    }
}

pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
    R: pallet_balances::Config + pallet_treasury::Config,
    u64: AddAssign<<R as pallet_balances::Config>::Balance>,
    pallet_treasury::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
    // <R as frame_system::Config>::AccountId: From<primitives::v2::AccountId>,
    // <R as frame_system::Config>::AccountId: Into<primitives::v2::AccountId>,
    <R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
        if let Some(fees) = fees_then_tips.next() {
            FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow_mut() += fees.peek());
            if let Some(tips) = fees_then_tips.next() {
                TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow_mut() += tips.peek());
            }
        }
    }
}

pub struct SequesterAccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for SequesterAccountIdToMultiLocation {
    fn convert(account: AccountId) -> MultiLocation {
        todo!()
    }
}

impl donations_pallet::Config for Test {
    type Event = Event;
    type BalancesEvent = Event;
    type UnsignedPriority = UnsignedPriority;
    type SendInterval = SendInterval;

    type TxnFeePercentage = TxnFeePercentage;
    type FeeCalculator = TransactionFeeCalculator<Self>;
    type AccountIdToMultiLocation = SequesterAccountIdToMultiLocation;
    type SequesterTransferFee = SequesterTransferFee;
    type SequesterTransferWeight = SequesterTransferWeight;

    type ReserveMultiLocation = ReserveMultiLocation;
    type SequesterMultiLocation = SequesterMultiLocation;
}

thread_local! {
    pub static TIP_UNBALANCED_AMOUNT: RefCell<u64> = RefCell::new(0);
    pub static FEE_UNBALANCED_AMOUNT: RefCell<u64> = RefCell::new(0);
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 100), (2, 200), (3, 300)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn new_test_ext_with_offchain_worker() -> (sp_io::TestExternalities, testing::TestOffchainExt) {
    let (offchain, _offchain_state) = testing::TestOffchainExt::new();
    let (pool, _pool_state) = testing::TestTransactionPoolExt::new();

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, ACC_BAL_1), (2, ACC_BAL_2), (3, ACC_BAL_3)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);

    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain.clone()));
    ext.register_extension(TransactionPoolExt::new(pool));
    (ext, offchain)
}
