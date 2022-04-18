use cosmwasm_std::{Addr, CanonicalAddr, Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terraswap::asset::{Asset, AssetInfo, AssetRaw};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMintExecuteMsg {
    Mint {
        position_idx: Uint128,
        asset: Asset,
        short_params: Option<()>,
    },
    Withdraw {
        position_idx: Uint128,
        collateral: Option<Asset>,
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
#[serde(rename_all = "snake_case")]
pub enum MirrorMintQueryMsg {
    Config {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorMintConfigResponse {
    pub owner: CanonicalAddr,
    pub oracle: CanonicalAddr,
    pub collector: CanonicalAddr,
    pub collateral_oracle: CanonicalAddr,
    pub staking: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub lock: CanonicalAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorMintInfo {
    // pub owner: Addr,
    pub oracle: Addr,
    // pub collector: Addr,
    pub collateral_oracle: Addr,
    // pub staking: Addr,
    pub terraswap_factory: Addr,
    // pub lock: Addr,
    // pub base_denom: String,
    // pub token_code_id: u64,
    // pub protocol_fee_rate: Decimal,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorCDPResponse {
    pub idx: Uint128,
    pub owner: Addr,
    pub collateral: AssetRaw,
    pub asset: AssetRaw,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CDPState {
    pub collateral_amount: Uint128,
    pub loan_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorCollateralOracleQueryMsg {
    CollateralPrice {
        asset: String,
        block_height: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorCollateralPriceResponse {
    pub asset: String,
    pub rate: Decimal,
    pub last_updated: u64,
    pub multiplier: Decimal,
    pub is_revoked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum MirrorOracleQueryMsg {
    Price {
        base_asset: String,
        quote_asset: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MirrorPriceResponse {
    pub rate: Decimal,
    pub last_updated: u64,
}