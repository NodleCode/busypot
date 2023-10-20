use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::dev;

#[subxt::subxt(runtime_metadata_path = "eden.scale")]
pub mod eden {}

use eden::runtime_types::{
    pallet_mandate::pallet::Call::apply,
    pallet_xcm::pallet::Call::send,
    runtime_eden::RuntimeCall,
    sp_weights::weight_v2::Weight,
    xcm::{
        double_encoded::DoubleEncoded,
        v2::OriginKind,
        v3::{
            junction::Junction,
            junctions::Junctions,
            multiasset::{
                AssetId, Fungibility, MultiAsset, MultiAssetFilter, MultiAssets, WildMultiAsset,
            },
            multilocation::MultiLocation,
            Instruction::{BuyExecution, DepositAsset, RefundSurplus, Transact, WithdrawAsset},
            WeightLimit, Xcm,
        },
        VersionedMultiLocation, VersionedXcm,
    },
};

fn build_fee_asset(amount: u128) -> MultiAsset {
    MultiAsset {
        id: AssetId::Concrete(MultiLocation {
            parents: 0,
            interior: Junctions::Here,
        }),
        fun: Fungibility::Fungible(amount),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url("ws://localhost:9280").await?;
    println!("Connection Established");

    let fee_limit = 1000000000000000000;

    let withdraw_asset = WithdrawAsset(MultiAssets(vec![build_fee_asset(fee_limit)]));

    let buy_execution = BuyExecution {
        fees: build_fee_asset(fee_limit),
        weight_limit: WeightLimit::Unlimited,
    };

    let register_swap_between_para_2026_and_para_2000 = hex::decode("4603ea070000d0070000")?;
    let transact = Transact {
        origin_kind: OriginKind::Native,
        require_weight_at_most: Weight {
            ref_time: 10000000000,
            proof_size: 1000000,
        },
        call: DoubleEncoded {
            encoded: register_swap_between_para_2026_and_para_2000,
        },
    };

    let refund_surplus = RefundSurplus;

    let deposit_asset = DepositAsset {
        assets: MultiAssetFilter::Wild(WildMultiAsset::All),
        beneficiary: MultiLocation {
            parents: 0,
            interior: Junctions::X1(Junction::Parachain(2026)),
        },
    };

    let dest = VersionedMultiLocation::V3(MultiLocation {
        parents: 1,
        interior: Junctions::Here,
    });

    let message = VersionedXcm::V3(Xcm(vec![
        withdraw_asset,
        buy_execution,
        transact,
        refund_surplus,
        deposit_asset,
    ]));

    let send_xcm_call = RuntimeCall::PolkadotXcm(send {
        message: Box::new(message),
        dest: Box::new(dest),
    });

    let technical_committee_call = RuntimeCall::Mandate(apply {
        call: send_xcm_call.into(),
    });

    let technical_committee =
        eden::tx()
            .technical_committee()
            .propose(1, technical_committee_call, 100);

    let from = dev::alice();
    let events = api
        .tx()
        .sign_and_submit_then_watch_default(&technical_committee, &from)
        .await?
        .wait_for_finalized_success()
        .await?;

    println!("events: {events:?}");
    Ok(())
}
