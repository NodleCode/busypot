use clap::Parser;
use std::str::FromStr;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::{sr25519, SecretUri};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Parachain RPC endpoint
    #[arg(short, long, default_value = "ws://localhost:9280")]
    url: String,

    /// A string containing a native transaction on Relaychain encoded in hex.
    ///
    /// Example: "4603ea070000d0070000" for registering swap between para 2026 and para 2000
    #[arg(short, long)]
    transact: String,

    /// The secret uri to the private key for the signer of the transactions. If not provided,
    /// you will be prompted to securely enter the seed manually.
    ///
    /// Here is the expected format for the secret uri:
    ///
    /// phrase/path0/path1///password
    ///
    /// 111111 22222 22222   33333333
    ///
    /// Where:
    ///
    /// 1s denotes a phrase or hex string. If this is not provided, the DEV_PHRASE is used instead.
    ///
    /// 2s denote optional "derivation junctions" which are used to derive keys. Each of these is
    /// separated by "/". A derivation junction beginning with "/" (ie "//" in the original string)
    /// is a "hard" path.
    ///
    /// 3s denotes an optional password which is used in conjunction with the phrase provided in 1
    /// to generate an initial key. If hex is provided for 1, it's ignored.
    ///
    /// Notes:
    ///
    /// If 1 is a 0x prefixed 64-digit hex string, then we'll interpret it as hex, and treat the hex
    /// bytes as a seed/MiniSecretKey directly, ignoring any password.
    ///
    /// Else if the phrase part is a valid BIP-39 phrase, we'll use the phrase (and password, if
    /// provided) to generate a seed/MiniSecretKey.
    ///
    /// Uris like "//Alice" correspond to keys derived from a DEV_PHRASE, since no phrase part is
    /// given.
    #[arg(short, long)]
    signer: Option<String>,

    /// The maixmum number of tokens we are willing to spend on fees.
    ///
    /// This is a float number, and is interpreted as the number of tokens in the highest
    /// denomination. For example, if the token has 18 decimals, then the default value of 1 means
    /// 1 token.
    #[arg(short, long, default_value = "1")]
    fee_limit: f32,
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

const DOT_DECIMALS: u128 = 10_000_000_000; // 10 decimals

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

    let fee_limit = (args.fee_limit * DOT_DECIMALS as f32) as u128;
    println!("fee_limit set to: {}", fee_limit);

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

    let seed = match args.signer {
        Some(s) => s,
        None => rpassword::prompt_password("Enter your seed: ")?,
    };
    let from = sr25519::Keypair::from_uri(&SecretUri::from_str(&seed)?)?;
    let events = api
        .tx()
        .sign_and_submit_then_watch_default(&technical_committee, &from)
        .await?
        .wait_for_finalized_success()
        .await?;

    println!("events: {events:?}");
    Ok(())
}
