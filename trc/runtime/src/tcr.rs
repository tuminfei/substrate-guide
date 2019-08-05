use super::token;
use parity_codec::{Decode, Encode};
use rstd::prelude::Vec;
use runtime_primitives::traits::{CheckedAdd, Hash, As};
use support::{decl_event, decl_module, decl_storage, ensure, StorageMap, StorageValue};
use system::ensure_signed;

pub trait Trait: timestamp::Trait + token::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Listing<U, V, W> {
    id: u32,
    data: Vec<u8>,
    deposit: U,
    owner: V,
    application_expiry: W,
    whitelisted: bool,
    challenge_id: u32,
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Challenge<T, U, V, W> {
    listing_hash: T,
    deposit: U,
    owner: V,
    voting_ends: W,
    resolved: bool,
    reward_pool: U,
    total_tokens: U,
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Vote<U> {
    value: bool,
    deposit: U,
    claimed: bool,
}

decl_event! {
    pub enum Event<T>
        where
            AccountId = <T as system::Trait>::AccountId,
            TokenBalance = <T as token::Trait>::TokenBalance,
            Hash = <T as system::Trait>::Hash,
    {
        Proposed(AccountId, Hash, TokenBalance),
        Challenged(AccountId, Hash, u32, TokenBalance),
        Voted(AccountId, u32, TokenBalance),
        Resolved(Hash, u32),
        Accepted(Hash),
        Rejected(Hash),
        Claimed(AccountId, u32),
    }
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Poll<T, U> {
    listing_hash: T,
    votes_for: U,
    votes_against: U,
    passed: bool,
}

decl_storage! {
    trait Store for Module<T: Trait> as Tcr {
        Owner get(owner) config(): T::AccountId;
        Admins get(admins): map T::AccountId => bool;
        MinDeposit get(min_deposit) config(): Option<T::TokenBalance>;
        ApplyStageLen get(apply_stage_len) config(): Option<T::Moment>;
        CommitStageLen get(commit_stage_len) config(): Option<T::Moment>;
        Listings get(listings): map T::Hash => Listing<T::TokenBalance, T::AccountId, T::Moment>;
        ListingCount get(listing_count): u32;
        ListingIndexHash get(index_hash): map u32 => T::Hash;
        PollNonce get(poll_nonce) config(): u32;
        Challenges get(challenges): map u32 => Challenge<T::Hash, T::TokenBalance, T::AccountId, T::Moment>;
        Polls get(polls): map u32 => Poll<T::Hash, T::TokenBalance>;
        Votes get(votes): map (u32, T::AccountId) => Vote<T::TokenBalance>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
        where
            origin: T::Origin
    {
        fn deposit_event<T>() = default;

        fn init(origin) {
            let sender = ensure_signed(origin)?;

            <token::Module<T>>::init(&sender)?;
            <Admins<T>>::insert(&sender, true);
        }

        fn propose(
            origin,
            data: Vec<u8>,
            #[compact] deposit: T::TokenBalance
        )
        {
            let sender = ensure_signed(origin)?;

            ensure!(data.len() <= 256, "listing data can not be more than 256 bytes");

            let min_deposit = Self::min_deposit().ok_or("min deposit not set")?;

            ensure!(deposit >= min_deposit, "deposit should be more than min_deposit");

            let now = <timestamp::Module<T>>::get();
            let apply_stage_len = Self::apply_stage_len().ok_or("apply stage lenth not set")?;
            let app_exp = now.checked_add(&apply_stage_len).ok_or("overflow")?;
            let hashed = <T as system::Trait>::Hashing::hash(&data);
            let listing_id = Self::listing_count();
            let listing = Listing {
                id: listing_id,
                data,
                deposit,
                owner: sender.clone(),
                whitelisted: false,
                challenge_id: 0,
                application_expiry: app_exp,
            };

            ensure!(!<Listings<T>>::exists(&hashed), "listing already exists");

            <token::Module<T>>::lock(&sender, deposit, &hashed)?;
            <ListingCount<T>>::put(listing_id + 1);
            <Listings<T>>::insert(&hashed, &listing);
            <ListingIndexHash<T>>::insert(&listing_id, &hashed);

            Self::deposit_event(RawEvent::Proposed(sender, hashed.clone(), deposit));
            runtime_io::print("listing create");
        }

        fn challenge(
            origin,
            listing_id: u32,
            #[compact] deposit: T::TokenBalance
        )
        {
            let sender = ensure_signed(origin)?;

            ensure!(<ListingIndexHash<T>>::exists(&listing_id), "listing not found");

            let listing_hash = Self::index_hash(&listing_id);
            let listing = Self::listings(&listing_hash);

            ensure!(listing.challenge_id == 0, "listing is already challenged");
            ensure!(listing.owner != sender, "you can not challenge your own listing");
            ensure!(deposit >= listing.deposit, "not enough deposit to challenge");

            let now = <timestamp::Module<T>>::get();
            let commit_stage_len = Self::commit_stage_len().ok_or("commit stage length not set")?;
            let voting_exp = now.checked_add(&commit_stage_len).ok_or("overflow")?;

            ensure!(listing.application_expiry > now, "apply stage length has passed");

            let challenge = Challenge {
                listing_hash,
                deposit,
                owner: sender.clone(),
                voting_ends: voting_exp,
                resolved: false,
                reward_pool: <T::TokenBalance as As<u64>>::sa(0),
                total_tokens: <T::TokenBalance as As<u64>>::sa(0),
            };
            let poll = Poll {
                listing_hash,
                votes_for: listing.deposit,
                votes_against: deposit,
                passed: false,
            };

            <token::Module<T>>::lock(&sender, deposit, &listing_hash)?;

            let poll_nonce = <PollNonce<T>>::get();

            <Challenges<T>>::insert(&poll_nonce, &challenge);
            <Polls<T>>::insert(&poll_nonce, &poll);
            <Listings<T>>::mutate(&listing_hash, |listing| listing.challenge_id = poll_nonce);
            <PollNonce<T>>::put(poll_nonce + 1);

            Self::deposit_event(RawEvent::Challenged(sender, listing_hash, poll_nonce, deposit));
            runtime_io::print("challenge created")
        }

        fn resolve(origin, listing_id: u32) {
            ensure!(<ListingIndexHash<T>>::exists(listing_id), "listing not found");

            let listing_hash = Self::index_hash(listing_id);
            let listing = Self::listings(listing_hash);
            let now = <timestamp::Module<T>>::get();
        }
    }
}
