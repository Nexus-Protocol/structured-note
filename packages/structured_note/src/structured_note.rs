use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    OpenPosition {
        masset_token: String,
        leverage: u8,
        initial_collateral_ratio: Decimal,
    },
    Deposit {
        masset_token: String,
        aim_collateral_ratio: Decimal,
    },
    ClosePosition {
        masset_token: String,
    },
    Withdraw {
        masset_token: String,
        amount: Uint128,
        aim_collateral_ratio: Decimal,
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
