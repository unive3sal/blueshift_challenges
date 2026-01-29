use {
    num_derive::FromPrimitive,
    pinocchio::error::{ProgramError, ToStr},
    thiserror::Error,
};

#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum PinocchioError {
    // 0
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,

    /// 1
    /// Miss signer
    #[error("Instruction miss a valid signer")]
    NotSigner,

    /// 2
    /// Account ownership mismatch
    #[error("Account ownership mismatch")]
    InvalidOwner,

    /// 3
    /// Account data field is invalid
    #[error("Account data field is invalid")]
    InvalidAccountData,

    /// 4
    /// PDA mismatch
    #[error("PDA mismatch")]
    InvalidAddress,
}

impl From<PinocchioError> for ProgramError {
    fn from(e: PinocchioError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl TryFrom<u32> for PinocchioError {
    type Error = ProgramError;
    fn try_from(error: u32) -> Result<Self, Self::Error> {
        match error {
            0 => Ok(PinocchioError::NotRentExempt),
            1 => Ok(PinocchioError::NotSigner),
            2 => Ok(PinocchioError::InvalidOwner),
            3 => Ok(PinocchioError::InvalidAccountData),
            4 => Ok(PinocchioError::InvalidAddress),
            _ => Err(ProgramError::InvalidArgument),
        }
    }
}

impl ToStr for PinocchioError {
    fn to_str(&self) -> &'static str {
        match self {
            PinocchioError::NotRentExempt => "Error: Lamport balance below rent-exempt threshold",
            PinocchioError::NotSigner => "Error: Instruction miss a valid signer",
            PinocchioError::InvalidOwner => "Error: Account ownership mismatch",
            PinocchioError::InvalidAccountData => "Error: Account data field is invalid",
            PinocchioError::InvalidAddress => "Error: PDA mismatch",
        }
    }
}
