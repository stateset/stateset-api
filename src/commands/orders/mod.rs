// Core order lifecycle commands (actively used and maintained)
pub mod add_item_to_order_command;
pub mod archive_order_command;
pub mod cancel_order_command;
pub mod create_order_command;
pub mod delete_order_command;
pub mod update_order_status_command;

// Re-export commands for easier access
pub use add_item_to_order_command::AddItemToOrderCommand;
pub use archive_order_command::ArchiveOrderCommand;
pub use cancel_order_command::CancelOrderCommand;
pub use create_order_command::CreateOrderCommand;
pub use delete_order_command::DeleteOrderCommand;
pub use update_order_status_command::UpdateOrderStatusCommand;

// Note: The following command modules exist but are not actively used.
// They may have broken imports or incomplete implementations:
// - add_order_note
// - add_tracking_information_command
// - apply_order_discount_command
// - apply_promotion_command
// - confirm_order_command
// - create_promotion_command
// - deactivate_promotion_command
// - delete_order_note_command
// - deliver_order_command
// - exchange_order_command
// - hold_order_command
// - merge_orders_command
// - order_routing_command
// - partial_cancel_order_command
// - refund_order_command
// - release_order_from_hold_command
// - remove_item_from_order_command
// - return_order_command
// - ship_order_command
// - split_order_command
// - tag_order_command
// - update_billing_address_command
// - update_order_items_command
// - update_order_note_command
// - update_payment_method_command
// - update_shipping_address_command
// - update_shipping_method_command
