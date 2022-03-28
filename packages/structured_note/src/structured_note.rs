use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub stable_denom: String,
    pub governance_contract: String,
    pub mirror_mint_contract: String,
    pub anchor_market_contract: String,
    pub aterra_addr: String,
    pub nexus_treasury: String,
    pub protocol_fee: Decimal,
    pub min_over_collateralization: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {
        masset_token: String,
        leverage: Option<u8>,
        aim_collateral_ratio: Decimal,
    },
    RawDeposit {
        masset_token: String,
    },
    Withdraw {
        masset_token: String,
        aim_collateral: Uint128,
        aim_collateral_ratio: Decimal,
    },
    RawWithdraw {
        masset_token: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Position {
        masset_token: String,
    }
}
