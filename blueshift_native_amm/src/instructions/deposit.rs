use pinocchio::{cpi::{Seed, Signer}, error::ProgramError, AccountView, ProgramResult};
use pinocchio_token::instructions::{MintTo, Transfer};
use pinocchio_token::state::{
    Mint,
    TokenAccount,
};
use constant_product_curve::ConstantProduct;

use super::utils::*;
use crate::state::*;

use super::utils::{ConfigAccount, DataAccount, MintInterface, SignerAccount};
pub struct DepositAccounts<'a> {
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

impl<'a> TryFrom<&'a [AccountView]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [user, mint_lp, vault_x, vault_y, user_x_ata, user_y_ata, user_lp_ata, config, token_program, _] =
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

pub struct DepositInstructionData {
    pub amount: u64,
    pub max_x: u64,
    pub max_y: u64,
    pub expiration: i64,
}

impl<'a> TryFrom<&'a [u8]> for DepositInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<DepositInstructionData>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(unsafe { (data.as_ptr() as *const Self).read() })
    }
}

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub instruction_data: DepositInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;
        let instruction_data = DepositInstructionData::try_from(data)?;

        // Return the initialized struct
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

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

        // Deserialize the token accounts
        let mint_lp = unsafe { Mint::from_account_view_unchecked(self.accounts.mint_lp)? };
        let vault_x = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_y)? };
        
        // Grab the amounts to deposit
        let (x, y) = match mint_lp.supply() == 0 && vault_x.amount() == 0 && vault_y.amount() == 0 {
            true => (self.instruction_data.max_x, self.instruction_data.max_y),
            false => {
                let amounts = ConstantProduct::xy_deposit_amounts_from_l(
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
        if !(x <= self.instruction_data.max_x && y <= self.instruction_data.max_y) {
            return Err(ProgramError::InvalidArgument);
        }

        let config_seed_binding = config_data.seed().to_le_bytes();
        let config_bump_binding = config_data.config_bump();
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(self.accounts.user.address().as_ref()),
            Seed::from(&config_seed_binding),
            Seed::from(&config_bump_binding),
        ];
        let config_signer = Signer::from(&config_seeds);
        let deposit_signers = [config_signer];

        // transfer from user ATA to corresponding vault
        Transfer {
            from: self.accounts.user_x_ata,
            to: self.accounts.vault_x,
            authority: self.accounts.config,
            amount: x,
        }.invoke_signed(&deposit_signers)?;
        Transfer {
            from: self.accounts.user_y_ata,
            to: self.accounts.vault_y,
            authority: self.accounts.config,
            amount: y,
        }.invoke_signed(&deposit_signers)?;

        // mint lp token
        let mint_lp_signers = deposit_signers;
        MintTo {
            mint: self.accounts.mint_lp,
            account: self.accounts.user_lp_ata,
            mint_authority: self.accounts.config,
            amount: self.instruction_data.amount,
        }.invoke_signed(&mint_lp_signers)?;

        Ok(())
    }
}
