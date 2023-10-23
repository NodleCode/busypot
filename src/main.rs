use async_std::fs::read_to_string;
use clap::Parser;
use serde_json;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519;

mod decrypt_key;
mod json_key;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Parachain RPC endpoint
    #[arg(short, long, default_value = "ws://localhost:9280")]
    url: String,

    /// A string containing a native transaction on Relaychain encoded in hex,
    /// Example: "4603ea070000d0070000" for registering swap between para 2026 and para 2000
    #[arg(short, long)]
    transact: String,

    /// Path to the json file for your account data including the signer key. When this is not
    /// specified then Alice account will be used as the signer
    #[arg(short, long, default_value = "")]
    json_key_path: String,

    /// Password for the json key file
    #[arg(short, long, default_value = "")]
    password: String,
}

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
    let args = Args::parse();

    let api = OnlineClient::<PolkadotConfig>::from_url(args.url).await?;
    println!("Connection Established");

    let fee_limit = 1000000000000000000;

    let withdraw_asset = WithdrawAsset(MultiAssets(vec![build_fee_asset(fee_limit)]));

    let buy_execution = BuyExecution {
        fees: build_fee_asset(fee_limit),
        weight_limit: WeightLimit::Unlimited,
    };

    let native_transact = hex::decode(args.transact)?;
    let transact = Transact {
        origin_kind: OriginKind::Native,
        require_weight_at_most: Weight {
            ref_time: 10000000000,
            proof_size: 1000000,
        },
        call: DoubleEncoded {
            encoded: native_transact,
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

    let from = if args.json_key_path.is_empty() {
        sr25519::dev::alice()
    } else {
        let json_key = read_to_string(args.json_key_path).await?;
        let account_data: json_key::AccountData = serde_json::from_str(&json_key)?;
        let private_key = decrypt_key::decrypt(
            account_data.encoded.into_bytes().as_slice(),
            args.password.as_str(),
        )?;
        let seed = sr25519::Seed::try_from(private_key.as_slice())?;
        sr25519::Keypair::from_seed(seed)?
    };

    let events = api
        .tx()
        .sign_and_submit_then_watch_default(&technical_committee, &from)
        .await?
        .wait_for_finalized_success()
        .await?;

    println!("events: {events:?}");
    Ok(())
}
