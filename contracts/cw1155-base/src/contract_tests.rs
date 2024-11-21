#[cfg(test)]
mod tests {
    use crate::entry::{execute, instantiate, query};
    use crate::{Cw1155BaseContract, Cw1155BaseExecuteMsg, Cw1155BaseQueryMsg};
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        coin, from_json, to_json_binary, wasm_execute, Addr, Binary, Empty, OverflowError,
        Response, StdError, Uint128,
    };
    use cw1155::error::Cw1155ContractError;
    use cw1155::execute::Cw1155Execute;
    use cw1155::msg::{
        ApprovedForAllResponse, Balance, BalanceResponse, BalancesResponse, Cw1155InstantiateMsg,
        Cw1155MintMsg, Cw1155QueryMsg, DefaultBaseUriResponse, IsApprovedForAllResponse,
        NumTokensResponse, OwnerToken, TokenAmount, TokenApproval, TokenApprovalResponse,
        TokenInfoResponse, TokenUpdate,
    };
    use cw1155::query::Cw1155Query;
    use cw1155::receiver::Cw1155BatchReceiveMsg;
    use cw721::msg::TokensResponse;
    use cw721::Approval;
    use cw_multi_test::{App, AppBuilder, ContractWrapper, Executor};
    use cw_ownable::OwnershipError;
    use cw_utils::Expiration;

    const USEI: &str = "usei";

    struct TestSuite {
        pub app: App,
        pub address_book: AddressBook,
    }

    impl TestSuite {
        pub fn mint(
            &mut self,
            minter: Option<&Addr>,
            recipient: &Addr,
            token_id: &str,
            amount: u64,
            token_uri: Option<String>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                minter.unwrap_or(&self.address_book.creator).clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::Mint {
                        recipient: recipient.to_string(),
                        msg: Cw1155MintMsg {
                            token_id: token_id.to_string(),
                            amount: amount.into(),
                            token_uri,
                            extension: None,
                        },
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "mint succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error minting tokens: {:?}", res);
            }
        }

        pub fn send(
            &mut self,
            sender: &Addr,
            from: Option<&Addr>,
            to: &Addr,
            token_id: &str,
            amount: u64,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                sender.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::Send {
                        from: from.map(|f| f.to_string()),
                        to: to.to_string(),
                        token_id: token_id.to_string(),
                        amount: amount.into(),
                        msg: None,
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "send succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error sending tokens: {:?}", res);
            }
        }

        pub fn send_batch(
            &mut self,
            sender: &Addr,
            from: Option<&Addr>,
            to: &Addr,
            batch: Vec<TokenAmount>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                sender.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::SendBatch {
                        from: from.map(|f| f.to_string()),
                        to: to.to_string(),
                        batch,
                        msg: None,
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "send batch succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error sending token batch: {:?}", res);
            }
        }

        pub fn approve_token_spend(
            &mut self,
            owner: &Addr,
            spender: &Addr,
            token_id: &str,
            amount: u64,
            expires: Option<Expiration>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::Approve {
                        spender: spender.to_string(),
                        token_id: token_id.to_string(),
                        amount: amount.into(),
                        expires,
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "approve succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error approving token spend: {:?}", res);
            }
        }

        pub fn revoke_token_spend(
            &mut self,
            owner: &Addr,
            spender: &Addr,
            token_id: &str,
            amount: Option<u64>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::Revoke {
                        spender: spender.to_string(),
                        token_id: token_id.to_string(),
                        amount: amount.map(Into::into),
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "revoke succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error revoking token spend: {:?}", res);
            }
        }

        pub fn approve_all(
            &mut self,
            owner: &Addr,
            operator: &Addr,
            expires: Option<Expiration>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::ApproveAll {
                        operator: operator.to_string(),
                        expires,
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "approve all succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error approving all: {:?}", res);
            }
        }

        pub fn revoke_all(
            &mut self,
            owner: &Addr,
            operator: &Addr,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::RevokeAll {
                        operator: operator.to_string(),
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "revoke all succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error revoking all: {:?}", res);
            }
        }

        pub fn burn(
            &mut self,
            owner: &Addr,
            token_id: &str,
            amount: u64,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::Burn {
                        from: None,
                        token_id: token_id.to_string(),
                        amount: amount.into(),
                    },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "burn succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error burning tokens: {:?}", res);
            }
        }

        pub fn burn_batch(
            &mut self,
            owner: &Addr,
            batch: Vec<TokenAmount>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                owner.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::BurnBatch { from: None, batch },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "burn batch succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error burning token batch: {:?}", res);
            }
        }

        pub fn update_default_uri(
            &mut self,
            uri: Option<String>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                self.address_book.creator.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::UpdateDefaultUri { uri },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "update default uri succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error updating default uri: {:?}", res);
            }
        }

        pub fn update_token_metadata(
            &mut self,
            token_id: &str,
            token_uri: Option<String>,
            metadata: Option<String>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                self.address_book.creator.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::UpdateMetadata(TokenUpdate {
                        token_id: token_id.to_string(),
                        token_uri,
                        metadata,
                    }),
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "update token metadata succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(res.is_ok(), "error updating token metadata: {:?}", res);
            }
        }

        pub fn update_token_metadata_batch(
            &mut self,
            updates: Vec<TokenUpdate<String>>,
            expected_error: Option<Cw1155ContractError>,
        ) {
            let res = self.app.execute(
                self.address_book.creator.clone(),
                wasm_execute(
                    &self.address_book.cw1155,
                    &Cw1155BaseExecuteMsg::UpdateMetadataBatch { updates },
                    vec![],
                )
                .unwrap()
                .into(),
            );

            if let Some(expected_error) = expected_error {
                assert!(
                    res.is_err(),
                    "update token metadata batch succeeded but expected error: {}",
                    expected_error
                );
                res.expect_err(&expected_error.to_string());
            } else {
                assert!(
                    res.is_ok(),
                    "error updating token metadata batch: {:?}",
                    res
                );
            }
        }

        pub fn query_balance_of(&self, owner: &Addr, token_id: &str) -> BalanceResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::BalanceOf(OwnerToken {
                        owner: owner.to_string(),
                        token_id: token_id.to_string(),
                    }),
                )
                .unwrap()
        }

        pub fn query_balance_of_batch(&self, batch: Vec<OwnerToken>) -> BalancesResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::BalanceOfBatch(batch),
                )
                .unwrap()
        }

        pub fn query_token_approvals(
            &self,
            owner: &Addr,
            token_id: &str,
        ) -> Vec<TokenApprovalResponse> {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::TokenApprovals {
                        owner: owner.to_string(),
                        token_id: token_id.to_string(),
                        include_expired: None,
                    },
                )
                .unwrap()
        }

        pub fn query_num_tokens(&self, token_id: Option<&str>) -> NumTokensResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::NumTokens {
                        token_id: token_id.map(ToString::to_string),
                    },
                )
                .unwrap()
        }

        pub fn query_is_approved_for_all(
            &self,
            owner: &Addr,
            operator: &Addr,
        ) -> IsApprovedForAllResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::IsApprovedForAll {
                        owner: owner.to_string(),
                        operator: operator.to_string(),
                    },
                )
                .unwrap()
        }

        pub fn query_approvals_for_all(
            &self,
            owner: &Addr,
            include_expired: Option<bool>,
            start_after: Option<String>,
            limit: Option<u32>,
        ) -> ApprovedForAllResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::ApprovalsForAll {
                        owner: owner.to_string(),
                        include_expired,
                        start_after,
                        limit,
                    },
                )
                .unwrap()
        }

        pub fn query_token_info(&self, token_id: &str) -> TokenInfoResponse<String> {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::TokenInfo {
                        token_id: token_id.to_string(),
                    },
                )
                .unwrap()
        }

        pub fn query_default_uri(&self) -> DefaultBaseUriResponse {
            self.app
                .wrap()
                .query_wasm_smart(
                    &self.address_book.cw1155,
                    &Cw1155BaseQueryMsg::DefaultBaseUri {},
                )
                .unwrap()
        }
    }

    #[cw_serde]
    struct AddressBook {
        pub cw1155: Addr,
        pub creator: Addr,
        pub user1: Addr,
        pub user2: Addr,
        pub user3: Addr,
    }

    impl AddressBook {
        pub fn new(cw1155: &Addr, creator: &Addr) -> Self {
            AddressBook {
                cw1155: cw1155.clone(),
                creator: creator.clone(),
                user1: Addr::unchecked("user1"),
                user2: Addr::unchecked("user2"),
                user3: Addr::unchecked("user3"),
            }
        }
    }

    fn setup(default_uri: Option<String>) -> TestSuite {
        let mut app = AppBuilder::new().build(|router, _api, storage| {
            // init test accounts with 1_000_000_000 usei (1_000 sei)
            let funds = [coin(1_000_000_000, USEI)];
            router
                .bank
                .init_balance(storage, &Addr::unchecked("user1"), funds.to_vec())
                .unwrap();
            router
                .bank
                .init_balance(storage, &Addr::unchecked("user2"), funds.to_vec())
                .unwrap();
            router
                .bank
                .init_balance(storage, &Addr::unchecked("user3"), funds.to_vec())
                .unwrap();
        });

        // upload marketplace code
        let cw1155_code_id = app.store_code(Box::new(ContractWrapper::new_with_empty(
            execute,
            instantiate,
            query,
        )));

        let creator = Addr::unchecked("creator");

        // init marketplace contract
        let cw1155 = app
            .instantiate_contract(
                cw1155_code_id,
                creator.clone(),
                &Cw1155InstantiateMsg {
                    name: "cw1155 base contract".to_string(),
                    symbol: "CW1155".to_string(),
                    minter: None,
                    default_uri,
                },
                &[],
                "init cw1155",
                None,
            )
            .unwrap();

        let address_book = AddressBook::new(&cw1155, &creator);

        TestSuite { app, address_book }
    }

    #[test]
    fn check_transfers() {
        // A long test case that try to cover as many cases as possible.
        // Summary of what it does:
        // - try mint without permission, fail
        // - mint with permission, success
        // - query balance of recipient, success
        // - try transfer without approval, fail
        // - approve
        // - transfer again, success
        // - query balance of transfer participants
        // - try batch transfer without approval, fail
        // - approve and try batch transfer again, success
        // - batch query balances
        // - user1 revoke approval to minter
        // - query approval status
        // - minter try to transfer, fail
        // - user1 burn token1
        // - user1 batch burn token2 and token3

        let mut suite = setup(None);
        let AddressBook {
            creator,
            user1,
            user2,
            ..
        } = suite.address_book.clone();
        let token1 = "1";
        let token2 = "2";
        let token3 = "3";

        // invalid mint, user1 don't mint permission
        suite.mint(
            Some(&user1),
            &user1,
            token1,
            1,
            None,
            Some(Cw1155ContractError::Ownership(OwnershipError::NotOwner)),
        );

        // valid mint
        suite.mint(None, &user1, token1, 1, None, None);

        // verify supply

        assert_eq!(
            suite.query_num_tokens(Some(token1)),
            NumTokensResponse {
                count: Uint128::one()
            }
        );
        assert_eq!(
            suite.query_num_tokens(None),
            NumTokensResponse {
                count: Uint128::one()
            }
        );

        // query balance
        assert_eq!(
            BalanceResponse {
                balance: 1u64.into()
            },
            suite.query_balance_of(&user1, token1),
        );

        // not approved yet
        suite.send(
            &creator,
            Some(&user1),
            &user2,
            token1,
            1,
            Some(Cw1155ContractError::Std(StdError::not_found("approval"))),
        );

        // approve
        suite.approve_token_spend(&user1, &creator, token1, 1, None, None);

        // transfer
        suite.send(&creator, Some(&user1), &user2, token1, 1, None);

        // query balance
        assert_eq!(
            BalanceResponse {
                balance: 1u64.into()
            },
            suite.query_balance_of(&user2, token1),
        );
        assert_eq!(
            BalanceResponse {
                balance: 0u64.into()
            },
            suite.query_balance_of(&user1, token1),
        );

        // mint token2 and token3
        suite.mint(None, &user2, token2, 1, None, None);
        suite.mint(None, &user2, token3, 1, None, None);

        // verify supply
        assert_eq!(
            suite.query_num_tokens(Some(token2)),
            NumTokensResponse {
                count: Uint128::one()
            }
        );
        assert_eq!(
            suite.query_num_tokens(Some(token3)),
            NumTokensResponse {
                count: Uint128::one()
            }
        );
        assert_eq!(
            suite.query_num_tokens(None),
            NumTokensResponse {
                count: Uint128::new(3)
            }
        );

        // invalid batch transfer, (user2 not approved yet)
        suite.send_batch(
            &creator,
            Some(&user2),
            &user1,
            vec![
                TokenAmount {
                    token_id: token1.to_string(),
                    amount: 1u64.into(),
                },
                TokenAmount {
                    token_id: token2.to_string(),
                    amount: 1u64.into(),
                },
                TokenAmount {
                    token_id: token3.to_string(),
                    amount: 1u64.into(),
                },
            ],
            Some(Cw1155ContractError::Std(StdError::not_found("approval"))),
        );

        // user2 approve all
        suite.approve_all(&user2, &creator, None, None);

        // verify approval status
        assert_eq!(
            suite.query_is_approved_for_all(&user2, &creator),
            IsApprovedForAllResponse { approved: true }
        );

        // valid batch transfer
        suite.send_batch(
            &creator,
            Some(&user2),
            &user1,
            vec![
                TokenAmount {
                    token_id: token1.to_string(),
                    amount: 1u64.into(),
                },
                TokenAmount {
                    token_id: token2.to_string(),
                    amount: 1u64.into(),
                },
                TokenAmount {
                    token_id: token3.to_string(),
                    amount: 1u64.into(),
                },
            ],
            None,
        );

        // batch query
        assert_eq!(
            suite.query_balance_of_batch(vec![
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token1.to_string(),
                },
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token2.to_string(),
                },
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token3.to_string(),
                }
            ]),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token1.to_string(),
                        owner: Addr::unchecked(user1.to_string()),
                        amount: Uint128::one(),
                    },
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(user1.to_string()),
                        amount: Uint128::one(),
                    },
                    Balance {
                        token_id: token3.to_string(),
                        owner: Addr::unchecked(user1.to_string()),
                        amount: Uint128::one(),
                    }
                ]
            },
        );

        // user1 revoke approval
        suite.revoke_all(&user1, &creator, None);

        // query approval status
        assert_eq!(
            suite.query_approvals_for_all(&user1, None, None, None),
            ApprovedForAllResponse { operators: vec![] }
        );

        assert_eq!(
            suite.query_is_approved_for_all(&user1, &creator),
            IsApprovedForAllResponse { approved: false }
        );

        // transfer without approval
        suite.send(
            &creator,
            Some(&user1),
            &user2,
            token1,
            1,
            Some(Cw1155ContractError::Std(StdError::not_found("approval"))),
        );

        // burn token1
        suite.burn(&user1, token1, 1, None);

        // verify supply
        assert_eq!(
            suite.query_num_tokens(Some(token1)),
            NumTokensResponse {
                count: Uint128::zero()
            }
        );
        assert_eq!(
            suite.query_num_tokens(None),
            NumTokensResponse {
                count: Uint128::new(2)
            }
        );

        // verify balance
        assert_eq!(
            suite.query_balance_of(&user1, token1),
            BalanceResponse {
                balance: Uint128::zero()
            }
        );

        // burn them all
        suite.burn_batch(
            &user1,
            vec![
                TokenAmount {
                    token_id: token2.to_string(),
                    amount: 1u64.into(),
                },
                TokenAmount {
                    token_id: token3.to_string(),
                    amount: 1u64.into(),
                },
            ],
            None,
        );

        // verify supply
        assert_eq!(
            suite.query_num_tokens(Some(token2)),
            NumTokensResponse {
                count: Uint128::zero()
            }
        );
        assert_eq!(
            suite.query_num_tokens(Some(token3)),
            NumTokensResponse {
                count: Uint128::zero()
            }
        );
        assert_eq!(
            suite.query_num_tokens(None),
            NumTokensResponse {
                count: Uint128::zero()
            }
        );

        // verify balances
        assert_eq!(
            suite.query_balance_of_batch(vec![
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token2.to_string(),
                },
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token3.to_string(),
                }
            ]),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(user1.to_string()),
                        amount: Uint128::zero(),
                    },
                    Balance {
                        token_id: token3.to_string(),
                        owner: Addr::unchecked(user1.to_string()),
                        amount: Uint128::zero(),
                    }
                ]
            },
        );
    }

    #[test]
    fn check_send_contract() {
        let contract = Cw1155BaseContract::default();
        let receiver = String::from("receive_contract");
        let minter = String::from("minter");
        let user1 = String::from("user1");
        let token1 = "token1".to_owned();
        let token2 = "token2".to_owned();
        let operator_info = mock_info("operator", &[]);
        let dummy_msg = Binary::default();

        let mut deps = mock_dependencies();
        let msg = Cw1155InstantiateMsg {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            minter: Some(minter.to_string()),
            default_uri: None,
        };
        let res = contract
            .instantiate(
                deps.as_mut(),
                mock_env(),
                operator_info.clone(),
                msg,
                "contract_name",
                "contract_version",
            )
            .unwrap();
        assert_eq!(0, res.messages.len());

        contract
            .execute(
                deps.as_mut(),
                mock_env(),
                mock_info(minter.as_ref(), &[]),
                Cw1155BaseExecuteMsg::MintBatch {
                    recipient: user1.clone(),
                    msgs: vec![
                        Cw1155MintMsg {
                            token_id: token1.clone(),
                            amount: 5u64.into(),
                            token_uri: None,
                            extension: None,
                        },
                        Cw1155MintMsg {
                            token_id: token2.clone(),
                            amount: 5u64.into(),
                            token_uri: None,
                            extension: None,
                        },
                    ],
                },
            )
            .unwrap();
        contract
            .execute(
                deps.as_mut(),
                mock_env(),
                mock_info(minter.as_ref(), &[]),
                Cw1155BaseExecuteMsg::MintBatch {
                    recipient: receiver.clone(),
                    msgs: vec![Cw1155MintMsg {
                        token_id: token1.clone(),
                        amount: 1u64.into(),
                        token_uri: None,
                        extension: None,
                    }],
                },
            )
            .unwrap();

        // BatchSendFrom
        assert_eq!(
            contract
                .execute(
                    deps.as_mut(),
                    mock_env(),
                    mock_info(user1.as_ref(), &[]),
                    Cw1155BaseExecuteMsg::SendBatch {
                        from: Some(user1.clone()),
                        to: receiver.clone(),
                        batch: vec![TokenAmount {
                            token_id: token2.to_string(),
                            amount: 1u64.into(),
                        },],
                        msg: Some(dummy_msg.clone()),
                    },
                )
                .unwrap(),
            Response::new()
                .add_message(
                    Cw1155BatchReceiveMsg {
                        operator: user1.clone(),
                        from: Some(user1.clone()),
                        batch: vec![TokenAmount {
                            token_id: token2.to_string(),
                            amount: 1u64.into(),
                        }],
                        msg: dummy_msg.clone(),
                    }
                    .into_cosmos_msg(&operator_info, receiver.clone())
                    .unwrap()
                )
                .add_attributes(vec![
                    ("action", "transfer_single"),
                    ("owner", user1.as_str()),
                    ("sender", user1.as_str()),
                    ("recipient", receiver.as_str()),
                    ("token_id", token2.as_str()),
                    ("amount", "1"),
                ])
        );

        // verify balances
        assert_eq!(
            from_json::<BalancesResponse>(
                contract
                    .query(
                        deps.as_ref(),
                        mock_env(),
                        Cw1155BaseQueryMsg::BalanceOfBatch(vec![
                            OwnerToken {
                                owner: user1.clone(),
                                token_id: token2.clone(),
                            },
                            OwnerToken {
                                owner: receiver.clone(),
                                token_id: token2.clone(),
                            }
                        ]),
                    )
                    .unwrap()
            )
            .unwrap(),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(&user1),
                        amount: Uint128::new(4),
                    },
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(&receiver),
                        amount: Uint128::one(),
                    }
                ]
            },
        );

        // BatchSend
        assert_eq!(
            contract
                .execute(
                    deps.as_mut(),
                    mock_env(),
                    mock_info(user1.as_ref(), &[]),
                    Cw1155BaseExecuteMsg::SendBatch {
                        from: None,
                        to: receiver.clone(),
                        batch: vec![
                            TokenAmount {
                                token_id: token1.to_string(),
                                amount: 1u64.into(),
                            },
                            TokenAmount {
                                token_id: token2.to_string(),
                                amount: 1u64.into(),
                            },
                        ],
                        msg: Some(dummy_msg.clone()),
                    },
                )
                .unwrap(),
            Response::new()
                .add_message(
                    Cw1155BatchReceiveMsg {
                        operator: user1.clone(),
                        from: Some(user1.clone()),
                        batch: vec![
                            TokenAmount {
                                token_id: token1.to_string(),
                                amount: 1u64.into(),
                            },
                            TokenAmount {
                                token_id: token2.to_string(),
                                amount: 1u64.into(),
                            }
                        ],
                        msg: dummy_msg,
                    }
                    .into_cosmos_msg(&operator_info, receiver.clone())
                    .unwrap()
                )
                .add_attributes(vec![
                    ("action", "transfer_batch"),
                    ("owner", user1.as_str()),
                    ("sender", user1.as_str()),
                    ("recipient", receiver.as_str()),
                    (
                        "token_ids",
                        &format!("{},{}", token1.as_str(), token2.as_str())
                    ),
                    ("amounts", &format!("{},{}", 1, 1)),
                ])
        );

        // verify balances
        assert_eq!(
            from_json::<BalancesResponse>(
                contract
                    .query(
                        deps.as_ref(),
                        mock_env(),
                        Cw1155BaseQueryMsg::BalanceOfBatch(vec![
                            OwnerToken {
                                owner: user1.clone(),
                                token_id: token1.clone(),
                            },
                            OwnerToken {
                                owner: user1.clone(),
                                token_id: token2.clone(),
                            },
                            OwnerToken {
                                owner: receiver.clone(),
                                token_id: token1.clone(),
                            },
                            OwnerToken {
                                owner: receiver.clone(),
                                token_id: token2.clone(),
                            }
                        ]),
                    )
                    .unwrap()
            )
            .unwrap(),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token1.to_string(),
                        owner: Addr::unchecked(&user1),
                        amount: Uint128::new(4),
                    },
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(&user1),
                        amount: Uint128::new(3),
                    },
                    Balance {
                        token_id: token1.to_string(),
                        owner: Addr::unchecked(&receiver),
                        amount: Uint128::new(2),
                    },
                    Balance {
                        token_id: token2.to_string(),
                        owner: Addr::unchecked(&receiver),
                        amount: Uint128::new(2),
                    }
                ]
            },
        );
    }

    #[test]
    fn check_queries() {
        let contract = Cw1155BaseContract::default();
        // mint multiple types of tokens, and query them
        // grant approval to multiple operators, and query them
        let tokens = (0..10).map(|i| format!("token{}", i)).collect::<Vec<_>>();
        let users = (0..10).map(|i| format!("user{}", i)).collect::<Vec<_>>();
        let minter = String::from("minter");

        let mut deps = mock_dependencies();
        let msg = Cw1155InstantiateMsg {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            minter: Some(minter.to_string()),
            default_uri: None,
        };
        let res = contract
            .instantiate(
                deps.as_mut(),
                mock_env(),
                mock_info("operator", &[]),
                msg,
                "contract_name",
                "contract_version",
            )
            .unwrap();
        assert_eq!(0, res.messages.len());

        for token_id in tokens.clone() {
            contract
                .execute(
                    deps.as_mut(),
                    mock_env(),
                    mock_info(minter.as_ref(), &[]),
                    Cw1155BaseExecuteMsg::Mint {
                        recipient: users[0].clone(),
                        msg: Cw1155MintMsg {
                            token_id: token_id.clone(),
                            amount: 1u64.into(),
                            token_uri: None,
                            extension: None,
                        },
                    },
                )
                .unwrap();
        }

        for user in users[1..].iter() {
            contract
                .execute(
                    deps.as_mut(),
                    mock_env(),
                    mock_info(minter.as_ref(), &[]),
                    Cw1155BaseExecuteMsg::Mint {
                        recipient: user.clone(),
                        msg: Cw1155MintMsg {
                            token_id: tokens[9].clone(),
                            amount: 1u64.into(),
                            token_uri: None,
                            extension: None,
                        },
                    },
                )
                .unwrap();
        }

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::NumTokens {
                    token_id: Some(tokens[0].clone()),
                },
            ),
            to_json_binary(&NumTokensResponse {
                count: Uint128::new(1),
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::NumTokens {
                    token_id: Some(tokens[0].clone()),
                },
            ),
            to_json_binary(&NumTokensResponse {
                count: Uint128::new(1),
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::AllBalances {
                    token_id: tokens[9].clone(),
                    start_after: None,
                    limit: Some(5),
                },
            ),
            to_json_binary(&BalancesResponse {
                balances: users[..5]
                    .iter()
                    .map(|user| {
                        Balance {
                            owner: Addr::unchecked(user),
                            amount: Uint128::new(1),
                            token_id: tokens[9].clone(),
                        }
                    })
                    .collect(),
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::AllBalances {
                    token_id: tokens[9].clone(),
                    start_after: Some("user5".to_owned()),
                    limit: Some(5),
                },
            ),
            to_json_binary(&BalancesResponse {
                balances: users[6..]
                    .iter()
                    .map(|user| {
                        Balance {
                            owner: Addr::unchecked(user),
                            amount: Uint128::new(1),
                            token_id: tokens[9].clone(),
                        }
                    })
                    .collect(),
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::Tokens {
                    owner: users[0].clone(),
                    start_after: None,
                    limit: Some(5),
                },
            ),
            to_json_binary(&TokensResponse {
                tokens: tokens[..5].to_owned()
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::Tokens {
                    owner: users[0].clone(),
                    start_after: Some("token5".to_owned()),
                    limit: Some(5),
                },
            ),
            to_json_binary(&TokensResponse {
                tokens: tokens[6..].to_owned()
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::AllTokens {
                    start_after: Some("token5".to_owned()),
                    limit: Some(5),
                },
            ),
            to_json_binary(&TokensResponse {
                tokens: tokens[6..].to_owned()
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::TokenInfo {
                    token_id: "token5".to_owned()
                },
            ),
            to_json_binary(&TokenInfoResponse::<Option<Empty>> {
                token_uri: "".to_string(),
                extension: None,
            }),
        );

        for user in users[1..].iter() {
            contract
                .execute(
                    deps.as_mut(),
                    mock_env(),
                    mock_info(users[0].as_ref(), &[]),
                    Cw1155BaseExecuteMsg::ApproveAll {
                        operator: user.clone(),
                        expires: None,
                    },
                )
                .unwrap();
        }

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::ApprovalsForAll {
                    owner: users[0].clone(),
                    include_expired: None,
                    start_after: Some(String::from("user2")),
                    limit: Some(1),
                },
            ),
            to_json_binary(&ApprovedForAllResponse {
                operators: vec![Approval {
                    spender: Addr::unchecked(&users[3]),
                    expires: Expiration::Never {},
                }],
            })
        );

        assert_eq!(
            contract.query(
                deps.as_ref(),
                mock_env(),
                Cw1155QueryMsg::IsApprovedForAll {
                    owner: users[0].to_string(),
                    operator: users[3].to_string(),
                },
            ),
            to_json_binary(&IsApprovedForAllResponse { approved: true })
        );
    }

    #[test]
    fn approval_expires() {
        let contract = Cw1155BaseContract::default();
        let mut deps = mock_dependencies();
        let token1 = "token1".to_owned();
        let minter = String::from("minter");
        let user1 = String::from("user1");
        let user2 = String::from("user2");

        let env = {
            let mut env = mock_env();
            env.block.height = 10;
            env
        };

        let msg = Cw1155InstantiateMsg {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            minter: Some(minter.to_string()),
            default_uri: None,
        };
        let res = contract
            .instantiate(
                deps.as_mut(),
                env.clone(),
                mock_info("operator", &[]),
                msg,
                "contract_name",
                "contract_version",
            )
            .unwrap();
        assert_eq!(0, res.messages.len());

        contract
            .execute(
                deps.as_mut(),
                env.clone(),
                mock_info(minter.as_ref(), &[]),
                Cw1155BaseExecuteMsg::Mint {
                    recipient: user1.clone(),
                    msg: Cw1155MintMsg {
                        token_id: token1,
                        amount: 1u64.into(),
                        token_uri: None,
                        extension: None,
                    },
                },
            )
            .unwrap();

        // invalid expires should be rejected
        assert!(contract
            .execute(
                deps.as_mut(),
                env.clone(),
                mock_info(user1.as_ref(), &[]),
                Cw1155BaseExecuteMsg::ApproveAll {
                    operator: user2.clone(),
                    expires: Some(Expiration::AtHeight(5)),
                },
            )
            .is_err());

        contract
            .execute(
                deps.as_mut(),
                env,
                mock_info(user1.as_ref(), &[]),
                Cw1155BaseExecuteMsg::ApproveAll {
                    operator: user2.clone(),
                    expires: Some(Expiration::AtHeight(100)),
                },
            )
            .unwrap();

        let approvals: ApprovedForAllResponse = from_json(
            contract
                .query(
                    deps.as_ref(),
                    mock_env(),
                    Cw1155QueryMsg::ApprovalsForAll {
                        owner: user1.to_string(),
                        include_expired: None,
                        start_after: None,
                        limit: None,
                    },
                )
                .unwrap(),
        )
        .unwrap();
        assert!(approvals
            .operators
            .iter()
            .all(|approval| approval.spender == user2));

        let env = {
            let mut env = mock_env();
            env.block.height = 100;
            env
        };

        let approvals: ApprovedForAllResponse = from_json(
            contract
                .query(
                    deps.as_ref(),
                    env,
                    Cw1155QueryMsg::ApprovalsForAll {
                        owner: user1,
                        include_expired: None,
                        start_after: None,
                        limit: None,
                    },
                )
                .unwrap(),
        )
        .unwrap();
        assert!(
            approvals.operators.is_empty()
                || !approvals
                    .operators
                    .iter()
                    .all(|approval| approval.spender == user2)
        );
    }

    #[test]
    fn mint_overflow() {
        let contract = Cw1155BaseContract::default();
        let mut deps = mock_dependencies();
        let token1 = "token1".to_owned();
        let token2 = "token2".to_owned();
        let minter = String::from("minter");
        let user1 = String::from("user1");

        let env = mock_env();
        let msg = Cw1155InstantiateMsg {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            minter: Some(minter.to_string()),
            default_uri: None,
        };
        let res = contract
            .instantiate(
                deps.as_mut(),
                env.clone(),
                mock_info("operator", &[]),
                msg,
                "contract_name",
                "contract_version",
            )
            .unwrap();
        assert_eq!(0, res.messages.len());

        // minting up to max u128 should pass
        let res = contract.execute(
            deps.as_mut(),
            env.clone(),
            mock_info(minter.as_ref(), &[]),
            Cw1155BaseExecuteMsg::Mint {
                recipient: user1.clone(),
                msg: Cw1155MintMsg {
                    token_id: token1.clone(),
                    amount: u128::MAX.into(),
                    token_uri: None,
                    extension: None,
                },
            },
        );

        assert!(res.is_ok());

        // minting one more should fail
        let res = contract.execute(
            deps.as_mut(),
            env.clone(),
            mock_info(minter.as_ref(), &[]),
            Cw1155BaseExecuteMsg::Mint {
                recipient: user1.clone(),
                msg: Cw1155MintMsg {
                    token_id: token1,
                    amount: 1u128.into(),
                    token_uri: None,
                    extension: None,
                },
            },
        );

        assert!(matches!(
            res,
            Err(Cw1155ContractError::Std(StdError::Overflow {
                source: OverflowError { .. },
                ..
            }))
        ));

        // minting one more of a different token id should fail
        let res = contract.execute(
            deps.as_mut(),
            env,
            mock_info(minter.as_ref(), &[]),
            Cw1155BaseExecuteMsg::Mint {
                recipient: user1,
                msg: Cw1155MintMsg {
                    token_id: token2,
                    amount: 1u128.into(),
                    token_uri: None,
                    extension: None,
                },
            },
        );

        assert!(matches!(
            res,
            Err(Cw1155ContractError::Std(StdError::Overflow {
                source: OverflowError { .. },
                ..
            }))
        ));
    }

    #[test]
    fn token_uri() {
        let mut suite = setup(None);
        let AddressBook { user1, .. } = suite.address_book.clone();

        let token1 = "token1".to_owned();
        let token2 = "token2".to_owned();
        let token3 = "token3".to_string();
        let url1 = "url1".to_owned();
        let url2 = "url2".to_owned();
        let base_uri = "base_uri/".to_owned();

        // vrify base uri is not set
        assert_eq!(
            suite.query_default_uri(),
            DefaultBaseUriResponse {
                uri: "".to_string()
            }
        );

        // mint without token uri (should pass, would end up with no token uri set)
        suite.mint(None, &user1, &token1, 1, None, None);

        // verify token uri is none
        assert_eq!(
            suite.query_token_info(&token1),
            TokenInfoResponse::<String> {
                token_uri: "".to_string(),
                extension: None,
            }
        );

        // mint with token uri
        suite.mint(None, &user1, &token2, 1, Some(url1.to_string()), None);

        assert_eq!(
            suite.query_token_info(&token2),
            TokenInfoResponse::<String> {
                token_uri: url1.clone(),
                extension: None,
            }
        );

        // mint again without token uri (should use metadata from previous mint)
        suite.mint(None, &user1, &token2, 1, None, None);

        // verify token uri is still the same
        assert_eq!(
            suite.query_token_info(&token2),
            TokenInfoResponse::<String> {
                token_uri: url1.clone(),
                extension: None,
            }
        );

        // update base uri
        suite.update_default_uri(Some(base_uri.to_string()), None);

        // verify base uri is set
        assert_eq!(
            suite.query_default_uri(),
            DefaultBaseUriResponse {
                uri: base_uri.clone()
            }
        );

        // verify previous token uri still has the old url
        assert_eq!(
            suite.query_token_info(&token2),
            TokenInfoResponse::<String> {
                token_uri: url1.clone(),
                extension: None,
            }
        );

        // verify token 1 now uses base uri
        assert_eq!(
            suite.query_token_info(&token1),
            TokenInfoResponse::<String> {
                token_uri: format!("{}{}", base_uri, token1),
                extension: None,
            }
        );

        // mint without token uri (should use base uri)
        suite.mint(None, &user1, &token3, 1, None, None);

        // url should be base uri + token id
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: format!("{}{}", base_uri, token3),
                extension: None,
            }
        );

        // mint again with token uri (should be ignored, should use previous minted metadata)
        suite.mint(None, &user1, &token3, 1, Some(url2.to_string()), None);

        // verify token uri is still the same
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: format!("{}{}", base_uri, token3),
                extension: None,
            }
        );

        // update token uri for token3
        suite.update_token_metadata(&token3, Some(url2.to_string()), None, None);

        // verify token uri is updated
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: url2.clone(),
                extension: None,
            }
        );

        // remove token uri for token3
        suite.update_token_metadata(&token3, None, None, None);

        // verify token uri resolves to default again
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: format!("{}{}", base_uri, token3),
                extension: None,
            }
        );

        // remove base uri
        suite.update_default_uri(None, None);

        // verify token uri is none for token3
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: "".to_string(),
                extension: None,
            }
        );

        // update token uris in batch
        suite.update_token_metadata_batch(
            vec![
                TokenUpdate {
                    token_id: token1.clone(),
                    token_uri: Some(url2.to_string()),
                    metadata: None,
                },
                TokenUpdate {
                    token_id: token2.clone(),
                    token_uri: None,
                    metadata: None,
                },
                TokenUpdate {
                    token_id: token3.clone(),
                    token_uri: Some(url2.clone()),
                    metadata: None,
                },
            ],
            None,
        );

        // verify token uris are updated
        assert_eq!(
            suite.query_token_info(&token1),
            TokenInfoResponse::<String> {
                token_uri: url2.clone(),
                extension: None,
            }
        );
        assert_eq!(
            suite.query_token_info(&token2),
            TokenInfoResponse::<String> {
                token_uri: "".to_string(),
                extension: None,
            }
        );
        assert_eq!(
            suite.query_token_info(&token3),
            TokenInfoResponse::<String> {
                token_uri: url2.clone(),
                extension: None,
            }
        );
    }

    #[test]
    fn check_token_approvals() {
        let mut suite = setup(None);
        let AddressBook {
            user1,
            user2,
            user3,
            ..
        } = suite.address_book.clone();

        let token_id = "1";

        // mint tokens to user 1
        suite.mint(None, &user1, token_id, 10u64, None, None);

        // verify balance of owner 1
        assert_eq!(
            suite.query_balance_of(&user1, token_id),
            BalanceResponse {
                balance: 10u64.into()
            }
        );

        // user 2 tries to send tokens from user 1 to user 3 without approval, should fail
        suite.send(
            &user2,
            Some(&user1),
            &user3,
            token_id,
            1u64,
            Some(Cw1155ContractError::Std(StdError::not_found("approval"))),
        );

        // user 1 approves themselves (should fail)
        suite.approve_token_spend(
            &user1,
            &user1,
            token_id,
            1u64,
            None,
            Some(Cw1155ContractError::Unauthorized {
                reason: "Operator cannot be the owner".to_string(),
            }),
        );

        // user1 approves user2 with 0 amount (should fail)
        suite.approve_token_spend(
            &user1,
            &user2,
            token_id,
            0u64,
            None,
            Some(Cw1155ContractError::InvalidZeroAmount {}),
        );

        // user 1 approves user 2 with valid amount
        suite.approve_token_spend(&user1, &user2, token_id, 1u64, None, None);

        // verify user 1 balance is still same
        assert_eq!(
            suite.query_balance_of(&user1, token_id),
            BalanceResponse {
                balance: 10u64.into()
            }
        );

        // verify user 2 balance is 0
        assert_eq!(
            suite.query_balance_of(&user2, token_id),
            BalanceResponse {
                balance: 0u64.into()
            }
        );

        // user 2 approves user 3 even though they have no tokens
        suite.approve_token_spend(&user2, &user3, token_id, 1u64, None, None);

        // user 3 sends tokens from user 2 to themselves (should fail, user 2 has no tokens)
        suite.send(
            &user3,
            Some(&user2),
            &user3,
            token_id,
            1u64,
            Some(Cw1155ContractError::NotEnoughTokens {
                available: Uint128::zero(),
                requested: Uint128::one(),
            }),
        );

        // user 2 sends more tokens than approved to user 3 (should fail)
        suite.send(
            &user2,
            Some(&user1),
            &user3,
            token_id,
            2u64,
            Some(Cw1155ContractError::NotEnoughTokens {
                available: Uint128::one(),
                requested: Uint128::new(2),
            }),
        );

        // user 2 sends tokens to user 3 with user 1's balance
        suite.send(&user2, Some(&user1), &user3, token_id, 1u64, None);

        // verify user 1 balance is now 9
        assert_eq!(
            suite.query_balance_of(&user1, token_id),
            BalanceResponse {
                balance: 9u64.into()
            }
        );

        // verify user 2 balance is still 0
        assert_eq!(
            suite.query_balance_of(&user2, token_id),
            BalanceResponse {
                balance: 0u64.into()
            }
        );

        // verify user 3 balance is now 1
        assert_eq!(
            suite.query_balance_of(&user3, token_id),
            BalanceResponse {
                balance: 1u64.into()
            }
        );

        // user 2 sends tokens to user 3 with user 1's balance (should fail, user 2 used up their approval)
        suite.send(
            &user2,
            Some(&user1),
            &user3,
            token_id,
            1u64,
            Some(Cw1155ContractError::Std(StdError::not_found("approval"))),
        );

        // user 3 sends tokens to themselves from user 2 balance (should fail, user 2 has no tokens)
        suite.send(
            &user3,
            Some(&user2),
            &user3,
            token_id,
            1u64,
            Some(Cw1155ContractError::NotEnoughTokens {
                available: Uint128::zero(),
                requested: Uint128::one(),
            }),
        );

        // user 1 sends tokens to user 2
        suite.send(&user1, Some(&user1), &user2, token_id, 1u64, None);

        // verify balances
        assert_eq!(
            suite.query_balance_of_batch(vec![
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user2.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user3.to_string(),
                    token_id: token_id.to_string(),
                }
            ]),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user1.clone(),
                        amount: 8u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user2.clone(),
                        amount: 1u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user3.clone(),
                        amount: 1u64.into(),
                    }
                ]
            }
        );

        // user 3 sends more tokens to themselves than approved from user 2 balance (should fail, not approved up to requested amount)
        suite.send(
            &user3,
            Some(&user2),
            &user3,
            token_id,
            2u64,
            Some(Cw1155ContractError::NotEnoughTokens {
                available: Uint128::one(),
                requested: Uint128::new(2),
            }),
        );

        // user 3 sends tokens to themselves from user 2 balance with valid amount (should succeed this time)
        suite.send(&user3, Some(&user2), &user3, token_id, 1u64, None);

        // verify balances
        assert_eq!(
            suite.query_balance_of_batch(vec![
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user2.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user3.to_string(),
                    token_id: token_id.to_string(),
                }
            ]),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user1.clone(),
                        amount: 8u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user2.clone(),
                        amount: 0u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user3.clone(),
                        amount: 2u64.into(),
                    }
                ]
            }
        );

        // verify approvals (should all be used up)
        assert_eq!(suite.query_token_approvals(&user1, token_id), vec![]);
        assert_eq!(suite.query_token_approvals(&user2, token_id), vec![]);
        assert_eq!(suite.query_token_approvals(&user3, token_id), vec![]);

        // user 1 approves user 2 with amount 2
        suite.approve_token_spend(&user1, &user2, token_id, 2u64, None, None);

        // verify approvals
        assert_eq!(
            suite.query_token_approvals(&user1, token_id),
            vec![TokenApprovalResponse {
                operator: user2.clone(),
                approval: TokenApproval {
                    amount: 2u64.into(),
                    expiration: Expiration::Never {},
                }
            }]
        );

        // user 1 revokes 1 amount from user 2
        suite.revoke_token_spend(&user1, &user2, token_id, Some(1u64), None);

        // verify approvals (amount 1 should be revoked)
        assert_eq!(
            suite.query_token_approvals(&user1, token_id),
            vec![TokenApprovalResponse {
                operator: user2.clone(),
                approval: TokenApproval {
                    amount: 1u64.into(),
                    expiration: Expiration::Never {},
                }
            }]
        );

        // user 2 sends amount 2 to themselves from user 1 balance (should fail, not enough tokens after revoke)
        suite.send(
            &user2,
            Some(&user1),
            &user2,
            token_id,
            2u64,
            Some(Cw1155ContractError::NotEnoughTokens {
                available: Uint128::one(),
                requested: Uint128::new(2),
            }),
        );

        // user 2 sends amount 1 to user 1 from user 1 balance (should fail, cant send to self)
        suite.send(
            &user2,
            Some(&user1),
            &user1,
            token_id,
            1u64,
            Some(Cw1155ContractError::Unauthorized {
                reason: "Cannot send to self".to_string(),
            }),
        );

        // verify approvals
        assert_eq!(
            suite.query_token_approvals(&user1, token_id),
            vec![TokenApprovalResponse {
                operator: user2.clone(),
                approval: TokenApproval {
                    amount: 1u64.into(),
                    expiration: Expiration::Never {},
                }
            }]
        );

        // user 2 sends amount 1 to themselves from user 1 balance (should succeed)
        suite.send(&user2, Some(&user1), &user2, token_id, 1u64, None);

        // verify balances
        assert_eq!(
            suite.query_balance_of_batch(vec![
                OwnerToken {
                    owner: user1.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user2.to_string(),
                    token_id: token_id.to_string(),
                },
                OwnerToken {
                    owner: user3.to_string(),
                    token_id: token_id.to_string(),
                }
            ]),
            BalancesResponse {
                balances: vec![
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user1.clone(),
                        amount: 7u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user2.clone(),
                        amount: 1u64.into(),
                    },
                    Balance {
                        token_id: token_id.to_string(),
                        owner: user3.clone(),
                        amount: 2u64.into(),
                    }
                ]
            }
        );

        // verify approvals
        assert_eq!(suite.query_token_approvals(&user1, token_id), vec![]);
    }
}
