use constant_product_curve::ConstantProduct;
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, ProgramResult,
};
use pinocchio_token::instructions::{Burn, Transfer};
use pinocchio_token::state::{Mint, TokenAccount};

use super::utils::{
    AssociatedTokenAccount, ConfigAccount, DataAccount, MintInterface, SignerAccount,
};
use crate::state::*;

pub struct WithdrawAccounts<'a> {
    pub user: &'a AccountView,
    pub mint_lp: &'a AccountView,
    pub vault_x: &'a AccountView,
    pub vault_y: &'a AccountView,
    pub user_x_ata: &'a AccountView,
    pub user_y_ata: &'a AccountView,
    pub user_lp_ata: &'a AccountView,
    pub config: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [user, mint_lp, vault_x, vault_y, user_x_ata, user_y_ata, user_lp_ata, config, token_program] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(user)?;
        MintInterface::check(mint_lp)?;
        ConfigAccount::check(config)?;

        Ok(Self {
            user,
            mint_lp,
            vault_x,
            vault_y,
            user_x_ata,
            user_y_ata,
            user_lp_ata,
            config,
            token_program,
        })
    }
}

pub struct WithdrawInstructionData {
    pub amount: u64,
    pub min_x: u64,
    pub min_y: u64,
    pub expiration: i64,
}

impl<'a> TryFrom<&'a [u8]> for WithdrawInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<WithdrawInstructionData>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(unsafe { (data.as_ptr() as *const Self).read() })
    }
}

pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
    pub instruction_data: WithdrawInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = WithdrawAccounts::try_from(accounts)?;
        let instruction_data = WithdrawInstructionData::try_from(data)?;

        // Return the initialized struct
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Withdraw<'a> {
    pub const DISCRIMINATOR: &'a u8 = &2;

    pub fn process(&mut self) -> ProgramResult {
        let config_data = Config::load(self.accounts.config)?;
        AssociatedTokenAccount::check(
            self.accounts.vault_x,
            self.accounts.config.address(),
            config_data.mint_x(),
            self.accounts.token_program.address(),
        )?;
        AssociatedTokenAccount::check(
            self.accounts.vault_y,
            self.accounts.config.address(),
            config_data.mint_y(),
            self.accounts.token_program.address(),
        )?;
        AssociatedTokenAccount::check(
            self.accounts.user_x_ata,
            self.accounts.user.address(),
            config_data.mint_x(),
            self.accounts.token_program.address(),
        )?;
        AssociatedTokenAccount::check(
            self.accounts.user_y_ata,
            self.accounts.user.address(),
            config_data.mint_y(),
            self.accounts.token_program.address(),
        )?;
        AssociatedTokenAccount::check(
            self.accounts.user_lp_ata,
            self.accounts.user.address(),
            self.accounts.mint_lp.address(),
            self.accounts.token_program.address(),
        )?;

        if config_data.state() == AmmState::Disabled as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        let mint_lp = unsafe { Mint::from_account_view_unchecked(self.accounts.mint_lp)? };
        let vault_x = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_y)? };

        let (x, y) = match mint_lp.supply() == self.instruction_data.amount {
            true => (vault_x.amount(), vault_y.amount()),
            false => {
                let amounts = ConstantProduct::xy_withdraw_amounts_from_l(
                    vault_x.amount(),
                    vault_y.amount(),
                    mint_lp.supply(),
                    self.instruction_data.amount,
                    6,
                )
                .map_err(|_| ProgramError::InvalidArgument)?;

                (amounts.x, amounts.y)
            }
        };

        // Check for slippage
        if !(x >= self.instruction_data.min_x && y >= self.instruction_data.min_y) {
            return Err(ProgramError::InvalidArgument);
        }

        let config_seed_binding = config_data.seed().to_le_bytes();
        let config_bump_binding = config_data.config_bump();
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(&config_seed_binding),
            Seed::from(config_data.mint_x().as_array()),
            Seed::from(config_data.mint_y().as_array()),
            Seed::from(&config_bump_binding),
        ];
        let withdraw_signer = [Signer::from(&config_seeds)];

        Transfer {
            from: self.accounts.vault_x,
            to: self.accounts.user_x_ata,
            authority: self.accounts.config,
            amount: x,
        }
        .invoke_signed(&withdraw_signer)?;
        Transfer {
            from: self.accounts.vault_y,
            to: self.accounts.user_y_ata,
            authority: self.accounts.config,
            amount: y,
        }
        .invoke_signed(&withdraw_signer)?;

        Burn {
            account: self.accounts.user_lp_ata,
            mint: self.accounts.mint_lp,
            authority: self.accounts.user,
            amount: self.instruction_data.amount,
        }
        .invoke()?;

        Ok(())
    }
}
