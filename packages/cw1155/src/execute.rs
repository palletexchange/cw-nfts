use cosmwasm_std::{
    Addr, Attribute, BankMsg, Binary, CustomMsg, DepsMut, Empty, Env, MessageInfo, Response,
    StdError, StdResult, Storage, SubMsg, Uint128,
};
use cw2::set_contract_version;
use cw721::execute::migrate_version;
use cw_ownable::initialize_owner;
use cw_utils::Expiration;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::vec::IntoIter;

use crate::event::{
    ApproveAllEvent, ApproveEvent, BurnEvent, MintEvent, RevokeAllEvent, RevokeEvent,
    TransferEvent, UpdateDefaultUriEvent, UpdateMetadataBatchEvent, UpdateMetadataEvent,
};
use crate::msg::{Balance, CollectionInfo, Cw1155MintMsg, TokenAmount, TokenApproval, TokenUpdate};
use crate::receiver::Cw1155BatchReceiveMsg;
use crate::state::TokenInfo;
use crate::{
    error::Cw1155ContractError,
    msg::{Cw1155ExecuteMsg, Cw1155InstantiateMsg},
    receiver::Cw1155ReceiveMsg,
    state::Cw1155Config,
};

pub trait Cw1155Execute<
    // Metadata defined in NftInfo (used for mint).
    TMetadataExtension,
    // Defines for `CosmosMsg::Custom<T>` in response. Barely used, so `Empty` can be used.
    TCustomResponseMessage,
    // Message passed for updating metadata.
    TMetadataExtensionMsg,
    // Extension query message.
    TQueryExtensionMsg,
