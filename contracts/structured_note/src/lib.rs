use std::convert::TryFrom;

use cosmwasm_std::StdError;

pub mod state;
mod anchor;
mod mirror;
mod terraswap;
mod contract;
mod commands;
mod utils;

pub enum SubmsgIds {
    //Deposit
    DepositStable,
    OpenCDP,
    DepositToCDP,
    MintAsset,
    SellMAsset,
    Exit,
    //Withdraw
    WithdrawCollateral,
    RedeemStable,
    BuyAsset,
    BurnAsset,
}

impl TryFrom<u64> for SubmsgIds {
    type Error = StdError;

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        match v {
            x if x == SubmsgIds::DepositStable.id() => Ok(SubmsgIds::DepositStable),
            x if x == SubmsgIds::OpenCDP.id() => Ok(SubmsgIds::OpenCDP),
            x if x == SubmsgIds::DepositToCDP.id() => Ok(SubmsgIds::DepositToCDP),
            x if x == SubmsgIds::MintAsset.id() => Ok(SubmsgIds::MintAsset),
            x if x == SubmsgIds::SellMAsset.id() => Ok(SubmsgIds::SellMAsset),
            x if x == SubmsgIds::Exit.id() => Ok(SubmsgIds::Exit),
            x if x == SubmsgIds::WithdrawCollateral.id() => Ok(SubmsgIds::WithdrawCollateral),
            x if x == SubmsgIds::RedeemStable.id() => Ok(SubmsgIds::RedeemStable),
            x if x == SubmsgIds::BuyAsset.id() => Ok(SubmsgIds::BuyAsset),
            x if x == SubmsgIds::BurnAsset.id() => Ok(SubmsgIds::BurnAsset),

            unknown => Err(StdError::generic_err(format!(
                "unknown reply message id: {}",
                unknown
            ))),
        }
    }
}

impl SubmsgIds {
    pub const fn id(&self) -> u64 {
        match self {
            SubmsgIds::OpenCDP => 0,
            SubmsgIds::DepositToCDP => 1,
            SubmsgIds::MintAsset => 2,
            SubmsgIds::SellMAsset => 3,
            SubmsgIds::DepositOnReply => 4,
            SubmsgIds::Exit => 5,
            SubmsgIds::WithdrawCollateral => 6,
            SubmsgIds::RedeemStable => 7,
            SubmsgIds::BuyAsset => 8,
            SubmsgIds::BurnAsset => 9,
        }
    }
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}