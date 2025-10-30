/*!
 * # Permissions Module
 *
 * This module defines permissions for resources in the system.
 * Permissions are organized by resource type and action.
 */

use lazy_static::lazy_static;
use std::collections::HashMap;

/// Permission definition
#[derive(Debug, Clone)]
pub struct Permission {
    pub name: String,
    pub description: String,
    pub resource_type: String,
    pub action: String,
}

/// Permission actions
pub struct Actions;

impl Actions {
    pub const READ: &'static str = "read";
    pub const CREATE: &'static str = "create";
    pub const UPDATE: &'static str = "update";
    pub const DELETE: &'static str = "delete";
    pub const MANAGE: &'static str = "manage";
    pub const ALL: &'static str = "*";
}

/// Resource types
pub struct Resources;

impl Resources {
    pub const ORDERS: &'static str = "orders";
    pub const PRODUCTS: &'static str = "products";
    pub const INVENTORY: &'static str = "inventory";
    pub const BOMS: &'static str = "boms";
    pub const RETURNS: &'static str = "returns";
    pub const SHIPMENTS: &'static str = "shipments";
    pub const PURCHASE_ORDERS: &'static str = "purchaseorders";
    pub const ASNS: &'static str = "asns";
    pub const CUSTOMERS: &'static str = "customers";
    pub const SUPPLIERS: &'static str = "suppliers";
    pub const USERS: &'static str = "users";
    pub const ROLES: &'static str = "roles";
    pub const PERMISSIONS: &'static str = "permissions";
    pub const API_KEYS: &'static str = "api-keys";
    pub const REPORTS: &'static str = "reports";
    pub const METRICS: &'static str = "metrics";
    pub const ADMIN: &'static str = "admin";
    pub const SETTINGS: &'static str = "settings";
    pub const SYSTEM: &'static str = "system";
}

/// Common permission string constants for compile-time safety
pub mod consts {
    // Orders
    pub const ORDERS_READ: &str = "orders:read";
    pub const ORDERS_CREATE: &str = "orders:create";
    pub const ORDERS_UPDATE: &str = "orders:update";
    pub const ORDERS_DELETE: &str = "orders:delete";
    pub const ORDERS_CANCEL: &str = "orders:cancel";

    // Inventory
    pub const INVENTORY_READ: &str = "inventory:read";
    pub const INVENTORY_ADJUST: &str = "inventory:adjust";
    pub const INVENTORY_TRANSFER: &str = "inventory:transfer";

    // Returns
    pub const RETURNS_READ: &str = "returns:read";
    pub const RETURNS_CREATE: &str = "returns:create";
    pub const RETURNS_APPROVE: &str = "returns:approve";
    pub const RETURNS_REJECT: &str = "returns:reject";

    // Shipments
    pub const SHIPMENTS_READ: &str = "shipments:read";
    pub const SHIPMENTS_CREATE: &str = "shipments:create";
    pub const SHIPMENTS_UPDATE: &str = "shipments:update";
    pub const SHIPMENTS_DELETE: &str = "shipments:delete";

    // Warranties
    pub const WARRANTIES_READ: &str = "warranties:read";
    pub const WARRANTIES_CREATE: &str = "warranties:create";
    pub const WARRANTIES_UPDATE: &str = "warranties:update";
    pub const WARRANTIES_DELETE: &str = "warranties:delete";

    // Work orders (resource key is `workorders` in RBAC)
    pub const WORKORDERS_READ: &str = "workorders:read";
    pub const WORKORDERS_CREATE: &str = "workorders:create";
    pub const WORKORDERS_UPDATE: &str = "workorders:update";
    pub const WORKORDERS_DELETE: &str = "workorders:delete";

    // Manufacturing BOMs
    pub const BOMS_MANAGE: &str = "boms:manage";

    // Purchase Orders
    pub const PURCHASEORDERS_MANAGE: &str = "purchaseorders:manage";

    // Advanced Shipping Notices
    pub const ASNS_MANAGE: &str = "asns:manage";

    // Analytics & Metrics
    pub const ANALYTICS_READ: &str = "metrics:read";
}

/// Format a permission string
pub fn format_permission(resource: &str, action: &str) -> String {
    format!("{}:{}", resource, action)
}

