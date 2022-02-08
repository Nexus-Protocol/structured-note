use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terraswap::asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMintExecuteMsg {
    Mint {
        position_idx: Uint128,
        asset: Asset,
        short_params: Option<()>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMintCW20HookMsg {
    OpenPosition {
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
        short_params: Option<()>,
    },
    Deposit { position_idx: Uint128 },
    Burn { position_idx: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorMintConfigResponse {
    pub owner: String,
    pub oracle: String,
    pub collector: String,
    pub collateral_oracle: String,
    pub staking: String,
    pub terraswap_factory: String,
    pub lock: String,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorAssetConfigResponse {
    pub token: String,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
    pub ipo_params: Option<IPOParams>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IPOParams {
    pub mint_end: u64,
    pub pre_ipo_price: Decimal,
    pub min_collateral_ratio_after_ipo: Decimal,
}