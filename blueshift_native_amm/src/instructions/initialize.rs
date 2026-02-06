use core::mem::MaybeUninit;
use pinocchio::cpi::Seed;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use pinocchio_pubkey::derive_address;
use pinocchio_token::state::Mint;

use super::utils::*;
use crate::state::*;

struct InitializeAccounts<'a> {
    pub initializer: &'a AccountView,
    pub mint_lp: &'a AccountView,
    pub config: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [initializer, mint_lp, config, system_account, token_account, _] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(initializer)?;

        Ok(Self {
            initializer,
            mint_lp,
            config,
        })
    }
}

#[repr(C, packed)]
struct InitializeInstructionData {
    pub seed: u64,
    pub fee: u16,
    pub mint_x: [u8; 32],
    pub mint_y: [u8; 32],
    pub config_bump: [u8; 1],
    pub lp_bump: [u8; 1],
    pub authority: [u8; 32],
}

impl TryFrom<&[u8]> for InitializeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        const INITIALIZE_DATA_LEN_WITH_AUTHORITY: usize = size_of::<InitializeInstructionData>();
        const INITIALIZE_DATA_LEN: usize =
            INITIALIZE_DATA_LEN_WITH_AUTHORITY - size_of::<[u8; 32]>();

        match data.len() {
            INITIALIZE_DATA_LEN_WITH_AUTHORITY => {
                Ok(unsafe { (data.as_ptr() as *const Self).read_unaligned() })
            }
            INITIALIZE_DATA_LEN => {
                // If the authority is not present, we need to build the buffer and add it at the end before transmuting to the struct
                let mut raw: MaybeUninit<[u8; INITIALIZE_DATA_LEN_WITH_AUTHORITY]> =
                    MaybeUninit::uninit();
                let raw_ptr = raw.as_mut_ptr() as *mut u8;
                unsafe {
                    // Copy the provided data
                    core::ptr::copy_nonoverlapping(data.as_ptr(), raw_ptr, INITIALIZE_DATA_LEN);
                    // Add the authority to the end of the buffer
                    core::ptr::write_bytes(raw_ptr.add(INITIALIZE_DATA_LEN), 0, 32);
                    // Now transmute to the struct
                    Ok((raw.as_ptr() as *const Self).read_unaligned())
                }
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub struct Initialize<'a> {
    accounts: InitializeAccounts<'a>,
    instruction_data: InitializeInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from(
        (instruction_data, accounts): (&'a [u8], &'a [AccountView]),
    ) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let instruction_data = InitializeInstructionData::try_from(instruction_data)?;

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: &'a u8 = &0;

    pub fn process(&self) -> ProgramResult {
        let seed_binding = self.instruction_data.seed.to_le_bytes();

        if derive_address(
            &[
                b"config",
                &seed_binding,
                &self.instruction_data.mint_x,
                &self.instruction_data.mint_y,
                &self.instruction_data.config_bump,
            ],
            None,
            &crate::ID.as_array(),
        )
        .ne(self.accounts.config.address().as_array())
        {
            return Err(ProgramError::InvalidSeeds);
        }

        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(&seed_binding),
            Seed::from(&self.instruction_data.mint_x),
            Seed::from(&self.instruction_data.mint_y),
            Seed::from(&self.instruction_data.config_bump),
        ];

        ConfigAccount::init(
            self.accounts.initializer,
            self.accounts.config,
            &config_seeds,
        )?;

        let mut config_data = Config::load_mut(self.accounts.config)?;
        config_data.set_inner(
            self.instruction_data.seed,
            self.instruction_data.authority.into(),
            self.instruction_data.mint_x.into(),
            self.instruction_data.mint_y.into(),
            self.instruction_data.fee,
            self.instruction_data.config_bump,
        )?;

        let mint_lp_decimals = 1;
        MintInterface::init_if_need(
            self.accounts.mint_lp,
            self.accounts.initializer,
            mint_lp_decimals,
            self.accounts.config.address(),
            None,
        )?;

        Ok(())
    }
}
