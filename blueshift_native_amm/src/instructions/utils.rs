use core::mem::size_of;
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{instructions::InitializeMint2, state::Mint};
use pinocchio_token_2022::ID as TOKEN_2022_PROGRAM_ID;

use crate::state::Config;

const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;
const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;
const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

pub struct SignerAccount;

impl SignerAccount {
    pub fn check(account: &AccountView) -> ProgramResult {
        if !account.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

pub trait DataAccount {
    type T: Sized;

    fn check(account: &AccountView) -> ProgramResult;
    fn init(payer: &AccountView, account: &AccountView, seeds: &[Seed]) -> ProgramResult;
}

pub struct ConfigAccount;

impl DataAccount for ConfigAccount {
    type T = Config;

    fn init(payer: &AccountView, account: &AccountView, seeds: &[Seed]) -> ProgramResult {
        let space = size_of::<Self::T>();

        // Get required lamports for rent
        let lamports = Rent::get()?.try_minimum_balance(space)?;

        // Create signer with seeds slice
        let signer = [Signer::from(seeds)];

        // Create the account
        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }

    fn check(account: &AccountView) -> ProgramResult {
        let len = size_of::<Self::T>();

        if !account.owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        if account.data_len().ne(&len) {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}

pub struct MintInterface;

impl MintInterface {
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(ProgramError::InvalidAccountOwner);
            } else {
                if account.data_len().ne(&pinocchio_token::state::Mint::LEN) {
                    return Err(ProgramError::InvalidAccountOwner);
                }
            }
        } else {
            let data = account.try_borrow()?;

            if data.len().ne(&pinocchio_token::state::Mint::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(ProgramError::InvalidAccountOwner);
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR)
                {
                    return Err(ProgramError::InvalidAccountOwner);
                }
            }
        }

        Ok(())
    }

    pub fn init_if_need(
        account: &AccountView,
        payer: &AccountView,
        decimals: u8,
        mint_authoriy: &Address,
        freeze_authority: Option<&Address>,
        signers: &[Signer],
    ) -> ProgramResult {
        if let Err(_) = Self::check(account) {
            let mint_lamport = Rent::get()?.try_minimum_balance(Mint::LEN)?;
            CreateAccount {
                from: payer,
                to: account,
                lamports: mint_lamport,
                space: Mint::LEN as u64,
                owner: &pinocchio_token::ID,
            }
            .invoke_signed(signers)?;

            InitializeMint2 {
                mint: account,
                decimals: decimals,
                mint_authority: mint_authoriy,
                freeze_authority: freeze_authority,
            }
            .invoke()?;
        }

        Ok(())
    }
}

pub struct TokenInterface;

impl TokenInterface {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(ProgramError::InvalidAccountOwner);
            } else {
                if account
                    .data_len()
                    .ne(&pinocchio_token::state::TokenAccount::LEN)
                {
                    return Err(ProgramError::InvalidAccountOwner);
                }
            }
        } else {
            let data = account.try_borrow()?;

            if data.len().ne(&pinocchio_token::state::TokenAccount::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(ProgramError::InvalidAccountData);
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                    .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
                {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
        }

        Ok(())
    }
}

pub struct AssociatedTokenAccount;

impl AssociatedTokenAccount {
    pub fn check(
        account: &AccountView,
        authority: &Address,
        mint: &Address,
        token_program: &Address,
    ) -> Result<(), ProgramError> {
        TokenInterface::check(account)?;

        if Address::find_program_address(
            &[
                authority.as_array(),
                token_program.as_array(),
                mint.as_array(),
            ],
            &pinocchio_associated_token_account::ID,
        )
        .0
        .ne(account.address())
        {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(())
    }

    pub fn init(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        owner: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        Create {
            funding_account: payer,
            account,
            wallet: owner,
            mint,
            system_program,
            token_program,
        }
        .invoke()
    }

    pub fn init_if_needed(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        owner: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        match Self::check(
            account,
            payer.address(),
            mint.address(),
            token_program.address(),
        ) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner, system_program, token_program),
        }
    }
}
