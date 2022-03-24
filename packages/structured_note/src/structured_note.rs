use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

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
        aim_collateral: String,
    },
    Withdraw {
        masset_token: String,
        aim_collateral: Uint128,
        aim_collateral_ratio: Decimal,
    },
    RawWithdraw {
        masset_token: String,
        aim_collateral: Uint128,
    },
    ClosePosition {
        masset_token: String,
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
