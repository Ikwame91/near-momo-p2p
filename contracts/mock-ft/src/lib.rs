//! TEST ONLY mock USDC: a NEP-141 fungible token with 6 decimals and OPEN minting, so tests and
//! testnet demos can give accounts mock USDC. Never deploy outside testnet / sandbox.
//! Built on near-contract-standards (https://github.com/near/near-sdk-rs, NEP-141 / NEP-145).

use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::{
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::U128;
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, PromiseOrValue};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockFt {
    token: FungibleToken,
}

#[near]
impl MockFt {
    #[init]
    pub fn new() -> Self {
        Self { token: FungibleToken::new(b"t".to_vec()) }
    }

    /// Open mint (test only): credits `amount` to `account_id`. Registering a NEW account costs
    /// storage, so the caller must attach at least `storage_balance_bounds().min` for it;
    /// otherwise anyone could drain this contract's NEAR by minting to many new accounts.
    #[payable]
    pub fn mint(&mut self, account_id: AccountId, amount: U128) {
        if !self.token.accounts.contains_key(&account_id) {
            let min = self.token.storage_balance_bounds().min;
            require!(
                env::attached_deposit() >= min,
                "attach the storage deposit (storage_balance_bounds.min) to mint to a new account"
            );
            self.token.internal_register_account(&account_id);
        }
        self.token.internal_deposit(&account_id, amount.0);
        near_contract_standards::fungible_token::events::FtMint {
            owner_id: &account_id,
            amount: amount,
            memo: Some("mock mint"),
        }
        .emit();
    }
}

#[near]
impl FungibleTokenCore for MockFt {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for MockFt {
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        let sender = sender_id.clone();
        let (used, burned) = self.token.internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        if burned > 0 {
            // Sender unregistered during the transfer: the refund is burned (NEP-141 events).
            near_contract_standards::fungible_token::events::FtBurn {
                owner_id: &sender,
                amount: U128(burned),
                memo: Some("refund burned: sender unregistered"),
            }
            .emit();
        }
        used.into()
    }
}

#[near]
impl StorageManagement for MockFt {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.token.internal_storage_unregister(force).is_some()
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenMetadataProvider for MockFt {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: "Mock USDC (test only)".to_string(),
            symbol: "mUSDC".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn storage_min(ft: &MockFt) -> NearToken {
        ft.storage_balance_bounds().min
    }

    #[test]
    #[should_panic(expected = "attach the storage deposit")]
    fn mint_to_new_account_without_storage_deposit_fails() {
        testing_env!(VMContextBuilder::new().predecessor_account_id(accounts(0)).build());
        let mut ft = MockFt::new();
        ft.mint(accounts(0), U128(1));
    }

    #[test]
    fn mint_to_existing_account_needs_no_deposit() {
        let mut ctx = VMContextBuilder::new();
        testing_env!(ctx.predecessor_account_id(accounts(0)).build());
        let mut ft = MockFt::new();
        let min = storage_min(&ft);
        testing_env!(ctx.attached_deposit(min).build());
        ft.mint(accounts(0), U128(1));
        testing_env!(ctx.attached_deposit(NearToken::from_yoctonear(0)).build());
        ft.mint(accounts(0), U128(2));
        assert_eq!(ft.ft_balance_of(accounts(0)).0, 3);
    }

    #[test]
    fn mint_and_transfer() {
        let mut ctx = VMContextBuilder::new();
        testing_env!(ctx.predecessor_account_id(accounts(0)).build());
        let mut ft = MockFt::new();
        let min = storage_min(&ft);
        testing_env!(ctx.attached_deposit(min).build());
        ft.mint(accounts(0), U128(1_000_000));
        ft.mint(accounts(1), U128(0));
        assert_eq!(ft.ft_metadata().decimals, 6);
        assert_eq!(ft.ft_balance_of(accounts(0)).0, 1_000_000);

        testing_env!(ctx
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1))
            .build());
        ft.ft_transfer(accounts(1), U128(250_000), None);
        assert_eq!(ft.ft_balance_of(accounts(0)).0, 750_000);
        assert_eq!(ft.ft_balance_of(accounts(1)).0, 250_000);
        assert_eq!(ft.ft_total_supply().0, 1_000_000);
    }
}
