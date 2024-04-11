// Copyright 2019-2023 Parity Technologies (UK) Ltd and Nodle International.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

//! Nodle specific configuration

use scale_info::PortableRegistry;
use subxt::client::OfflineClientT;
use subxt::config::{
    signed_extensions, Config, ExtrinsicParams, ExtrinsicParamsEncoder, ExtrinsicParamsError,
    Header, SignedExtension, SubstrateConfig,
};
use subxt::utils::MultiAddress;

pub struct CheckWeight;
impl<T: Config> SignedExtension<T> for CheckWeight {
    type Decoded = ();
    fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
        identifier == "CheckWeight"
    }
}
impl<T: Config> ExtrinsicParams<T> for CheckWeight {
    type Params = ();

    fn new<Client: OfflineClientT<T>>(
        _client: Client,
        _other_params: Self::Params,
    ) -> Result<Self, ExtrinsicParamsError> {
        Ok(CheckWeight)
    }
}
impl ExtrinsicParamsEncoder for CheckWeight {}

pub struct ChargeSponsor;
impl<T: Config> SignedExtension<T> for ChargeSponsor {
    type Decoded = ();
    fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
        identifier == "ChargeSponsor"
    }
}
impl<T: Config> ExtrinsicParams<T> for ChargeSponsor {
    type Params = ();

    fn new<Client: OfflineClientT<T>>(
        _client: Client,
        _other_params: Self::Params,
    ) -> Result<Self, ExtrinsicParamsError> {
        Ok(ChargeSponsor)
    }
}
impl ExtrinsicParamsEncoder for ChargeSponsor {}

/// A struct representing the signed extra and additional parameters required
/// to construct a transaction for a Nodle node.
pub type NodleExtrinsicParams<T> = signed_extensions::AnyOf<
    T,
    (
        signed_extensions::CheckSpecVersion,
        signed_extensions::CheckTxVersion,
        signed_extensions::CheckGenesis<T>,
        signed_extensions::CheckMortality<T>,
        signed_extensions::CheckNonce,
        CheckWeight,
        signed_extensions::ChargeTransactionPayment,
        ChargeSponsor,
    ),
>;

/// Default set of commonly used types by Nodle nodes.
pub enum NodleConfig {}
impl Config for NodleConfig {
    type Hash = <SubstrateConfig as Config>::Hash;
    type AccountId = <SubstrateConfig as Config>::AccountId;
    type Address = MultiAddress<Self::AccountId, ()>;
    type Signature = <SubstrateConfig as Config>::Signature;
    type Hasher = <SubstrateConfig as Config>::Hasher;
    type Header = <SubstrateConfig as Config>::Header;
    type ExtrinsicParams = NodleExtrinsicParams<Self>;
    type AssetId = u32;
}

/// A builder that outputs the set of [`super::ExtrinsicParams::OtherParams`] required for
/// [`NodleExtrinsicParams`].
pub struct NodleExtrinsicParamsBuilder<T: Config> {
    /// `None` means the tx will be immortal.
    mortality: Option<Mortality<T::Hash>>,
    tip: u128,
    nonce: Option<u64>,
}

struct Mortality<Hash> {
    /// Block hash that mortality starts from
    checkpoint_hash: Hash,
    /// Block number that mortality starts from (must
    // point to the same block as the hash above)
    checkpoint_number: u64,
    /// How many blocks the tx is mortal for
    period: u64,
}

impl<T: Config> Default for NodleExtrinsicParamsBuilder<T> {
    fn default() -> Self {
        Self {
            mortality: None,
            tip: 0,
            nonce: None,
        }
    }
}

impl<T: Config> NodleExtrinsicParamsBuilder<T> {
    /// Make the transaction mortal, given a block header that it should be mortal from,
    /// and the number of blocks (roughly; it'll be rounded to a power of two) that it will
    /// be mortal for.
    #[allow(dead_code)]
    pub fn mortal(mut self, from_block: &T::Header, for_n_blocks: u64) -> Self {
        self.mortality = Some(Mortality {
            checkpoint_hash: from_block.hash(),
            checkpoint_number: from_block.number().into(),
            period: for_n_blocks,
        });
        self
    }

    /// Provide a tip to the block author in the chain's native token.
    #[allow(dead_code)]
    pub fn tip(mut self, tip: u128) -> Self {
        self.tip = tip;
        self
    }

    /// Allow nonce to be set explicitly.
    #[allow(dead_code)]
    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Build the extrinsic parameters.
    pub fn build(self) -> <NodleExtrinsicParams<T> as ExtrinsicParams<T>>::Params {
        let check_mortality_params = if let Some(mortality) = self.mortality {
            signed_extensions::CheckMortalityParams::mortal(
                mortality.period,
                mortality.checkpoint_number,
                mortality.checkpoint_hash,
            )
        } else {
            signed_extensions::CheckMortalityParams::immortal()
        };

        let charge_transaction_params =
            signed_extensions::ChargeTransactionPaymentParams::tip(self.tip);

        (
            (),
            (),
            (),
            check_mortality_params,
            signed_extensions::CheckNonceParams(self.nonce),
            (),
            charge_transaction_params,
            (),
        )
    }
}
