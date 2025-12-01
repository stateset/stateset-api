//! Work Order Commands Module
//!
//! This module contains CQRS-style commands for general work order operations
//! (maintenance, repairs, asset management) using the UUID-based work order system.
//!
//! ## Important: Two Work Order Systems
//!
//! 1. **Manufacturing Work Orders** (i64-based)
//!    - For production operations
//!    - Use: `ManufacturingService` in `src/services/manufacturing.rs`
//!
//! 2. **General Work Orders** (UUID-based)
//!    - For maintenance and asset management
//!    - Use: `WorkOrderService` in `src/services/work_orders.rs`
//!
//! ## Architectural Note
//!
//! Many commands are disabled because functionality has been refactored into
//! service layer methods for better separation of concerns and testability.
//! See README.md in this directory for details.

// Active Commands - Currently in use
pub mod add_note_to_work_order_command;
pub mod assign_work_order_command;
pub mod calculate_average_cost_command;
pub mod calculate_cogs_command;
pub mod calculate_monthly_cogs_command;
pub mod calculate_weighted_average_cogs_command;
pub mod delete_work_order_command;
pub mod get_work_order_command;
pub mod list_work_orders;

// Disabled Commands - Refactored to WorkOrderService
// These commands have been superseded by service layer methods.
// To re-enable, update implementations and uncomment.
//
// pub mod cancel_work_order_command;        // → WorkOrderService::cancel()
// pub mod complete_work_order_command;      // → WorkOrderService::complete()
// pub mod create_work_order_command;        // → WorkOrderService::create_work_order()
// pub mod issue_work_order_command;         // → WorkOrderService methods
// pub mod pick_work_order_command;          // → WorkOrderService methods
// pub mod schedule_work_order_command;      // → WorkOrderService methods
// pub mod start_work_order_command;         // → WorkOrderService methods
// pub mod unassign_work_order_command;      // → WorkOrderService::unassign()
// pub mod update_work_order_command;        // → WorkOrderService::update_work_order()
// pub mod yield_work_order_command;         // → WorkOrderService methods

pub use add_note_to_work_order_command::AddNoteToWorkOrderCommand;
pub use assign_work_order_command::AssignWorkOrderCommand;
pub use calculate_average_cost_command::CalculateAverageCostCommand;
pub use calculate_cogs_command::CalculateCOGSCommand;
pub use calculate_monthly_cogs_command::CalculateMonthlyCOGSCommand;
pub use calculate_weighted_average_cogs_command::CalculateWeightedAverageCOGSCommand;
// pub use cancel_work_order_command::CancelWorkOrderCommand;
// pub use complete_work_order_command::CompleteWorkOrderCommand;
// pub use create_work_order_command::CreateWorkOrderCommand;
pub use delete_work_order_command::DeleteWorkOrderCommand;
pub use get_work_order_command::GetWorkOrderCommand;
// pub use issue_work_order_command::IssueWorkOrderCommand;
pub use list_work_orders::ListWorkOrdersCommand;
// pub use pick_work_order_command::PickWorkOrderCommand;
// pub use schedule_work_order_command::ScheduleWorkOrderCommand;
// pub use start_work_order_command::StartWorkOrderCommand;
// pub use unassign_work_order_command::UnassignWorkOrderCommand;
// pub use update_work_order_command::UpdateWorkOrderCommand;
// pub use yield_work_order_command::YieldWorkOrderCommand;
