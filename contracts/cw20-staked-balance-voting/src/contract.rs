#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult, SubMsg,
    Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::parse_reply_instantiate_data;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, StakingInfo, TokenInfo};
use crate::state::{
    STAKING_CONTRACT, STAKING_CONTRACT_CODE_ID, STAKING_CONTRACT_UNSTAKING_DURATION, TOKEN,
};

const CONTRACT_NAME: &str = "crates.io:cw20-staked-balance-voting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_TOKEN_REPLY_ID: u64 = 0;
const INSTANTIATE_STAKING_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    match msg.token_info {
        TokenInfo::Existing {
            address,
            staking_contract,
        } => {
            let address = deps.api.addr_validate(&address)?;
            TOKEN.save(deps.storage, &address)?;

            match staking_contract {
                StakingInfo::Existing {
                    staking_contract_address,
                } => {
                    let staking_contract_address =
                        deps.api.addr_validate(&staking_contract_address)?;
                    let resp: stake_cw20::msg::GetConfigResponse = deps.querier.query_wasm_smart(
                        &staking_contract_address,
                        &stake_cw20::msg::QueryMsg::GetConfig {},
                    )?;

                    if address != resp.token_address {
                        return Err(ContractError::StakingContractMismatch {});
                    }

                    STAKING_CONTRACT.save(deps.storage, &staking_contract_address)?;
                    Ok(Response::default()
                        .add_attribute("action", "instantiate")
                        .add_attribute("token", "existing_token")
                        .add_attribute("token_address", address)
                        .add_attribute("staking_contract", staking_contract_address))
                }
                StakingInfo::New {
                    staking_code_id,
                    unstaking_duration,
                } => {
                    let msg = WasmMsg::Instantiate {
                        code_id: staking_code_id,
                        funds: vec![],
                        admin: Some(env.contract.address.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&stake_cw20::msg::InstantiateMsg {
                            admin: Some(env.contract.address.to_string()),
                            unstaking_duration,
                            token_address: address.to_string(),
                        })?,
                    };
                    let msg = SubMsg::reply_on_success(msg, INSTANTIATE_STAKING_REPLY_ID);
                    Ok(Response::default()
                        .add_attribute("action", "instantiate")
                        .add_attribute("token", "existing_token")
                        .add_attribute("token_address", address)
                        .add_submessage(msg))
                }
            }
        }
        TokenInfo::New {
            code_id,
            label,
            name,
            symbol,
            decimals,
            initial_balances,
            marketing,
            staking_code_id,
            unstaking_duration,
        } => {
            let initial_supply = initial_balances
                .iter()
                .fold(Uint128::zero(), |p, n| p + n.amount);
            if initial_supply.is_zero() {
                return Err(ContractError::InitialBalancesError {});
            }

            STAKING_CONTRACT_CODE_ID.save(deps.storage, &staking_code_id)?;
            STAKING_CONTRACT_UNSTAKING_DURATION.save(deps.storage, &unstaking_duration)?;

            let msg = WasmMsg::Instantiate {
                admin: Some(info.sender.to_string()),
                code_id,
                msg: to_binary(&cw20_base::msg::InstantiateMsg {
                    name,
                    symbol,
                    decimals,
                    initial_balances,
                    mint: Some(cw20::MinterResponse {
                        minter: info.sender.to_string(),
                        cap: None,
                    }),
                    marketing,
                })?,
                funds: vec![],
                label,
            };
            let msg = SubMsg::reply_on_success(msg, INSTANTIATE_TOKEN_REPLY_ID);

            Ok(Response::default()
                .add_attribute("action", "instantiate")
                .add_attribute("token", "new_token")
                .add_submessage(msg))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {}
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TokenContract {} => query_token_contract(deps),
        QueryMsg::StakingContract {} => query_staking_contract(deps),
        QueryMsg::VotingPowerAtHeight { address, height: _ } => {
            query_voting_power_at_height(deps, env, address)
        }
        QueryMsg::TotalPowerAtHeight { height: _ } => query_total_power_at_height(deps, env),
        QueryMsg::Info {} => query_info(deps),
    }
}

pub fn query_token_contract(deps: Deps) -> StdResult<Binary> {
    let token = TOKEN.load(deps.storage)?;
    to_binary(&token)
}

pub fn query_staking_contract(deps: Deps) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    to_binary(&staking_contract)
}

pub fn query_voting_power_at_height(deps: Deps, env: Env, address: String) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    let address = deps.api.addr_validate(&address)?;
    let res: stake_cw20::msg::StakedBalanceAtHeightResponse = deps.querier.query_wasm_smart(
        staking_contract,
        &stake_cw20::msg::QueryMsg::StakedBalanceAtHeight {
            address: address.to_string(),
            height: Some(env.block.height),
        },
    )?;
    to_binary(
        &cw_governance_interface::voting::VotingPowerAtHeightResponse {
            power: res.balance,
            height: env.block.height,
        },
    )
}

pub fn query_total_power_at_height(deps: Deps, env: Env) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    let res: stake_cw20::msg::TotalStakedAtHeightResponse = deps.querier.query_wasm_smart(
        staking_contract,
        &stake_cw20::msg::QueryMsg::TotalStakedAtHeight {
            height: Some(env.block.height),
        },
    )?;
    to_binary(
        &cw_governance_interface::voting::TotalPowerAtHeightResponse {
            power: res.total,
            height: env.block.height,
        },
    )
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&cw_governance_interface::voting::InfoResponse { info })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_TOKEN_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    let token = TOKEN.may_load(deps.storage)?;
                    if token.is_some() {
                        return Err(ContractError::DuplicateToken {});
                    }
                    let token = deps.api.addr_validate(&res.contract_address)?;
                    TOKEN.save(deps.storage, &token)?;
                    let staking_contract_code_id = STAKING_CONTRACT_CODE_ID.load(deps.storage)?;
                    let unstaking_duration =
                        STAKING_CONTRACT_UNSTAKING_DURATION.load(deps.storage)?;
                    let msg = WasmMsg::Instantiate {
                        code_id: staking_contract_code_id,
                        funds: vec![],
                        admin: Some(env.contract.address.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&stake_cw20::msg::InstantiateMsg {
                            admin: Some(env.contract.address.to_string()),
                            unstaking_duration,
                            token_address: token.to_string(),
                        })?,
                    };
                    let msg = SubMsg::reply_on_success(msg, INSTANTIATE_STAKING_REPLY_ID);
                    Ok(Response::default()
                        .add_attribute("token_address", token)
                        .add_submessage(msg))
                }
                Err(_) => Err(ContractError::TokenInstantiateError {}),
            }
        }
        INSTANTIATE_STAKING_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    // Validate contract address
                    let staking_contract_addr = deps.api.addr_validate(&res.contract_address)?;

                    // Save staking contract addr
                    STAKING_CONTRACT.save(deps.storage, &staking_contract_addr)?;

                    Ok(Response::new().add_attribute("staking_contract", staking_contract_addr))
                }
                Err(_) => Err(ContractError::TokenInstantiateError {}),
            }
        }
        _ => Err(ContractError::UnknownReplyId { id: msg.id }),
    }
}
