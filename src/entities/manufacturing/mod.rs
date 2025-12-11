pub mod bom;
pub mod bom_audit;
pub mod bom_component;
pub mod work_order;
pub mod work_order_material;
pub mod work_order_note;
pub mod work_order_task;

// Phase 1: Serial Number & Traceability
pub mod component_serial_number;
pub mod robot_component_genealogy;
pub mod robot_serial_number;

// Phase 2: Quality Control & Testing
pub mod non_conformance_report;
pub mod test_protocol;
pub mod test_result;

// Phase 3: Configuration Management
pub mod robot_configuration;

// Phase 4: Enhanced BOM
pub mod engineering_change_order;

// Phase 5: Advanced Work Orders
pub mod production_line;

// Phase 6: Compliance & Certifications
pub mod robot_certification;

// Phase 7: Service & Maintenance
pub mod robot_service_history;

// Phase 8: Supplier Quality
pub mod supplier_performance;

// Phase 9: Production Analytics
pub mod production_metrics;

// Phase 10: Subassembly Management
pub mod subassembly_serial_number;
