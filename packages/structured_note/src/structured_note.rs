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
    PlaneDeposit {
        masset_token: String,
    },
    PlaneMint {
        masset_token: String,
    },
    ClosePosition {
        masset_token: String,
    },
    Withdraw {
        masset_token: String,
        amount: Uint128,
        aim_collateral_ratio: Decimal,
    },
    PlaneWithdraw {
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
