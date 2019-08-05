use parity_codec::Codec;
use runtime_primitives::traits::{As, CheckedAdd, CheckedSub, Member, SimpleArithmetic};
use support::{
    decl_event, decl_module, decl_storage, dispatch, ensure, Parameter, StorageMap, StorageValue,
};
use system::ensure_signed;

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
        LockedDeposits get(locked_deposits): map T::Hash => T::TokenBalance;
    }
}

impl<T: Trait> Module<T> {
    pub fn init(sender: &T::AccountId) -> dispatch::Result {
        ensure!(!Self::is_init(), "already init");

        <BalanceOf<T>>::insert(sender, Self::total_supply());
        <Init<T>>::put(true);

        Ok(())
    }

    pub fn lock(
        from: &T::AccountId,
        value: T::TokenBalance,
        listing_hash: &T::Hash,
    ) -> dispatch::Result {
        let sender_balance = Self::balance_of(from)
            .checked_sub(&value)
            .ok_or("not enough balance")?;
        let deposit = Self::locked_deposits(listing_hash)
            .checked_add(&value)
            .ok_or("overflow")?;

        <BalanceOf<T>>::insert(from, sender_balance);
        <LockedDeposits<T>>::insert(listing_hash, deposit);

        Ok(())
    }

    pub fn unlock(
        to: &T::AccountId,
        value: T::TokenBalance,
        listing_hash: &T::Hash,
    ) -> dispatch::Result {
        let to_balance = Self::balance_of(to).checked_add(&value).ok_or("overflow")?;
        let deposit = Self::locked_deposits(listing_hash)
            .checked_sub(&value)
            .ok_or("overflow")?;

        <BalanceOf<T>>::insert(to, to_balance);
        <LockedDeposits<T>>::insert(listing_hash, deposit);

        Ok(())
    }

    fn int_transfer(
        from: T::AccountId,
        to: T::AccountId,
        value: T::TokenBalance,
    ) -> dispatch::Result {
        let from_balance = Self::balance_of(&from)
            .checked_sub(&value)
            .ok_or("not enough balance")?;
        let to_balance = Self::balance_of(&to)
            .checked_add(&value)
            .ok_or("overflow")?;
        <BalanceOf<T>>::insert(&from, from_balance);
        <BalanceOf<T>>::insert(&to, to_balance);

        Self::deposit_event(RawEvent::Transfer(from, to, value));

        Ok(())
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
        where
            origin: T::Origin
    {
        fn deposit_event<T>() = default;

        fn transfer(
            origin,
            to: T::AccountId,
            #[compact] value: T::TokenBalance
        )
        {
            let sender = ensure_signed(origin)?;

            Self::int_transfer(sender, to, value)?;
        }

        fn approve(
            origin,
            spender: T::AccountId,
            #[compact] value: T::TokenBalance
        )
        {
            let sender = ensure_signed(origin)?;
            let allowance = Self::allowance((sender.clone(), spender.clone()))
                .checked_add(&value)
                .ok_or("overflow")?;
            <Allowance<T>>::insert((sender.clone(), spender.clone()), allowance);

            Self::deposit_event(RawEvent::Approval(sender, spender, value));
        }

        fn transfer_from(
            origin,
            from: T::AccountId,
            to: T::AccountId,
            #[compact] value: T::TokenBalance
        )
        {
            let sender = ensure_signed(origin)?;

            ensure!(<Allowance<T>>::exists((from.clone(), sender.clone())), "allowance does not exist");

            let allowance = Self::allowance((from.clone(), sender.clone()))
                .checked_sub(&value)
                .ok_or("not enough allowance")?;

            <Allowance<T>>::insert((from.clone(), sender.clone()), allowance);

            Self::int_transfer(from, to, value)?;
        }
    }
}
