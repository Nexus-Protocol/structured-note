use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{Addr, Decimal, Order, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static KEY_CONFIG: Item<Config> = Item::new("config");
static KEY_STATE: Item<State> = Item::new("state");
// Map<cdp.masset_token, CDP>
static KEY_CDPS: Map<&Addr, CDP> = Map::new("cdps");
// Map<(position.farmer_addr, position.masset_token), Position>
static KEY_POSITIONS: Map<(&Addr, &Addr), Position> = Map::new("positions");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub stable_denom: String,
    pub governance_contract: Addr,
    pub mirror_mint_contract: Addr,
    pub anchor_market_contract: Addr,
    pub aterra_addr: Addr,
    pub nexus_treasury: Addr,
    pub protocol_fee: Decimal256,
    pub min_over_collateralization: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CDP {
    pub idx: Uint128,
    pub masset_token: Addr,
    pub farmers: Vec<Addr>,
}

//Store data for recursive deposit and withdraw
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub cdp_idx: Option<Uint128>,
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub leverage: u8,
    pub cur_iteration_index: u8,
    pub asset_price_in_collateral_asset: Decimal,
    pub pair_addr: Addr,
    pub aim_collateral_ratio: Decimal,
    pub loan_amount_diff: Uint128,
    pub collateral_amount_diff: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub cdp_idx: Uint128,
    pub leverage: u8,
    pub total_loan_amount: Uint128,
    pub total_collateral_amount: Uint128,
    pub aim_collateral_ratio: Decimal,
}

pub fn load_config(storage: &dyn Storage) -> StdResult<Config> {
    KEY_CONFIG.load(storage)
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    KEY_CONFIG.save(storage, config)
}

pub fn may_load_cdp(storage: &dyn Storage, masset_token: &Addr) -> StdResult<Option<CDP>> {
    KEY_CDPS.may_load(storage, masset_token)
}

pub fn load_cdp(storage: &dyn Storage, masset_token: &Addr) -> StdResult<CDP> {
    KEY_CDPS.load(storage, masset_token)
}

pub fn load_all_cdps(storage: &dyn Storage) -> StdResult<Vec<CDP>> {
    KEY_CDPS
        .range(storage, None, None, Order::Ascending)
        .map(|cdp| {
            let (_, v) = cdp?;
            Ok(CDP {
                idx: v.idx,
                masset_token: v.masset_token,
                farmers: v.farmers,
            })
        })
        .collect()
}

pub fn save_cdp(storage: &mut dyn Storage, cdp: &CDP) -> StdResult<()> {
    KEY_CDPS.save(storage, &cdp.masset_token, cdp)
}

pub fn remove_cdp(storage: &mut dyn Storage, masset_token: &Addr) {
    KEY_CDPS.remove(storage, masset_token)
}

pub fn update_cdp(storage: &mut dyn Storage, state: &State) -> StdResult<CDP> {
    let action = |cdp: Option<CDP>| -> StdResult<CDP> {
        match cdp {
            None => Ok(
                CDP {
                    idx: state.cdp_idx.unwrap(),
                    masset_token: state.masset_token.clone(),
                    farmers: vec![state.farmer_addr.clone()],
                }
            ),
            Some(mut cdp) => {
                if !cdp.farmers.contains(&state.farmer_addr) {
                    cdp.farmers.push(state.farmer_addr.clone());
                }
                Ok(cdp)
            }
        }
    };
    KEY_CDPS.update(storage, &state.masset_token, action)
}

pub fn load_state(storage: &dyn Storage) -> StdResult<State> {
    KEY_STATE.load(storage)
}

pub fn store_state(storage: &mut dyn Storage, data: &State) -> StdResult<()> {
    KEY_STATE.save(storage, data)
}

pub fn increment_iteration_index(storage: &mut dyn Storage) -> StdResult<State> {
    KEY_STATE.update(storage, |mut s: State| -> StdResult<State> {
        s.cur_iteration_index += 1;
        Ok(s)
    })
}

