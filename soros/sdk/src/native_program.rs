use crate::account::KeyedAccount;
use crate::pubkey::Pubkey;
use std;

/// Reasons a program might have rejected an instruction.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProgramError {
    /// The program instruction returned an error
    GenericError,

    /// The arguments provided to a program instruction where invalid
    InvalidArgument,

    /// An instruction resulted in an account with a negative balance
    /// The difference from InsufficientFundsForFee is that the transaction was executed by the
    /// contract
    ResultWithNegativeLamports,

    /// Program's instruction lamport balance does not equal the balance after the instruction
    UnbalancedInstruction,

    /// Program modified an account's program id
    ModifiedProgramId,

    /// Program spent the lamports of an account that doesn't belong to it
    ExternalAccountLamportSpend,

    /// Program modified the userdata of an account that doesn't belong to it
    ExternalAccountDataModified,

    /// An instruction's data contents was invalid
    InvalidInstructionData,

    /// An account's data contents was invalid
    InvalidAccountData,

    /// An account's data was too small
    AccountDataTooSmall,

    /// SystemInstruction::Assign was attempted on an account unowned by the system program
    AssignOfUnownedAccount,

    /// An account was referenced more than once in a single instruction
    DuplicateAccountIndex,

    /// CustomError allows on-chain programs to implement program-specific error types and see
    /// them returned by the Bitconch runtime. A CustomError may be any type that is serialized
    /// to a Vec of bytes, max length 32 bytes. Any CustomError Vec greater than this length will
    /// be truncated by the runtime.
    CustomError(Vec<u8>),
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "error")
    }
}
impl std::error::Error for ProgramError {}

// All native programs export a symbol named process()
pub const ENTRYPOINT: &str = "process";

// Native program ENTRYPOINT prototype
pub type Entrypoint = unsafe extern "C" fn(
    program_id: &Pubkey,
    keyed_accounts: &mut [KeyedAccount],
    data: &[u8],
    tick_height: u64,
) -> Result<(), ProgramError>;

// Convenience macro to define the native program entrypoint.  Supply a fn to this macro that
// conforms to the `Entrypoint` type signature.
#[macro_export]
macro_rules! soros_entrypoint(
    ($entrypoint:ident) => (
        #[no_mangle]
        pub extern "C" fn process(
            program_id: &Pubkey,
            keyed_accounts: &mut [KeyedAccount],
            data: &[u8],
            tick_height: u64
        ) -> Result<(), ProgramError> {
            $entrypoint(program_id, keyed_accounts, data, tick_height)
        }
    )
);
