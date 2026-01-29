use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer, TransferWithSeedBumps};

declare_id!("22222222222222222222222222222222222222222222");

#[program]
pub mod blueshift_anchor_vault {
    use super::*;

    pub fn deposit(ctx: Context<VaultAction>, amount: u64) -> Result<()> {
        require_eq!(ctx.accounts.vault.lamports(), 0, VaultError::VaultAlreadyExists);
        require_gt!(amount, Rent::get()?.minimum_balance(0), VaultError::InvalidAmount);
        ctx.accounts.deposit(amount)
    }

    pub fn withdraw(ctx: Context<VaultAction>) -> Result<()> {
        require_neq!(ctx.accounts.vault.lamports(), 0, VaultError::InvalidAmount);
        ctx.accounts.withdraw(ctx.bumps.vault)
    }
}

#[derive(Accounts)]
pub struct VaultAction<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", signer.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> VaultAction<'info> {
    fn deposit(&mut self, amount: u64) -> Result<()> {
        transfer(
            CpiContext::new(
                self.system_program.to_account_info(), 
                Transfer {
                    from: self.signer.to_account_info(),
                    to: self.vault.to_account_info(),
                },
            ),
            amount
        )
    }

    fn withdraw(&mut self, bump: u8) -> Result<()> {
        let signer_seeds = [
            b"vault",
            self.signer.key.as_ref(),
            &[bump]
        ];
        transfer(
            CpiContext::new_with_signer(
                self.system_program.to_account_info(), 
                Transfer {
                    from: self.vault.to_account_info(),
                    to: self.signer.to_account_info(),
                }, 
                &[&signer_seeds],
            ),
            self.vault.lamports()
        )
    }
}

#[error_code]
pub enum VaultError {
    #[msg("Vault already exists")]
    VaultAlreadyExists,
    #[msg("Invalid amount")]
    InvalidAmount,
}