pub fn insert_state_cdp_idx(storage: &mut dyn Storage, cdp_idx: Uint128) -> StdResult<State> {
    KEY_STATE.update(storage, |mut s: State| -> StdResult<State> {
        if (s.cdp_idx.is_none()) {
            s.cdp_idx = Some(cdp_idx);
        }
        Ok(s)
    })
}

pub fn increase_state_collateral_diff(storage: &mut dyn Storage, diff: Uint128) -> StdResult<State> {
    KEY_STATE.update(storage, |mut s: State| -> StdResult<State> {
        s.collateral_amount_diff += diff;
        Ok(s)
    })
}

pub fn increase_state_loan_diff(storage: &mut dyn Storage, diff: Uint128) -> StdResult<State> {
    KEY_STATE.update(storage, |mut s: State| -> StdResult<State> {
        s.loan_amount_diff += diff;
        Ok(s)
    })
}

pub fn may_load_position(storage: &dyn Storage, farmer_addr: &Addr, masset_token: &Addr) -> StdResult<Option<Position>> {
    KEY_POSITIONS.may_load(storage, (farmer_addr, masset_token))
}

pub fn load_position(storage: &dyn Storage, farmer_addr: &Addr, masset_token: &Addr) -> StdResult<Position> {
    KEY_POSITIONS.load(storage, (farmer_addr, masset_token))
}

pub fn upsert_position(storage: &mut dyn Storage,
                       state: &State,
                       loan_diff: Uint128,
                       collateral_diff: Uint128,
) -> StdResult<Position> {
    let action = |p: Option<Position>| -> StdResult<Position> {
        match p {
            None => Ok(
                Position {
                    farmer_addr: state.farmer_addr.clone(),
                    masset_token: state.masset_token.clone(),
                    cdp_idx: state.cdp_idx.unwrap(),
                    leverage: state.leverage,
                    aim_collateral_ratio: state.aim_collateral_ratio,
                    total_loan_amount: loan_diff,
                    total_collateral_amount: collateral_diff,
                }
            ),
            Some(mut position) => {
                position.total_collateral_amount += collateral_diff;
                position.total_loan_amount += loan_diff;
                Ok(position)
            }
        }
    };
    KEY_POSITIONS.update(storage, (&state.farmer_addr, &state.masset_token), action)
}

pub fn load_all_positions(storage: &dyn Storage) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .range(storage, None, None, Order::Ascending)
        .map(|position| {
            let (_, v) = position?;
            Ok(Position {
                farmer_addr: v.farmer_addr,
                masset_token: v.masset_token,
                cdp_idx: v.cdp_idx,
                leverage: v.leverage,
                total_loan_amount: v.total_loan_amount,
                total_collateral_amount: v.total_collateral_amount,
                aim_collateral_ratio: v.aim_collateral_ratio,
            })
        })
        .collect()
}

pub fn load_positions_by_farmer_addr(storage: &dyn Storage, farmer_addr: &Addr) -> StdResult<Vec<Position>> {
    KEY_POSITIONS
        .prefix(farmer_addr)
        .range(storage, None, None, Order::Ascending)
        .map(|position| {
            let (_, v) = position?;
            Ok(Position {
                farmer_addr: v.farmer_addr,
                masset_token: v.masset_token,
                cdp_idx: v.cdp_idx,
                leverage: v.leverage,
                total_loan_amount: v.total_loan_amount,
                total_collateral_amount: v.total_collateral_amount,
                aim_collateral_ratio: v.aim_collateral_ratio,
            })
        })
        .collect()
}

pub fn save_position(storage: &mut dyn Storage, position: &Position) -> StdResult<()> {
    KEY_POSITIONS.save(storage, (&position.farmer_addr, &position.masset_token), position)
}

pub fn remove_position(storage: &mut dyn Storage, farmer_addr: &Addr, masset_token: &Addr) {
    KEY_POSITIONS.remove(storage, (farmer_addr, masset_token))
}