use clap::{Parser, Subcommand};
use codec::Encode;
use std::collections::VecDeque;
use std::str::FromStr;
use subxt::{
    backend::{legacy::LegacyRpcMethods, rpc::RpcClient},
    OnlineClient, PolkadotConfig,
};
use subxt_signer::{sr25519, SecretUri};
const MAX_USERS_ONE_BLOCK: usize = 500;

#[derive(Debug, Subcommand)]
enum Commands {
    /// Proposes an xcm as a technical committee member for a native transaction on the relay chain
    #[command(arg_required_else_help = true)]
    ProposeXcm {
        /// A string containing a native transaction on relay-chain encoded in hex.
        ///
        /// Example: "4603ea070000d0070000" for registering swap between para 2026 and para 2000
        #[arg(short, long)]
        transact: String,
        /// "Dry Run" the proposal. This will output the proposal to be sent to the chain without
        /// actually doing so.
        #[arg(short, long)]
        dry_run: bool,
        /// The maximum number of tokens we are willing to spend on fees.
        ///
        /// This is a float number, and is interpreted as the number of tokens in the highest
        /// denomination. For example, if the token has 18 decimals, then the default value of 1 means
        /// 1 token.
        #[arg(short, long, default_value = "1")]
        fee_limit: f32,
    },
    /// Creates a number of sponsorship pots with their ids starting from 0 and incrementing
    CreatePots { pots: usize },
    /// Registers a number of users for the specified sponsorship pot
    RegisterUsers {
        /// The pot to register users in.
        #[arg(short, long, default_value_t = 0)]
        pot_id: u32,
        /// The number of users to add with their addresses derived form //Alice
        #[arg(short, long, default_value_t = 0)]
        users: usize,
    },
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Parachain RPC endpoint. This endpoint is necessary even `dry_run` is set to true for some
    /// commands. This is because for composing some of the transactions, there is a need to query
    /// the chain for some information.
    #[arg(short, long, default_value = "ws://localhost:9280")]
    url: String,

    /// The secret uri to the private key for the signer of the transactions.
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
    #[arg(short, long, default_value = "//Alice")]
    signer: String,

    #[command(subcommand)]
    command: Commands,
}

#[subxt::subxt(runtime_metadata_path = "eden.scale")]
pub mod eden {}

use eden::runtime_types::{
    pallet_mandate::pallet::Call::apply,
    pallet_xcm::pallet::Call::send,
    runtime_eden::{pallets_util::SponsorshipType, RuntimeCall},
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
const NODL_DECIMALS: u128 = 100_000_000_000; // 11 decimals

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

    let rpc_client = RpcClient::from_url(args.url.clone()).await?;
    let rpc = LegacyRpcMethods::<PolkadotConfig>::new(rpc_client.clone());
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client).await?;
    let from = sr25519::Keypair::from_uri(&SecretUri::from_str(&args.signer)?)?;

    let mut nonce = rpc
        .system_account_next_index(&from.public_key().into())
        .await?;
    println!("Connection Established nonce = {nonce}");

    match args.command {
        Commands::ProposeXcm {
            transact,
            dry_run,
            fee_limit,
        } => {
            let fee_limit = (fee_limit * DOT_DECIMALS as f32) as u128;
            println!("fee_limit set to: {}", fee_limit);

            let withdraw_asset = WithdrawAsset(MultiAssets(vec![build_fee_asset(fee_limit)]));

            let buy_execution = BuyExecution {
                fees: build_fee_asset(fee_limit),
                weight_limit: WeightLimit::Unlimited,
            };

            let native_transact = hex::decode(transact)?;
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

            let members_query = eden::storage().technical_membership().members();
            let members = api
                .storage()
                .at_latest()
                .await?
                .fetch(&members_query)
                .await?
                .unwrap();
            let threshold = members.0.len() / 2 + 1;
            println!("using tech committee threshold: {}", threshold);

            let length_bound = technical_committee_call.encoded_size() as u32;

            let technical_committee = eden::tx().technical_committee().propose(
                threshold as u32,
                technical_committee_call,
                length_bound,
            );

            if dry_run {
                let mocked = api.tx().call_data(&technical_committee)?;
                let mocked = format!("0x{}", hex::encode(mocked));

                println!("final extrinsic: {}", mocked);
                println!(
                    "shortlink: https://nodleprotocol.io/?rpc={}#/extrinsics/decode/{}",
                    urlencoding::encode(&args.url),
                    mocked
                );
            } else {
                let events = api
                    .tx()
                    .sign_and_submit_then_watch_default(&technical_committee, &from)
                    .await?
                    .wait_for_finalized_success()
                    .await?;

                println!("events: {events:?}");
            }
        }
        Commands::CreatePots { pots } => {
            println!("Creating {pots} pots... ");
            let mut tx_progresses = VecDeque::new();
            for i in 0..pots {
                println!("Creating pot {}/{}", i, pots);
                let create_pot = eden::tx().sponsorship().create_pot(
                    i as u32,
                    SponsorshipType::AnySafe,
                    123 * NODL_DECIMALS,
                    9 * NODL_DECIMALS,
                );
                let tx_progress = api
                    .tx()
                    .create_signed_with_nonce(&create_pot, &from, nonce, Default::default())?
                    .submit_and_watch()
                    .await?;
                tx_progresses.push_back(tx_progress);
                nonce += 1;
            }
            while let Some(tx_progress) = tx_progresses.pop_front() {
                tx_progress.wait_for_finalized_success().await?;
            }
            println!("Done!");
        }
        Commands::RegisterUsers { pot_id, users } => {
            println!("Creating {users} users... ");
            let chunked = (0..users)
                .into_iter()
                .collect::<Vec<_>>()
                .chunks(MAX_USERS_ONE_BLOCK)
                .map(|chunk| {
                    chunk
                        .into_iter()
                        .filter_map(|&i| {
                            SecretUri::from_str(format!("//Alice/{i}").as_str())
                                .map(|s| {
                                    sr25519::Keypair::from_uri(&s).map(|k| k.public_key().into())
                                })
                                .map_or(None, |r| r.ok())
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut tx_progresses = VecDeque::new();
            for chunk in chunked {
                println!(
                    "Registering {chunk_len} users / {users}",
                    chunk_len = chunk.len()
                );
                let register_user = eden::tx().sponsorship().register_users(
                    pot_id,
                    chunk,
                    43 * NODL_DECIMALS,
                    7 * NODL_DECIMALS,
                );
                let tx_progress = api
                    .tx()
                    .create_signed_with_nonce(&register_user, &from, nonce, Default::default())?
                    .submit_and_watch()
                    .await?;
                tx_progresses.push_back(tx_progress);
                nonce += 1;
            }
            while let Some(tx_progress) = tx_progresses.pop_front() {
                tx_progress.wait_for_finalized_success().await?;
            }
            println!("Done!");
        }
    };

    Ok(())
}