> where
    TMetadataExtension: Serialize + DeserializeOwned + Clone,
    TCustomResponseMessage: CustomMsg,
    TMetadataExtensionMsg: CustomMsg,
    TQueryExtensionMsg: Serialize + DeserializeOwned + Clone,
{
    fn instantiate(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        msg: Cw1155InstantiateMsg,
        contract_name: &str,
        contract_version: &str,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        set_contract_version(deps.storage, contract_name, contract_version)?;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        let collection_info = CollectionInfo {
            name: msg.name,
            symbol: msg.symbol,
        };
        config
            .collection_info
            .save(deps.storage, &collection_info)?;

        // store minter
        let minter = match msg.minter {
            Some(owner) => deps.api.addr_validate(&owner)?,
            None => info.sender,
        };
        initialize_owner(deps.storage, deps.api, Some(minter.as_ref()))?;

        // store total supply
        config.supply.save(deps.storage, &Uint128::zero())?;

        // store default base uri
        config
            .default_base_uri
            .save(deps.storage, &msg.default_uri)?;

        Ok(Response::default().add_attribute("minter", minter))
    }

    fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: Cw1155ExecuteMsg<TMetadataExtension, TMetadataExtensionMsg>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let env = ExecuteEnv { deps, env, info };
        match msg {
            // cw1155
            Cw1155ExecuteMsg::SendBatch {
                from,
                to,
                batch,
                msg,
            } => self.send_batch(env, from, to, batch, msg),
            Cw1155ExecuteMsg::MintBatch { recipient, msgs } => {
                self.mint_batch(env, recipient, msgs)
            }
            Cw1155ExecuteMsg::BurnBatch { from, batch } => self.burn_batch(env, from, batch),
            Cw1155ExecuteMsg::ApproveAll { operator, expires } => {
                self.approve_all(env, operator, expires)
            }
            Cw1155ExecuteMsg::RevokeAll { operator } => self.revoke_all(env, operator),

            // cw721
            Cw1155ExecuteMsg::Send {
                from,
                to,
                token_id,
                amount,
                msg,
            } => self.send(env, from, to, token_id, amount, msg),
            Cw1155ExecuteMsg::Mint { recipient, msg } => self.mint(env, recipient, msg),
            Cw1155ExecuteMsg::Burn {
                from,
                token_id,
                amount,
            } => self.burn(env, from, token_id, amount),
            Cw1155ExecuteMsg::Approve {
                spender,
                token_id,
                amount,
                expires,
            } => self.approve_token(env, spender, token_id, amount, expires),
            Cw1155ExecuteMsg::Revoke {
                spender,
                token_id,
                amount,
            } => self.revoke_token(env, spender, token_id, amount),
            Cw1155ExecuteMsg::UpdateOwnership(action) => Self::update_ownership(env, action),
            Cw1155ExecuteMsg::UpdateMetadata(update) => self.update_metadata(env, update),
            Cw1155ExecuteMsg::UpdateMetadataBatch { updates } => {
                self.update_metadata_batch(env, updates)
            }
            Cw1155ExecuteMsg::UpdateDefaultUri { uri } => self.update_default_base_uri(env, uri),

            Cw1155ExecuteMsg::Extension { .. } => unimplemented!(),
        }
    }

    fn migrate(
        &self,
        deps: DepsMut,
        _env: Env,
        _msg: Empty,
        contract_name: &str,
        contract_version: &str,
    ) -> Result<Response, Cw1155ContractError> {
        let response = Response::<Empty>::default();
        // migrate
        let response = migrate_version(deps.storage, contract_name, contract_version, response)?;
        Ok(response)
    }

    fn mint(
        &self,
        env: ExecuteEnv,
        recipient: String,
        msg: Cw1155MintMsg<TMetadataExtension>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();

        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let to = deps.api.addr_validate(&recipient)?;

        let mut rsp = Response::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            &info,
            None,
            Some(to),
            vec![TokenAmount {
                token_id: msg.token_id.to_string(),
                amount: msg.amount,
            }],
        )?;
        rsp = rsp.add_attributes(event);

        // store token info if not exist (if it is the first mint)
        if !config.tokens.has(deps.storage, &msg.token_id) {
            let token_info = TokenInfo {
                token_uri: msg.token_uri,
                extension: msg.extension,
            };
            config
                .tokens
                .save(deps.storage, &msg.token_id, &token_info)?;
        }

        Ok(rsp)
    }

    fn mint_batch(
        &self,
        env: ExecuteEnv,
        recipient: String,
        msgs: Vec<Cw1155MintMsg<TMetadataExtension>>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();

        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let to = deps.api.addr_validate(&recipient)?;

        let batch = msgs
            .iter()
            .map(|msg| {
                // store token info if not exist (if it is the first mint)
                if !config.tokens.has(deps.storage, &msg.token_id) {
                    let token_info = TokenInfo {
                        token_uri: msg.token_uri.clone(),
                        extension: msg.extension.clone(),
                    };
                    config
                        .tokens
                        .save(deps.storage, &msg.token_id, &token_info)?;
                }
                Ok(TokenAmount {
                    token_id: msg.token_id.to_string(),
                    amount: msg.amount,
                })
            })
            .collect::<StdResult<Vec<_>>>()?;

        let mut rsp = Response::default();
        let event = self.update_balances(&mut deps, &env, &info, None, Some(to), batch)?;
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn send(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        to: String,
        token_id: String,
        amount: Uint128,
        msg: Option<Binary>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            env,
            info,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };
        let to = deps.api.addr_validate(&to)?;

        let balance_update =
            self.verify_approval(deps.storage, &env, &info, &from, &token_id, amount)?;

        let mut rsp = Response::<TCustomResponseMessage>::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            &info,
            Some(from.clone()),
            Some(to.clone()),
            vec![TokenAmount {
                token_id: token_id.to_string(),
                amount: balance_update.amount,
            }],
        )?;
        rsp.attributes.extend(event);

        if let Some(msg) = msg {
            rsp.messages.push(SubMsg::new(
                Cw1155ReceiveMsg {
                    operator: info.sender.to_string(),
                    from: Some(from.to_string()),
                    amount,
                    token_id,
                    msg,
                }
                .into_cosmos_msg(&info, to)?,
            ));
        } else {
            // transfer funds along to recipient
            if !info.funds.is_empty() {
                let transfer_msg = BankMsg::Send {
                    to_address: to.to_string(),
                    amount: info.funds.to_vec(),
                };
                rsp.messages.push(SubMsg::new(transfer_msg));
            }
        }

        Ok(rsp)
    }

    fn send_batch(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        to: String,
        batch: Vec<TokenAmount>,
        msg: Option<Binary>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            env,
            info,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };
        let to = deps.api.addr_validate(&to)?;

        let batch = self.verify_approvals(deps.storage, &env, &info, &from, batch)?;

        let mut rsp = Response::<TCustomResponseMessage>::default();
        let event = self.update_balances(
            &mut deps,
            &env,
            &info,
            Some(from.clone()),
            Some(to.clone()),
            batch.to_vec(),
        )?;
        rsp.attributes.extend(event);

        if let Some(msg) = msg {
            rsp.messages.push(SubMsg::new(
                Cw1155BatchReceiveMsg {
                    operator: info.sender.to_string(),
                    from: Some(from.to_string()),
                    batch,
                    msg,
                }
                .into_cosmos_msg(&info, to)?,
            ));
        } else {
            // transfer funds along to recipient
            if !info.funds.is_empty() {
                let transfer_msg = BankMsg::Send {
                    to_address: to.to_string(),
                    amount: info.funds.to_vec(),
                };
                rsp.messages.push(SubMsg::new(transfer_msg));
            }
        }

        Ok(rsp)
    }

    fn burn(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        token_id: String,
        amount: Uint128,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };

        // whoever can transfer these tokens can burn
        let balance_update =
            self.verify_approval(deps.storage, &env, &info, &from, &token_id, amount)?;

        let mut rsp = Response::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            &info,
            Some(from),
            None,
            vec![TokenAmount {
                token_id,
                amount: balance_update.amount,
            }],
        )?;
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn burn_batch(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        batch: Vec<TokenAmount>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };

        let batch = self.verify_approvals(deps.storage, &env, &info, &from, batch)?;

        let mut rsp = Response::default();
        let event = self.update_balances(&mut deps, &env, &info, Some(from), None, batch)?;
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn approve_token(
        &self,
        env: ExecuteEnv,
        operator: String,
        token_id: String,
        approval_amount: Uint128,
        expiration: Option<Expiration>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, env } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();

        // reject expired data as invalid
        let expiration = expiration.unwrap_or_default();
        if expiration.is_expired(&env.block) {
            return Err(Cw1155ContractError::Expired {});
        }

        // validate approval amount
        if approval_amount.is_zero() {
            return Err(Cw1155ContractError::InvalidZeroAmount {});
        }

        // verify operator != owner
        if info.sender == operator {
            return Err(Cw1155ContractError::Unauthorized {
                reason: "Operator cannot be the owner".to_string(),
            });
        }

        // store the approval
        let operator = deps.api.addr_validate(&operator)?;
        config.token_approves.save(
            deps.storage,
            (&token_id, &info.sender, &operator),
            &TokenApproval {
                amount: approval_amount,
                expiration,
            },
        )?;

        let mut rsp = Response::default();

        let event = ApproveEvent::new(&info.sender, &operator, &token_id, approval_amount);
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn approve_all(
        &self,
        env: ExecuteEnv,
        operator: String,
        expires: Option<Expiration>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, env } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();

        if info.sender == operator {
            return Err(Cw1155ContractError::Unauthorized {
                reason: "Operator cannot be the owner".to_string(),
            });
        }

        // reject expired data as invalid
        let expires = expires.unwrap_or_default();
        if expires.is_expired(&env.block) {
            return Err(Cw1155ContractError::Expired {});
        }

        // set the operator for us
        let operator = deps.api.addr_validate(&operator)?;
        config
            .approves
            .save(deps.storage, (&info.sender, &operator), &expires)?;

        let mut rsp = Response::default();

        let event = ApproveAllEvent::new(&info.sender, &operator);
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn revoke_token(
        &self,
        env: ExecuteEnv,
        operator: String,
        token_id: String,
        amount: Option<Uint128>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        let operator = deps.api.addr_validate(&operator)?;

        // get prev approval amount to get valid revoke amount
        let prev_approval = config
            .token_approves
            .load(deps.storage, (&token_id, &info.sender, &operator))?;
        let revoke_amount = amount.unwrap_or(Uint128::MAX).min(prev_approval.amount);

        // remove or update approval
        if revoke_amount == prev_approval.amount {
            config
                .token_approves
                .remove(deps.storage, (&token_id, &info.sender, &operator));
        } else {
            config.token_approves.update(
                deps.storage,
                (&token_id, &info.sender, &operator),
                |prev| -> StdResult<_> {
                    let mut new_approval = prev.unwrap();
                    new_approval.amount = new_approval.amount.checked_sub(revoke_amount)?;
                    Ok(new_approval)
                },
            )?;
        }

        let mut rsp = Response::default();

        let event = RevokeEvent::new(&info.sender, &operator, &token_id, revoke_amount);
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    fn revoke_all(
        &self,
        env: ExecuteEnv,
        operator: String,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        let operator = deps.api.addr_validate(&operator)?;

        config
            .approves
            .remove(deps.storage, (&info.sender, &operator));

        let mut rsp = Response::default();

        let event = RevokeAllEvent::new(&info.sender, &operator);
        rsp = rsp.add_attributes(event);

        Ok(rsp)
    }

    /// When from is None: mint new tokens
    /// When to is None: burn tokens
    /// When both are Some: transfer tokens
    ///
    /// Make sure permissions are checked before calling this.
    fn update_balances(
        &self,
        deps: &mut DepsMut,
        env: &Env,
        info: &MessageInfo,
        from: Option<Addr>,
        to: Option<Addr>,
        tokens: Vec<TokenAmount>,
    ) -> Result<impl IntoIterator<Item = Attribute>, Cw1155ContractError> {
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        if let Some(from) = &from {
            for TokenAmount { token_id, amount } in tokens.iter() {
                if amount.is_zero() {
                    return Err(Cw1155ContractError::InvalidZeroAmount {});
                }
                config.balances.update(
                    deps.storage,
                    (from.clone(), token_id.to_string()),
                    |balance: Option<Balance>| -> StdResult<_> {
                        let mut new_balance = balance.unwrap();
                        new_balance.amount = new_balance.amount.checked_sub(*amount)?;
                        Ok(new_balance)
                    },
                )?;
            }
        }

        if let Some(to) = &to {
            for TokenAmount { token_id, amount } in tokens.iter() {
                if amount.is_zero() {
                    return Err(Cw1155ContractError::InvalidZeroAmount {});
                }
                config.balances.update(
                    deps.storage,
                    (to.clone(), token_id.to_string()),
                    |balance: Option<Balance>| -> StdResult<_> {
                        let mut new_balance: Balance = if let Some(balance) = balance {
                            balance
                        } else {
                            Balance {
                                owner: to.clone(),
                                amount: Uint128::zero(),
                                token_id: token_id.to_string(),
                            }
                        };

                        new_balance.amount = new_balance.amount.checked_add(*amount)?;
                        Ok(new_balance)
                    },
                )?;
            }
        }

        let event: IntoIter<Attribute> = if let Some(from) = &from {
            for TokenAmount { token_id, amount } in &tokens {
                if amount.is_zero() {
                    return Err(Cw1155ContractError::InvalidZeroAmount {});
                }
                // decrement token approvals from operator if different from balance owner
                if from != info.sender {
                    let mut approval = config
                        .token_approves
                        .load(deps.storage, (token_id, from, &info.sender))
                        .unwrap_or_default();
                    if approval.is_expired(env) {
                        return Err(Cw1155ContractError::Expired {});
                    }
                    if approval.amount <= *amount {
                        config
                            .token_approves
                            .remove(deps.storage, (token_id, from, &info.sender));
                    } else {
                        approval.amount = approval.amount.checked_sub(*amount)?;
                        config.token_approves.save(
                            deps.storage,
                            (token_id, from, &info.sender),
                            &approval,
                        )?;
                    }
                }

                // decrement tokens if burning
                if to.is_none() {
                    config.decrement_tokens(deps.storage, token_id, amount)?;
                }
            }

            if let Some(to) = &to {
                // verify sender != recipient
                if from == to {
                    return Err(Cw1155ContractError::Unauthorized {
                        reason: "Cannot send to self".to_string(),
                    });
                }
                // transfer
                TransferEvent::new(info, Some(from.clone()), to, tokens).into_iter()
            } else {
                // burn
                BurnEvent::new(info, Some(from.clone()), tokens).into_iter()
            }
        } else if let Some(to) = &to {
            // mint
            for TokenAmount { token_id, amount } in &tokens {
                if amount.is_zero() {
                    return Err(Cw1155ContractError::InvalidZeroAmount {});
                }
                config.increment_tokens(deps.storage, token_id, amount)?;
            }
            MintEvent::new(info, to, tokens).into_iter()
        } else {
            panic!("Invalid transfer: from and to cannot both be None")
        };

        Ok(event)
    }

    /// returns valid token amount if the sender can execute or is approved to execute
    fn verify_approval(
        &self,
        storage: &dyn Storage,
        env: &Env,
        info: &MessageInfo,
        owner: &Addr,
        token_id: &str,
        amount: Uint128,
    ) -> Result<TokenAmount, Cw1155ContractError> {
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        let operator = &info.sender;

        let balance_update = TokenAmount {
            token_id: token_id.to_string(),
            amount,
        };

        let owner_balance = config
            .balances
            .load(storage, (owner.clone(), token_id.to_string()))
            .unwrap_or_else(|_| Balance {
                owner: owner.clone(),
                amount: Uint128::zero(),
                token_id: token_id.to_string(),
            });

        // owner or all operator can execute
        if owner == operator || config.verify_all_approval(storage, env, owner, operator) {
            if owner_balance.amount < amount {
                return Err(Cw1155ContractError::NotEnoughTokens {
                    available: owner_balance.amount,
                    requested: amount,
                });
            }
            return Ok(balance_update);
        }

        // token operator can execute up to approved amount
        if let Some(token_approval) =
            self.get_active_token_approval(storage, env, owner, operator, token_id)
        {
            let available_amount = token_approval.amount.min(owner_balance.amount);
            if available_amount < amount {
                return Err(Cw1155ContractError::NotEnoughTokens {
                    available: available_amount,
                    requested: amount,
                });
            }
            return Ok(balance_update);
        }

        Err(StdError::not_found("approval").into())
    }

    /// returns valid token amounts if the sender can execute or is approved to execute on all provided tokens
    fn verify_approvals(
        &self,
        storage: &dyn Storage,
        env: &Env,
        info: &MessageInfo,
        owner: &Addr,
        tokens: Vec<TokenAmount>,
    ) -> Result<Vec<TokenAmount>, Cw1155ContractError> {
        tokens
            .iter()
            .map(|TokenAmount { token_id, amount }| {
                self.verify_approval(storage, env, info, owner, token_id, *amount)
            })
            .collect()
    }

    fn get_active_token_approval(
        &self,
        storage: &dyn Storage,
        env: &Env,
        owner: &Addr,
        operator: &Addr,
        token_id: &str,
    ) -> Option<TokenApproval> {
        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        match config
            .token_approves
            .load(storage, (token_id, owner, operator))
        {
            Ok(approval) => {
                if !approval.is_expired(env) {
                    Some(approval)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    fn update_ownership(
        env: ExecuteEnv,
        action: cw_ownable::Action,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, env } = env;
        let ownership =
            cw_ownable::update_ownership(deps.api, deps.storage, &env.block, &info.sender, action)?;
        Ok(Response::new().add_attributes(ownership.into_attributes()))
    }

    /// Allows creator to update onchain metadata and token uri.
    /// From `update_uri` on ERC-1155.
    fn update_metadata(
        &self,
        env: ExecuteEnv,
        update: TokenUpdate<TMetadataExtension>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let TokenUpdate {
            token_id,
            token_uri,
            metadata,
        } = update;

        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        let token_info = config.tokens.load(deps.storage, &token_id)?;

        config.update_token_metadata(
            deps.storage,
            &token_id,
            token_info.clone(),
            token_uri,
            metadata.clone(),
        )?;

        Ok(Response::new().add_attributes(UpdateMetadataEvent::new(
            &token_id,
            token_info.token_uri.clone(),
            metadata.is_some(),
        )))
    }

    /// Allows creator to update onchain metadata and token uri in batches.
    /// From `update_uri` on ERC-1155.
    fn update_metadata_batch(
        &self,
        env: ExecuteEnv,
        updates: Vec<TokenUpdate<TMetadataExtension>>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        if updates.is_empty() {
            return Err(Cw1155ContractError::EmptyUpdateRequest {});
        }

        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();

        let events = updates
            .into_iter()
            .map(
                |TokenUpdate {
                     token_id,
                     token_uri,
                     metadata,
                 }| {
                    let token_info = config.tokens.load(deps.storage, &token_id)?;
                    let (token_id, token_info) = config.update_token_metadata(
                        deps.storage,
                        &token_id,
                        token_info,
                        token_uri,
                        metadata.clone(),
                    )?;
                    Ok(UpdateMetadataEvent::new(
                        &token_id,
                        token_info.token_uri,
                        metadata.is_some(),
                    ))
                },
            )
            .collect::<Result<Vec<_>, Cw1155ContractError>>()?;

        Ok(Response::new().add_attributes(UpdateMetadataBatchEvent::new(events)))
    }

    /// Allows owner to update the default base uri.
    /// From `update_uri` on ERC-1155.
    fn update_default_base_uri(
        &self,
        env: ExecuteEnv,
        uri: Option<String>,
    ) -> Result<Response<TCustomResponseMessage>, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let config = Cw1155Config::<
            TMetadataExtension,
            TCustomResponseMessage,
            TMetadataExtensionMsg,
            TQueryExtensionMsg,
        >::default();
        config.default_base_uri.save(deps.storage, &uri)?;

        Ok(Response::new().add_attributes(UpdateDefaultUriEvent { default_uri: uri }))
    }
}

/// To mitigate clippy::too_many_arguments warning
pub struct ExecuteEnv<'a> {
    deps: DepsMut<'a>,
    env: Env,
    info: MessageInfo,
}