// Permission set definition with descriptions
lazy_static! {
    pub static ref PERMISSIONS: HashMap<String, Permission> = {
        let mut perms = HashMap::new();

        // Orders permissions
        perms.insert(
            format_permission(Resources::ORDERS, Actions::READ),
            Permission {
                name: format_permission(Resources::ORDERS, Actions::READ),
                description: "View orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ORDERS, Actions::CREATE),
            Permission {
                name: format_permission(Resources::ORDERS, Actions::CREATE),
                description: "Create new orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: Actions::CREATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ORDERS, Actions::UPDATE),
            Permission {
                name: format_permission(Resources::ORDERS, Actions::UPDATE),
                description: "Update existing orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: Actions::UPDATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ORDERS, Actions::DELETE),
            Permission {
                name: format_permission(Resources::ORDERS, Actions::DELETE),
                description: "Delete orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: Actions::DELETE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ORDERS, "cancel"),
            Permission {
                name: format_permission(Resources::ORDERS, "cancel"),
                description: "Cancel orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: "cancel".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ORDERS, Actions::ALL),
            Permission {
                name: format_permission(Resources::ORDERS, Actions::ALL),
                description: "Full control over orders".to_string(),
                resource_type: Resources::ORDERS.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::BOMS, Actions::MANAGE),
            Permission {
                name: format_permission(Resources::BOMS, Actions::MANAGE),
                description: "Manage bill of materials".to_string(),
                resource_type: Resources::BOMS.to_string(),
                action: Actions::MANAGE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::PURCHASE_ORDERS, Actions::MANAGE),
            Permission {
                name: format_permission(Resources::PURCHASE_ORDERS, Actions::MANAGE),
                description: "Manage purchase orders".to_string(),
                resource_type: Resources::PURCHASE_ORDERS.to_string(),
                action: Actions::MANAGE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ASNS, Actions::MANAGE),
            Permission {
                name: format_permission(Resources::ASNS, Actions::MANAGE),
                description: "Manage advanced shipping notices".to_string(),
                resource_type: Resources::ASNS.to_string(),
                action: Actions::MANAGE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::METRICS, Actions::READ),
            Permission {
                name: format_permission(Resources::METRICS, Actions::READ),
                description: "View analytics dashboards and metrics".to_string(),
                resource_type: Resources::METRICS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        // Inventory permissions
        perms.insert(
            format_permission(Resources::INVENTORY, Actions::READ),
            Permission {
                name: format_permission(Resources::INVENTORY, Actions::READ),
                description: "View inventory".to_string(),
                resource_type: Resources::INVENTORY.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::INVENTORY, "adjust"),
            Permission {
                name: format_permission(Resources::INVENTORY, "adjust"),
                description: "Adjust inventory levels".to_string(),
                resource_type: Resources::INVENTORY.to_string(),
                action: "adjust".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::INVENTORY, "transfer"),
            Permission {
                name: format_permission(Resources::INVENTORY, "transfer"),
                description: "Transfer inventory between locations".to_string(),
                resource_type: Resources::INVENTORY.to_string(),
                action: "transfer".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::INVENTORY, Actions::ALL),
            Permission {
                name: format_permission(Resources::INVENTORY, Actions::ALL),
                description: "Full control over inventory".to_string(),
                resource_type: Resources::INVENTORY.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        // Returns permissions
        perms.insert(
            format_permission(Resources::RETURNS, Actions::READ),
            Permission {
                name: format_permission(Resources::RETURNS, Actions::READ),
                description: "View returns".to_string(),
                resource_type: Resources::RETURNS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::RETURNS, Actions::CREATE),
            Permission {
                name: format_permission(Resources::RETURNS, Actions::CREATE),
                description: "Create new returns".to_string(),
                resource_type: Resources::RETURNS.to_string(),
                action: Actions::CREATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::RETURNS, "approve"),
            Permission {
                name: format_permission(Resources::RETURNS, "approve"),
                description: "Approve returns".to_string(),
                resource_type: Resources::RETURNS.to_string(),
                action: "approve".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::RETURNS, "reject"),
            Permission {
                name: format_permission(Resources::RETURNS, "reject"),
                description: "Reject returns".to_string(),
                resource_type: Resources::RETURNS.to_string(),
                action: "reject".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::RETURNS, Actions::ALL),
            Permission {
                name: format_permission(Resources::RETURNS, Actions::ALL),
                description: "Full control over returns".to_string(),
                resource_type: Resources::RETURNS.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        // Admin permissions
        perms.insert(
            format_permission(Resources::ADMIN, Actions::ALL),
            Permission {
                name: format_permission(Resources::ADMIN, Actions::ALL),
                description: "Full administrator access".to_string(),
                resource_type: Resources::ADMIN.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        // API key permissions
        perms.insert(
            format_permission(Resources::API_KEYS, Actions::READ),
            Permission {
                name: format_permission(Resources::API_KEYS, Actions::READ),
                description: "View API keys".to_string(),
                resource_type: Resources::API_KEYS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::API_KEYS, Actions::CREATE),
            Permission {
                name: format_permission(Resources::API_KEYS, Actions::CREATE),
                description: "Create API keys".to_string(),
                resource_type: Resources::API_KEYS.to_string(),
                action: Actions::CREATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::API_KEYS, Actions::DELETE),
            Permission {
                name: format_permission(Resources::API_KEYS, Actions::DELETE),
                description: "Delete API keys".to_string(),
                resource_type: Resources::API_KEYS.to_string(),
                action: Actions::DELETE.to_string(),
            },
        );

        // User management permissions
        perms.insert(
            format_permission(Resources::USERS, Actions::READ),
            Permission {
                name: format_permission(Resources::USERS, Actions::READ),
                description: "View users".to_string(),
                resource_type: Resources::USERS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::USERS, Actions::CREATE),
            Permission {
                name: format_permission(Resources::USERS, Actions::CREATE),
                description: "Create users".to_string(),
                resource_type: Resources::USERS.to_string(),
                action: Actions::CREATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::USERS, Actions::UPDATE),
            Permission {
                name: format_permission(Resources::USERS, Actions::UPDATE),
                description: "Update users".to_string(),
                resource_type: Resources::USERS.to_string(),
                action: Actions::UPDATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::USERS, Actions::DELETE),
            Permission {
                name: format_permission(Resources::USERS, Actions::DELETE),
                description: "Delete users".to_string(),
                resource_type: Resources::USERS.to_string(),
                action: Actions::DELETE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::USERS, Actions::ALL),
            Permission {
                name: format_permission(Resources::USERS, Actions::ALL),
                description: "Full control over users".to_string(),
                resource_type: Resources::USERS.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        // Role management permissions
        perms.insert(
            format_permission(Resources::ROLES, Actions::READ),
            Permission {
                name: format_permission(Resources::ROLES, Actions::READ),
                description: "View roles".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ROLES, Actions::CREATE),
            Permission {
                name: format_permission(Resources::ROLES, Actions::CREATE),
                description: "Create roles".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: Actions::CREATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ROLES, Actions::UPDATE),
            Permission {
                name: format_permission(Resources::ROLES, Actions::UPDATE),
                description: "Update roles".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: Actions::UPDATE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ROLES, Actions::DELETE),
            Permission {
                name: format_permission(Resources::ROLES, Actions::DELETE),
                description: "Delete roles".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: Actions::DELETE.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ROLES, "assign"),
            Permission {
                name: format_permission(Resources::ROLES, "assign"),
                description: "Assign roles to users".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: "assign".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::ROLES, Actions::ALL),
            Permission {
                name: format_permission(Resources::ROLES, Actions::ALL),
                description: "Full control over roles".to_string(),
                resource_type: Resources::ROLES.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        // Report permissions
        perms.insert(
            format_permission(Resources::REPORTS, Actions::READ),
            Permission {
                name: format_permission(Resources::REPORTS, Actions::READ),
                description: "View reports".to_string(),
                resource_type: Resources::REPORTS.to_string(),
                action: Actions::READ.to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::REPORTS, "export"),
            Permission {
                name: format_permission(Resources::REPORTS, "export"),
                description: "Export reports".to_string(),
                resource_type: Resources::REPORTS.to_string(),
                action: "export".to_string(),
            },
        );

        perms.insert(
            format_permission(Resources::REPORTS, Actions::ALL),
            Permission {
                name: format_permission(Resources::REPORTS, Actions::ALL),
                description: "Full control over reports".to_string(),
                resource_type: Resources::REPORTS.to_string(),
                action: Actions::ALL.to_string(),
            },
        );

        perms
    };
}

/// Service for managing permissions
#[derive(Clone)]
pub struct PermissionService {
    // In a real implementation, this would be backed by a database
}

impl PermissionService {
    /// Create a new permission service
    pub fn new() -> Self {
        Self {}
    }

    /// Get a permission by name
    pub fn get_permission(&self, name: &str) -> Option<&Permission> {
        PERMISSIONS.get(name)
    }

    /// Get all permissions
    pub fn get_all_permissions(&self) -> Vec<&Permission> {
        PERMISSIONS.values().collect()
    }

    /// Get all permissions for a resource
    pub fn get_resource_permissions(&self, resource: &str) -> Vec<&Permission> {
        PERMISSIONS
            .values()
            .filter(|p| p.resource_type == resource)
            .collect()
    }

    /// Check if a permission exists
    pub fn permission_exists(&self, name: &str) -> bool {
        PERMISSIONS.contains_key(name)
    }

    /// Check if a permission is implied by another permission
    pub fn is_permission_implied(&self, user_perm: &str, required_perm: &str) -> bool {
        // Direct match
        if user_perm == required_perm {
            return true;
        }

        // Wildcard match (resource:*)
        let user_parts: Vec<&str> = user_perm.split(':').collect();
        let required_parts: Vec<&str> = required_perm.split(':').collect();

        if user_parts.len() == 2 && required_parts.len() == 2 {
            let user_resource = user_parts[0];
            let user_action = user_parts[1];
            let required_resource = required_parts[0];
            let _required_action = required_parts[1];

            // Check for resource wildcard (resource:*)
            if user_resource == required_resource && user_action == "*" {
                return true;
            }

            // Check for admin permission (admin:*)
            if user_resource == "admin" && user_action == "*" {
                return true;
            }
        }

        // Global wildcard match
        if user_perm == "*" {
            return true;
        }

        false
    }
}

/// Default implementation for PermissionService
impl Default for PermissionService {
    fn default() -> Self {
        Self::new()
    }
}
