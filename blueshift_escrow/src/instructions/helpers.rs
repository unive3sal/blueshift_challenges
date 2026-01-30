use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_pubkey::derive_address;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token_2022::ID as TOKEN_2022_PROGRAM_ID;

const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;
const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;
const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

use crate::errors::PinocchioError;

pub struct SignerAccount;

impl SignerAccount {
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.is_signer() {
            return Err(PinocchioError::NotSigner.into());
        }
        Ok(())
    }
}

pub struct MintInterface;

impl MintInterface {
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(PinocchioError::InvalidOwner.into());
            } else {
                if account.data_len().ne(&pinocchio_token::state::Mint::LEN) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        } else {
            let data = account.try_borrow()?;

            if data.len().ne(&pinocchio_token::state::Mint::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        }

        Ok(())
    }
}

pub struct TokenInterface;

impl TokenInterface {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(PinocchioError::InvalidOwner.into());
            } else {
                if account
                    .data_len()
                    .ne(&pinocchio_token::state::TokenAccount::LEN)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
            }
        } else {
            let data = account.try_borrow()?;

            if data.len().ne(&pinocchio_token::state::TokenAccount::LEN) {
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(PinocchioError::InvalidAccountData.into());
                }
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                    .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
                {
                    return Err(PinocchioError::InvalidAccountData.into());
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
        authority: &AccountView,
        mint: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError> {
        TokenInterface::check(account)?;

        if derive_address(
            &[
                authority.address().as_array(),
                token_program.address().as_array(),
                mint.address().as_array(),
            ],
            None,
            &pinocchio_associated_token_account::ID.as_array(),
        )
        .ne(account.address().as_array())
        {
            return Err(PinocchioError::InvalidAddress.into());
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
        match Self::check(account, payer, mint, token_program) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner, system_program, token_program),
        }
    }
}

pub struct ProgramAccount;

impl ProgramAccount {
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&crate::ID) {
            return Err(PinocchioError::InvalidOwner.into());
        }

        if account.data_len().ne(&crate::state::Escrow::LEN) {
            return Err(PinocchioError::InvalidAccountData.into());
        }

        Ok(())
    }

    pub fn init<'a, T: Sized>(
        payer: &AccountView,
        account: &AccountView,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
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

    pub fn close(account: &AccountView, destination: &AccountView) -> ProgramResult {
        {
            let mut data = account.try_borrow_mut()?;
            data[0] = 0xff;
        }

        destination.set_lamports(
            destination
                .lamports()
                .checked_add(account.lamports())
                .ok_or(ProgramError::ArithmeticOverflow)?,
        );
        account.resize(1)?;
        account.close()
    }
}
