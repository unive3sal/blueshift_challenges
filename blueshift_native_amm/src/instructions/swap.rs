use core::mem::size_of;

use constant_product_curve::{ConstantProduct, LiquidityPair};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use pinocchio_token::{instructions::Transfer, state::TokenAccount};

use super::utils::{AssociatedTokenAccount, ConfigAccount, DataAccount, SignerAccount};
use crate::state::Config;
use crate::AmmState;

pub struct SwapAccounts<'a> {
    pub user: &'a AccountView,
    pub user_x_ata: &'a AccountView,
    pub user_y_ata: &'a AccountView,
    pub vault_x: &'a AccountView,
    pub vault_y: &'a AccountView,
    pub config: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for SwapAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [user, user_x_ata, user_y_ata, vault_x, vault_y, config, token_program] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(user)?;
        ConfigAccount::check(config)?;

        Ok(Self {
            user,
            user_x_ata,
            user_y_ata,
            vault_x,
            vault_y,
            config,
            token_program,
        })
    }
}

#[repr(C, packed)]
pub struct SwapInstructionData {
    pub is_x: bool,
    pub amount: u64,
    pub min: u64,
    pub expiration: i64,
}

impl<'a> TryFrom<&'a [u8]> for SwapInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<SwapInstructionData>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(unsafe { (data.as_ptr() as *const Self).read() })
    }
}

pub struct Swap<'a> {
    pub accounts: SwapAccounts<'a>,
    pub instruction_data: SwapInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Swap<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = SwapAccounts::try_from(accounts)?;
        let instruction_data = SwapInstructionData::try_from(data)?;

        // Return the initialized struct
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}
impl<'a> Swap<'a> {
    pub const DISCRIMINATOR: &'a u8 = &3;

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

        if config_data.state() != AmmState::Initialized as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        // Deserialize the token accounts
        let vault_x = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_y)? };

        // Swap Calculations
        let mut curve = ConstantProduct::init(
            vault_x.amount(),
            vault_y.amount(),
            vault_x.amount(),
            config_data.fee(),
            None,
        )
        .map_err(|_| ProgramError::Custom(1))?;

        let p = match self.instruction_data.is_x {
            true => LiquidityPair::X,
            false => LiquidityPair::Y,
        };

        let swap_result = curve
            .swap(p, self.instruction_data.amount, self.instruction_data.min)
            .map_err(|_| ProgramError::Custom(1))?;

        // Check for correct values
        if swap_result.deposit == 0 || swap_result.withdraw == 0 {
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
        let signer_seeds = [Signer::from(&config_seeds)];

        if self.instruction_data.is_x {
            // User deposits X, receives Y
            Transfer {
                from: self.accounts.user_x_ata,
                to: self.accounts.vault_x,
                authority: self.accounts.user,
                amount: swap_result.deposit,
            }
            .invoke()?;

            Transfer {
                from: self.accounts.vault_y,
                to: self.accounts.user_y_ata,
                authority: self.accounts.config,
                amount: swap_result.withdraw,
            }
            .invoke_signed(&signer_seeds)?;
        } else {
            // User deposits Y, receives X
            Transfer {
                from: self.accounts.user_y_ata,
                to: self.accounts.vault_y,
                authority: self.accounts.user,
                amount: swap_result.deposit,
            }
            .invoke()?;

            Transfer {
                from: self.accounts.vault_x,
                to: self.accounts.user_x_ata,
                authority: self.accounts.config,
                amount: swap_result.withdraw,
            }
            .invoke_signed(&signer_seeds)?;
        }

        Ok(())
    }
}
