use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{Addr, Decimal, Order, StdError, StdResult, Storage, Uint128};
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
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub leverage: u8,
    pub cur_iteration_index: u8,
    pub asset_price_in_collateral_asset: Decimal,
    pub pair_addr: Addr,
    pub aim_collateral_ratio: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub farmer_addr: Addr,
    pub masset_token: Addr,
    pub cdp_idx: Uint128,
    pub leverage: u8,
    pub loan_amount: Uint128,
    pub collateral_amount: Uint128,
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

pub fn update_cdp(storage: &mut dyn Storage, cdp_idx: Uint128, farmer_addr: Addr, masset_token: Addr) -> StdResult<CDP> {
    let action = |cdp: Option<CDP>| -> StdResult<CDP> {
        match cdp {
            None => Ok(
                CDP {
                    idx: cdp_idx,
                    masset_token,
                    farmers: vec![farmer_addr],
                }
            ),
            Some(mut cdp) => {
                if !cdp.farmers.contains(&farmer_addr) {
                    cdp.farmers.push(farmer_addr);
                }
                Ok(cdp)
            }
        }
    };
    KEY_CDPS.update(storage, &masset_token, action)
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

pub fn may_load_position(storage: &dyn Storage, farmer_addr: &Addr, masset_token: &Addr) -> StdResult<Option<Position>> {
    KEY_POSITIONS.may_load(storage, (farmer_addr, masset_token))
}

pub fn increase_position_collateral(storage: &mut dyn Storage, farmer_addr: &Addr, masset_token: &Addr, diff: Uint128) -> StdResult<Position> {
    KEY_POSITIONS.update(storage, (farmer_addr, masset_token), |mut p: Option<Position>| -> StdResult<Position> {
        if let Some(mut p) = p {
            p.collateral_amount += diff;
            Ok(p)
        } else {
            Err(StdError::generic_err(format!(
                "There isn't position: farmer_addr: {}, masset_token: {}.",
                &farmer_addr.to_string(),
                &masset_token.to_string())))
        }
    })
}

pub fn increase_position_loan(storage: &mut dyn Storage, farmer_addr: &Addr, masset_token: &Addr, diff: Uint128) -> StdResult<Position> {
    KEY_POSITIONS.update(storage, (farmer_addr, masset_token), |mut p: Option<Position>| -> StdResult<Position> {
        if let Some(mut p) = p {
            p.loan_amount += diff;
            Ok(p)
        } else {
            Err(StdError::generic_err(format!(
                "There isn't position: farmer_addr: {}, masset_token: {}.",
                &farmer_addr.to_string(),
                &masset_token.to_string())))
        }
    })
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
                loan_amount: v.loan_amount,
                collateral_amount: v.collateral_amount,
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
                loan_amount: v.loan_amount,
                collateral_amount: v.collateral_amount,
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