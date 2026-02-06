use core::mem::size_of;
use pinocchio::{
    account::{Ref, RefMut},
    error::ProgramError,
    AccountView, Address,
};

#[repr(C)]
pub struct Config {
    state: u8,
    seed: [u8; 8],
    authority: Address,
    mint_x: Address,
    mint_y: Address,
    fee: [u8; 2],
    config_bump: [u8; 1],
}

#[repr(u8)]
pub enum AmmState {
    Uninitialized = 0u8,
    Initialized = 1u8,
    Disabled = 2u8,
    WithdrawOnly = 3u8,
}

impl Config {
    const LEN: usize = size_of::<Self>();

    #[inline(always)]
    pub fn load(account_info: &AccountView) -> Result<Ref<Self>, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if !account_info.owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(Ref::map(account_info.try_borrow()?, |data| unsafe {
            Self::from_bytes_unchecked(data)
        }))
    }

    #[inline(always)]
    pub unsafe fn load_unchecked(account_info: &AccountView) -> Result<&Self, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(Self::from_bytes_unchecked(account_info.borrow_unchecked()))
    }

    /// Return a `Config` from the given bytes.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `bytes` contains a valid representation of `Config`, and
    /// it is properly aligned to be interpreted as an instance of `Config`.
    /// At the moment `Config` has an alignment of 1 byte.
    /// This method does not perform a length validation.
    #[inline(always)]
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const Config)
    }

    /// Return a mutable `Config` reference from the given bytes.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `bytes` contains a valid representation of `Config`.
    #[inline(always)]
    pub unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        &mut *(bytes.as_mut_ptr() as *mut Config)
    }

    // Getter methods for safe field access
    #[inline(always)]
    pub fn state(&self) -> u8 {
        self.state
    }

    #[inline(always)]
    pub fn seed(&self) -> u64 {
        u64::from_le_bytes(self.seed)
    }

    #[inline(always)]
    pub fn authority(&self) -> &Address {
        &self.authority
    }

    #[inline(always)]
    pub fn mint_x(&self) -> &Address {
        &self.mint_x
    }

    #[inline(always)]
    pub fn mint_y(&self) -> &Address {
        &self.mint_y
    }

    #[inline(always)]
    pub fn fee(&self) -> u16 {
        u16::from_le_bytes(self.fee)
    }

    #[inline(always)]
    pub fn config_bump(&self) -> [u8; 1] {
        self.config_bump
    }

    #[inline(always)]
    pub fn load_mut(account_info: &AccountView) -> Result<RefMut<Self>, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if !account_info.owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(RefMut::map(account_info.try_borrow_mut()?, |data| unsafe {
            Self::from_bytes_unchecked_mut(data)
        }))
    }

    #[inline(always)]
    pub fn set_state(&mut self, state: u8) -> Result<(), ProgramError> {
        if state.ge(&(AmmState::WithdrawOnly as u8)) {
            return Err(ProgramError::InvalidAccountData);
        }
        self.state = state as u8;
        Ok(())
    }

    #[inline(always)]
    pub fn set_fee(&mut self, fee: u16) -> Result<(), ProgramError> {
        if fee.ge(&10_000) {
            return Err(ProgramError::InvalidAccountData);
        }
        self.fee = fee.to_le_bytes();
        Ok(())
    }

    #[inline(always)]
    pub fn set_inner(
        &mut self,
        seed: u64,
        authority: Address,
        mint_x: Address,
        mint_y: Address,
        fee: u16,
        config_bump: [u8; 1],
    ) -> Result<(), ProgramError> {
        self.set_state(AmmState::Initialized as u8)?;
        self.set_seed(seed);
        self.set_authority(authority);
        self.set_mint_x(mint_x);
        self.set_mint_y(mint_y);
        self.set_fee(fee)?;
        self.set_config_bump(config_bump);
        Ok(())
    }

    #[inline(always)]
    pub fn has_authority(&self) -> Option<Address> {
        let bytes = self.authority();
        let chunks: &[u64; 4] = unsafe { &*(bytes.to_bytes().as_ptr() as *const [u64; 4]) };
        if chunks.iter().any(|&x| x != 0) {
            Some(self.authority)
        } else {
            None
        }
    }

    #[inline(always)]
    fn set_seed(&mut self, seed: u64) {
        self.seed = seed.to_le_bytes();
    }

    #[inline(always)]
    fn set_authority(&mut self, authority: Address) {
        self.authority = authority;
    }

    #[inline(always)]
    fn set_mint_x(&mut self, mint_x: Address) {
        self.mint_x = mint_x;
    }

    #[inline(always)]
    fn set_mint_y(&mut self, mint_y: Address) {
        self.mint_y = mint_y;
    }

    #[inline(always)]
    fn set_config_bump(&mut self, config_bump: [u8; 1]) {
        self.config_bump = config_bump;
    }
}
