use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HubQueryMsg {
    Price {
        asset_token: String,
        timeframe: Option<u64>,
    }
}
