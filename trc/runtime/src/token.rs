use parity_codec::Codec;
use runtime_primitives::traits::{As, Member, SimpleArithmetic, CheckedSub, CheckedAdd};
use support::{decl_event, decl_module, decl_storage, ensure, Parameter, StorageMap, StorageValue};

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type TokenBalance: As<u64>
        + As<usize>
        + Codec
        + Copy
        + Default
        + Member
        + Parameter
        + SimpleArithmetic;
}

decl_event! {
    pub enum Event<T>
        where
            AccountId = <T as system::Trait>::AccountId,
            TokenBalance = <T as Trait>::TokenBalance,
    {
        Transfer(AccountId, AccountId, TokenBalance),
        Approval(AccountId, AccountId, TokenBalance),
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Token {
        Init get(is_init): bool;
        TotalSupply get(total_supply): T::TokenBalance;
        BalanceOf get(balance_of): map T::AccountId => T::TokenBalance;
        Allowance get(allowance): map (T::AccountId, T::AccountId) => T::TokenBalance;
        LockedDeposists get(locked_deposists): map T::Hash => T::TokenBalance;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
        where
            origin: T::Origin
    {
        fn deposit_event<T>() = default;

        fn init(sender: T::AccountId) {
            ensure!(!Self::is_init(), "already init");

            <BalanceOf<T>>::insert(sender, Self::total_supply());
            <Init<T>>::put(true);
        }

        fn lock(
            from: T::AccountId,
            value: T::TokenBalance,
            listing_hash: T::Hash
        )
        {
            let sender_balance = Self::balance_of(&from)
                .checked_sub(&value)
                .ok_or("not enough balance")?;
            let deposit = Self::locked_deposists(listing_hash)
                .checked_add(&value)
                .ok_or("overflow");

            <BalanceOf<T>>::insert(from, sender_balance);
            <LockedDeposits<T>>::insert(listing_hash, deposit);
        }
    }
}
