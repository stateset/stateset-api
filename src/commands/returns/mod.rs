pub mod create_return_command;
pub mod cancel_return_command;
pub mod close_return_command;
pub mod delete_return_command;
pub mod complete_return_command;
pub mod refund_return_command;
pub mod approve_return_command;
pub mod reject_return_command;

// Re-export commands for easier access
pub use create_return_command::InitiateReturnCommand;
pub use cancel_return_command::CancelReturnCommand;
pub use close_return_command::CloseReturnCommand;
pub use delete_return_command::DeleteReturnCommand;
pub use complete_return_command::CompleteReturnCommand;
pub use refund_return_command::RefundReturnCommand;
pub use approve_return_command::ApproveReturnCommand;
pub use reject_return_command::RejectReturnCommand;
