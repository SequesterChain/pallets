# Sequester Donations Pallet

A pallet that can be added to any Polkadot or Kusama based chain which will track transaction fees sent over a period of time, and send a percentage of them to the Sequester Common Good Chain, where the funds are exchanged for Carbon Credits that are permanently retired.

## Adding the Pallet To Your Chain

To ease the process of adding the pallet to your chain, we have provided both an example node, which implements the Sequester pallet, and documentation on configuration options.

### Example Node

The example node can be found in the [sequester-example-node repository](https://github.com/SequesterChain/sequester-example-node).

Included are docker files to set up the development environment and instructions on how to run our testing infrastructure.

### Documentation/Configuration Options

A short overview of our pallet types are below, with comments. You can view an example implementation [here](https://github.com/SequesterChain/sequester-example-node/blob/main/runtime/src/lib.rs#L325-L360). For a more in-depth overview, please see our technical deep dive on the donations pallet [here](https://medium.com/@sequester.chain/introducing-sequesters-donations-pallet-3e55f54cdfd1).

```
// The standard event type
type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

// Type used to convert generic frame events into the
// event type specifically emitted by the balances pallet
type BalancesEvent: From<<Self as frame_system::Config>::Event>
    + TryInto<pallet_balances::Event<Self>>;

// A struct that calculates your chain’s transaction fees from
// the frame_events in a given block
type FeeCalculator: FeeCalculator<Self>;

// A standard AccountIdToMultiLocation converter
type AccountIdToMultiLocation: Convert<Self::AccountId, MultiLocation>;

// Weight information for extrinsics in this pallet
type WeightInfo: WeightInfo;

// weight of an xcm transaction to send to sequester
#[pallet::constant]
type SequesterTransferFee: Get<BalanceOf<Self>>;

// weight of an xcm transaction to send to sequester
#[pallet::constant]
type SequesterTransferWeight: Get<Weight>;

// The priority of the unsigned transactions submitted by
// the Sequester pallet
#[pallet::constant]
type UnsignedPriority: Get<TransactionPriority>;

// Interval (in blocks) at which we store the pending fees and reset the
// txn-fee-sum variable
#[pallet::constant]
type OnChainUpdateInterval: Get<Self::BlockNumber>;

// The percentage of transaction fees that you would like to send
// from your chain’s treasury to the Sequester chain
#[pallet::constant]
type TxnFeePercentage: Get<Percent>;

// The MultiLocation representing where the reserve funds
// are stored (e.g Statemine/Statemint)
#[pallet::constant]
type ReserveMultiLocation: Get<MultiLocation>;

// The MultiLocation representing where the funds will be going
// e.g (sequester kusama chain or sequester polkadot chain)
#[pallet::constant]
type SequesterMultiLocation: Get<MultiLocation>;
```

### Testing

To run our tests directly, run the following command from the donations folder:

```
cargo test --package pallet-donations --lib -- tests --nocapture
```
