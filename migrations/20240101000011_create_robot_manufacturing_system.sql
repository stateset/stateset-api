-- =====================================================
-- ROBOT MANUFACTURING SYSTEM - COMPREHENSIVE MIGRATION
-- =====================================================
-- This migration adds complete robot manufacturing capabilities including:
-- 1. Serial Number & Traceability
-- 2. Quality Control & Testing
-- 3. Configuration Management
-- 4. Enhanced BOM with Multi-level Support
-- 5. Advanced Work Order Features
-- 6. Compliance & Certifications
-- 7. Enhanced Warranty & Service
-- 8. Supplier Quality Management
-- 9. Production Analytics
-- 10. Subassembly Management

-- =====================================================
-- PHASE 1: SERIAL NUMBER & TRACEABILITY SYSTEM
-- =====================================================

-- Serial Numbers for finished robots
CREATE TABLE IF NOT EXISTS robot_serial_numbers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    serial_number VARCHAR(100) UNIQUE NOT NULL,
    product_id UUID NOT NULL,
    work_order_id UUID,
    robot_model VARCHAR(100) NOT NULL,
    robot_type VARCHAR(50) NOT NULL, -- 'articulated_arm', 'cobot', 'amr', 'specialized'
    manufacturing_date TIMESTAMP WITH TIME ZONE,
    ship_date TIMESTAMP WITH TIME ZONE,
    customer_id UUID,
    order_id UUID,
    status VARCHAR(50) NOT NULL DEFAULT 'in_production', -- 'in_production', 'testing', 'ready', 'shipped', 'in_service', 'returned', 'decommissioned'
    warranty_start_date TIMESTAMP WITH TIME ZONE,
    warranty_end_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_robot_serial_number ON robot_serial_numbers(serial_number);
CREATE INDEX idx_robot_product_id ON robot_serial_numbers(product_id);
CREATE INDEX idx_robot_work_order ON robot_serial_numbers(work_order_id);
CREATE INDEX idx_robot_status ON robot_serial_numbers(status);

-- Component Serial Numbers (for critical components like motors, controllers, sensors)
CREATE TABLE IF NOT EXISTS component_serial_numbers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    serial_number VARCHAR(100) UNIQUE NOT NULL,
    component_type VARCHAR(100) NOT NULL, -- 'servo_motor', 'controller', 'sensor', 'encoder', etc.
    component_sku VARCHAR(100) NOT NULL,
    supplier_id UUID,
    supplier_lot_number VARCHAR(100),
    manufacture_date DATE,
    receive_date DATE,
    status VARCHAR(50) NOT NULL DEFAULT 'in_stock', -- 'in_stock', 'allocated', 'installed', 'failed', 'returned'
    location VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_component_serial_number ON component_serial_numbers(serial_number);
CREATE INDEX idx_component_type ON component_serial_numbers(component_type);
CREATE INDEX idx_component_sku ON component_serial_numbers(component_sku);
CREATE INDEX idx_component_supplier_lot ON component_serial_numbers(supplier_lot_number);

-- Robot-Component Traceability (which components are in which robot)
CREATE TABLE IF NOT EXISTS robot_component_genealogy (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    component_serial_id UUID NOT NULL REFERENCES component_serial_numbers(id),
    position VARCHAR(100), -- 'joint_1', 'joint_2', 'gripper', 'controller_slot_1', etc.
    installed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    installed_by UUID,
    removed_at TIMESTAMP WITH TIME ZONE,
    removed_by UUID,
    removal_reason VARCHAR(500),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_genealogy_robot ON robot_component_genealogy(robot_serial_id);
CREATE INDEX idx_genealogy_component ON robot_component_genealogy(component_serial_id);

-- =====================================================
-- PHASE 2: QUALITY CONTROL & TESTING SYSTEM
-- =====================================================

-- QA Checkpoints (inspection stages in production)
CREATE TABLE IF NOT EXISTS qa_checkpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    checkpoint_type VARCHAR(50) NOT NULL, -- 'incoming_inspection', 'in_process', 'final_inspection', 'calibration'
    required BOOLEAN DEFAULT true,
    sequence INTEGER NOT NULL,
    applicable_product_types VARCHAR(100)[], -- which robot types this applies to
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Test Protocols (test procedures)
CREATE TABLE IF NOT EXISTS test_protocols (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_number VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    test_type VARCHAR(50) NOT NULL, -- 'mechanical', 'electrical', 'software', 'integration', 'safety'
    applicable_models VARCHAR(100)[],
    test_equipment_required VARCHAR(200)[],
    estimated_duration_minutes INTEGER,
    pass_criteria JSONB, -- detailed pass/fail criteria
    procedure_steps JSONB, -- step-by-step instructions
    revision VARCHAR(20),
    status VARCHAR(50) DEFAULT 'active', -- 'draft', 'active', 'obsolete'
    created_by UUID,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_test_protocol_number ON test_protocols(protocol_number);

-- Test Results (actual test execution results)
CREATE TABLE IF NOT EXISTS test_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    test_protocol_id UUID NOT NULL REFERENCES test_protocols(id),
    robot_serial_id UUID REFERENCES robot_serial_numbers(id),
    work_order_id UUID,
    tested_by UUID NOT NULL,
    test_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    status VARCHAR(50) NOT NULL, -- 'pass', 'fail', 'conditional_pass', 'retest_required'
    measurements JSONB, -- actual measured values
    test_equipment_ids UUID[],
    calibration_due_dates DATE[],
    notes TEXT,
    attachments JSONB, -- photos, videos, documents
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_test_result_robot ON test_results(robot_serial_id);
CREATE INDEX idx_test_result_protocol ON test_results(test_protocol_id);
CREATE INDEX idx_test_result_status ON test_results(status);

-- Test Equipment (tools and equipment used for testing)
CREATE TABLE IF NOT EXISTS test_equipment (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    equipment_number VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    equipment_type VARCHAR(100) NOT NULL, -- 'torque_sensor', 'multimeter', 'oscilloscope', 'positioning_laser'
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    serial_number VARCHAR(100),
    calibration_due_date DATE,
    calibration_interval_days INTEGER,
    location VARCHAR(100),
    status VARCHAR(50) DEFAULT 'active', -- 'active', 'calibration_due', 'out_of_service'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_equipment_number ON test_equipment(equipment_number);
CREATE INDEX idx_equipment_calibration ON test_equipment(calibration_due_date);

-- Non-Conformance Reports (NCRs)
CREATE TABLE IF NOT EXISTS non_conformance_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ncr_number VARCHAR(50) UNIQUE NOT NULL,
    robot_serial_id UUID REFERENCES robot_serial_numbers(id),
    work_order_id UUID,
    component_serial_id UUID REFERENCES component_serial_numbers(id),
    reported_by UUID NOT NULL,
    reported_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    issue_type VARCHAR(100) NOT NULL, -- 'dimensional', 'functional', 'cosmetic', 'documentation'
    severity VARCHAR(50) NOT NULL, -- 'critical', 'major', 'minor'
    description TEXT NOT NULL,
    root_cause TEXT,
    corrective_action TEXT,
    preventive_action TEXT,
    assigned_to UUID,
    status VARCHAR(50) DEFAULT 'open', -- 'open', 'investigating', 'action_required', 'resolved', 'closed'
    resolution_date TIMESTAMP WITH TIME ZONE,
    disposition VARCHAR(50), -- 'scrap', 'rework', 'use_as_is', 'return_to_supplier'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_ncr_number ON non_conformance_reports(ncr_number);
CREATE INDEX idx_ncr_robot ON non_conformance_reports(robot_serial_id);
CREATE INDEX idx_ncr_status ON non_conformance_reports(status);

-- =====================================================
-- PHASE 3: CONFIGURATION MANAGEMENT
-- =====================================================

-- Robot Configurations (as-ordered and as-built configs)
CREATE TABLE IF NOT EXISTS robot_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    configuration_type VARCHAR(50) NOT NULL, -- 'as_ordered', 'as_built'
    robot_model VARCHAR(100) NOT NULL,
    payload_kg DECIMAL(10, 2),
    reach_mm INTEGER,
    degrees_of_freedom INTEGER,
    end_effector_type VARCHAR(100),
    power_requirements VARCHAR(100),
    mounting_type VARCHAR(50), -- 'floor', 'ceiling', 'wall', 'mobile'
    custom_options JSONB, -- any custom configuration options
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_config_robot_serial ON robot_configurations(robot_serial_id);

-- Software/Firmware Versions
CREATE TABLE IF NOT EXISTS robot_software_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    component_type VARCHAR(100) NOT NULL, -- 'controller_firmware', 'motion_control', 'safety_plc', 'hmi'
    software_name VARCHAR(100) NOT NULL,
    version VARCHAR(50) NOT NULL,
    installed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    installed_by UUID,
    installation_method VARCHAR(50), -- 'factory', 'field_update', 'remote_update'
    previous_version VARCHAR(50),
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_software_robot ON robot_software_versions(robot_serial_id);
CREATE INDEX idx_software_component ON robot_software_versions(component_type);

-- =====================================================
-- PHASE 4: ENHANCED MULTI-LEVEL BOM SYSTEM
-- =====================================================

-- BOM Hierarchy (for subassemblies)
CREATE TABLE IF NOT EXISTS bom_hierarchy (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_bom_id UUID NOT NULL REFERENCES manufacturing_boms(id) ON DELETE CASCADE,
    child_bom_id UUID NOT NULL REFERENCES manufacturing_boms(id) ON DELETE CASCADE,
    quantity DECIMAL(10, 4) NOT NULL DEFAULT 1,
    reference_designator VARCHAR(100), -- position/location in parent assembly
    is_phantom BOOLEAN DEFAULT false, -- phantom BOM (for kitting, not actually assembled)
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_bom_hierarchy_parent ON bom_hierarchy(parent_bom_id);
CREATE INDEX idx_bom_hierarchy_child ON bom_hierarchy(child_bom_id);

-- Alternative Components (substitutions)
CREATE TABLE IF NOT EXISTS bom_component_alternatives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bom_component_id UUID NOT NULL REFERENCES manufacturing_bom_components(id) ON DELETE CASCADE,
    alternative_product_id UUID,
    alternative_item_id BIGINT,
    preference_order INTEGER DEFAULT 1, -- 1 = first choice alternative
    reason VARCHAR(200),
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_alt_component ON bom_component_alternatives(bom_component_id);

-- Engineering Change Orders (ECOs)
CREATE TABLE IF NOT EXISTS engineering_change_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    eco_number VARCHAR(50) UNIQUE NOT NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    reason TEXT NOT NULL,
    affected_bom_ids UUID[],
    affected_product_ids UUID[],
    change_type VARCHAR(50), -- 'component_change', 'process_change', 'documentation', 'safety'
    priority VARCHAR(50) DEFAULT 'normal', -- 'low', 'normal', 'high', 'critical'
    status VARCHAR(50) DEFAULT 'draft', -- 'draft', 'review', 'approved', 'released', 'rejected'
    requested_by UUID NOT NULL,
    approved_by UUID,
    effective_date DATE,
    implementation_notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_eco_number ON engineering_change_orders(eco_number);
CREATE INDEX idx_eco_status ON engineering_change_orders(status);

-- =====================================================
-- PHASE 5: ADVANCED WORK ORDER FEATURES
-- =====================================================

-- Work Order Dependencies
CREATE TABLE IF NOT EXISTS work_order_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    predecessor_work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    successor_work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    dependency_type VARCHAR(50) NOT NULL, -- 'finish_to_start', 'start_to_start', 'finish_to_finish'
    lag_hours DECIMAL(10, 2) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_wo_dep_predecessor ON work_order_dependencies(predecessor_work_order_id);
CREATE INDEX idx_wo_dep_successor ON work_order_dependencies(successor_work_order_id);

-- Production Lines/Cells
CREATE TABLE IF NOT EXISTS production_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    line_number VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    line_type VARCHAR(50), -- 'assembly', 'subassembly', 'testing', 'packaging'
    location VARCHAR(100),
    capacity_units_per_day DECIMAL(10, 2),
    status VARCHAR(50) DEFAULT 'active', -- 'active', 'maintenance', 'inactive'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Add production_line_id to work orders (we'll handle this in entity updates)

-- Labor Time Tracking
CREATE TABLE IF NOT EXISTS work_order_labor (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    task_id UUID REFERENCES manufacturing_work_order_tasks(id),
    employee_id UUID NOT NULL,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    hours DECIMAL(10, 2),
    labor_type VARCHAR(50), -- 'direct', 'indirect', 'rework'
    hourly_rate DECIMAL(10, 2),
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_labor_work_order ON work_order_labor(work_order_id);
CREATE INDEX idx_labor_employee ON work_order_labor(employee_id);

-- Material Waste and Scrap
CREATE TABLE IF NOT EXISTS work_order_scrap (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    component_id UUID REFERENCES manufacturing_bom_components(id),
    quantity DECIMAL(10, 4) NOT NULL,
    scrap_reason VARCHAR(100) NOT NULL,
    scrap_category VARCHAR(50), -- 'material_defect', 'process_error', 'operator_error', 'design_issue'
    reported_by UUID NOT NULL,
    reported_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    cost_impact DECIMAL(10, 2),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_scrap_work_order ON work_order_scrap(work_order_id);

-- =====================================================
-- PHASE 6: COMPLIANCE & CERTIFICATIONS
-- =====================================================

-- Certifications (CE, UL, ISO, RIA, etc.)
CREATE TABLE IF NOT EXISTS robot_certifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    certification_type VARCHAR(100) NOT NULL, -- 'CE', 'UL', 'ISO', 'RIA', 'CSA'
    certification_number VARCHAR(100),
    issuing_authority VARCHAR(200),
    issue_date DATE NOT NULL,
    expiration_date DATE,
    certification_scope TEXT,
    certificate_document_url VARCHAR(500),
    status VARCHAR(50) DEFAULT 'valid', -- 'valid', 'expired', 'pending_renewal'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_cert_robot ON robot_certifications(robot_serial_id);
CREATE INDEX idx_cert_type ON robot_certifications(certification_type);
CREATE INDEX idx_cert_expiration ON robot_certifications(expiration_date);

-- Material Certifications (for critical components)
CREATE TABLE IF NOT EXISTS material_certifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    component_serial_id UUID REFERENCES component_serial_numbers(id),
    certification_type VARCHAR(100) NOT NULL, -- 'material_test_report', 'rohs', 'reach', 'conflict_minerals'
    certificate_number VARCHAR(100),
    issuing_authority VARCHAR(200),
    issue_date DATE,
    certificate_document_url VARCHAR(500),
    test_results JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_mat_cert_component ON material_certifications(component_serial_id);

-- Documentation Packages (per robot)
CREATE TABLE IF NOT EXISTS robot_documentation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    document_type VARCHAR(100) NOT NULL, -- 'user_manual', 'service_manual', 'safety_data', 'calibration_cert', 'test_report'
    document_name VARCHAR(200) NOT NULL,
    document_url VARCHAR(500),
    document_version VARCHAR(50),
    language VARCHAR(10),
    generated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_doc_robot ON robot_documentation(robot_serial_id);

-- =====================================================
-- PHASE 7: ENHANCED WARRANTY & SERVICE
-- =====================================================

-- Service History (linked to serial numbers)
CREATE TABLE IF NOT EXISTS robot_service_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID NOT NULL REFERENCES robot_serial_numbers(id) ON DELETE CASCADE,
    service_ticket_number VARCHAR(50) UNIQUE NOT NULL,
    service_type VARCHAR(50) NOT NULL, -- 'preventive_maintenance', 'repair', 'calibration', 'software_update', 'inspection'
    service_date DATE NOT NULL,
    technician_id UUID,
    description TEXT,
    work_performed TEXT,
    parts_replaced JSONB, -- list of parts replaced
    labor_hours DECIMAL(10, 2),
    service_cost DECIMAL(10, 2),
    next_service_due DATE,
    status VARCHAR(50) DEFAULT 'completed', -- 'scheduled', 'in_progress', 'completed', 'cancelled'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_service_robot ON robot_service_history(robot_serial_id);
CREATE INDEX idx_service_ticket ON robot_service_history(service_ticket_number);
CREATE INDEX idx_service_next_due ON robot_service_history(next_service_due);

-- Preventive Maintenance Schedules
CREATE TABLE IF NOT EXISTS maintenance_schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_model VARCHAR(100) NOT NULL,
    maintenance_type VARCHAR(100) NOT NULL, -- 'lubrication', 'brake_check', 'battery_replacement', 'calibration'
    frequency_days INTEGER NOT NULL,
    estimated_duration_hours DECIMAL(10, 2),
    required_parts JSONB,
    procedure_document_url VARCHAR(500),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_maint_schedule_model ON maintenance_schedules(robot_model);

-- Failure Analysis
CREATE TABLE IF NOT EXISTS failure_analysis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    robot_serial_id UUID REFERENCES robot_serial_numbers(id),
    component_serial_id UUID REFERENCES component_serial_numbers(id),
    failure_date TIMESTAMP WITH TIME ZONE NOT NULL,
    reported_by UUID,
    failure_mode VARCHAR(200) NOT NULL,
    failure_description TEXT NOT NULL,
    operating_hours_at_failure DECIMAL(10, 2),
    root_cause TEXT,
    corrective_action TEXT,
    warranty_claim BOOLEAN DEFAULT false,
    cost_impact DECIMAL(10, 2),
    mtbf_impact BOOLEAN DEFAULT true, -- include in MTBF calculations
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_failure_robot ON failure_analysis(robot_serial_id);
CREATE INDEX idx_failure_component ON failure_analysis(component_serial_id);

-- =====================================================
-- PHASE 8: SUPPLIER QUALITY MANAGEMENT
-- =====================================================

-- Supplier Performance Scorecards
CREATE TABLE IF NOT EXISTS supplier_performance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    supplier_id UUID NOT NULL,
    evaluation_period_start DATE NOT NULL,
    evaluation_period_end DATE NOT NULL,
    on_time_delivery_rate DECIMAL(5, 2), -- percentage
    quality_acceptance_rate DECIMAL(5, 2), -- percentage
    defect_rate DECIMAL(5, 2), -- parts per million
    responsiveness_score INTEGER, -- 1-10
    cost_competitiveness_score INTEGER, -- 1-10
    overall_score DECIMAL(5, 2),
    rating VARCHAR(20), -- 'excellent', 'good', 'acceptable', 'needs_improvement', 'unacceptable'
    evaluated_by UUID,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_supplier_perf_supplier ON supplier_performance(supplier_id);
CREATE INDEX idx_supplier_perf_period ON supplier_performance(evaluation_period_end);

-- Incoming Inspection Results
CREATE TABLE IF NOT EXISTS incoming_inspection (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    purchase_order_id UUID,
    supplier_id UUID NOT NULL,
    component_sku VARCHAR(100) NOT NULL,
    lot_number VARCHAR(100),
    quantity_received INTEGER NOT NULL,
    quantity_inspected INTEGER NOT NULL,
    quantity_accepted INTEGER NOT NULL,
    quantity_rejected INTEGER NOT NULL,
    inspector_id UUID NOT NULL,
    inspection_date DATE NOT NULL,
    defect_types VARCHAR(100)[],
    disposition VARCHAR(50), -- 'accepted', 'rejected', 'conditional_acceptance', 'return_to_supplier'
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_incoming_supplier ON incoming_inspection(supplier_id);
CREATE INDEX idx_incoming_component ON incoming_inspection(component_sku);

-- Supplier Corrective Action Requests (SCARs)
CREATE TABLE IF NOT EXISTS supplier_corrective_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scar_number VARCHAR(50) UNIQUE NOT NULL,
    supplier_id UUID NOT NULL,
    issue_date DATE NOT NULL,
    issue_description TEXT NOT NULL,
    component_affected VARCHAR(100),
    quantity_affected INTEGER,
    severity VARCHAR(50) NOT NULL, -- 'critical', 'major', 'minor'
    requested_action TEXT NOT NULL,
    supplier_response TEXT,
    supplier_response_date DATE,
    effectiveness_verified BOOLEAN,
    verification_date DATE,
    verified_by UUID,
    status VARCHAR(50) DEFAULT 'open', -- 'open', 'awaiting_response', 'action_in_progress', 'closed'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_scar_number ON supplier_corrective_actions(scar_number);
CREATE INDEX idx_scar_supplier ON supplier_corrective_actions(supplier_id);

-- =====================================================
-- PHASE 9: PRODUCTION ANALYTICS & METRICS
-- =====================================================

-- Production Metrics (daily/shift tracking)
CREATE TABLE IF NOT EXISTS production_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_date DATE NOT NULL,
    shift VARCHAR(50), -- 'day', 'night', 'swing'
    production_line_id UUID REFERENCES production_lines(id),
    product_id UUID,
    robot_model VARCHAR(100),
    planned_quantity INTEGER,
    actual_quantity INTEGER,
    quantity_passed INTEGER,
    quantity_failed INTEGER,
    quantity_rework INTEGER,
    first_pass_yield DECIMAL(5, 2), -- percentage
    scrap_rate DECIMAL(5, 2), -- percentage
    planned_hours DECIMAL(10, 2),
    actual_hours DECIMAL(10, 2),
    downtime_hours DECIMAL(10, 2),
    downtime_reason VARCHAR(200),
    oee DECIMAL(5, 2), -- Overall Equipment Effectiveness percentage
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_prod_metrics_date ON production_metrics(production_date);
CREATE INDEX idx_prod_metrics_line ON production_metrics(production_line_id);

-- Cost Tracking (per work order / per robot)
CREATE TABLE IF NOT EXISTS work_order_costs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_order_id UUID NOT NULL REFERENCES manufacturing_work_orders(id) ON DELETE CASCADE,
    robot_serial_id UUID REFERENCES robot_serial_numbers(id),
    material_cost DECIMAL(10, 2) DEFAULT 0,
    labor_cost DECIMAL(10, 2) DEFAULT 0,
    overhead_cost DECIMAL(10, 2) DEFAULT 0,
    scrap_cost DECIMAL(10, 2) DEFAULT 0,
    total_cost DECIMAL(10, 2) GENERATED ALWAYS AS (material_cost + labor_cost + overhead_cost + scrap_cost) STORED,
    standard_cost DECIMAL(10, 2),
    cost_variance DECIMAL(10, 2) GENERATED ALWAYS AS (total_cost - COALESCE(standard_cost, 0)) STORED,
    calculated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_wo_costs_work_order ON work_order_costs(work_order_id);
CREATE INDEX idx_wo_costs_robot ON work_order_costs(robot_serial_id);

-- =====================================================
-- PHASE 10: SUBASSEMBLY MANAGEMENT
-- =====================================================

-- Subassembly Serial Numbers
CREATE TABLE IF NOT EXISTS subassembly_serial_numbers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    serial_number VARCHAR(100) UNIQUE NOT NULL,
    subassembly_type VARCHAR(100) NOT NULL, -- 'arm', 'gripper', 'base', 'controller_assembly'
    bom_id UUID REFERENCES manufacturing_boms(id),
    work_order_id UUID REFERENCES manufacturing_work_orders(id),
    product_id UUID,
    parent_robot_serial_id UUID REFERENCES robot_serial_numbers(id), -- if installed in a robot
    status VARCHAR(50) DEFAULT 'in_production', -- 'in_production', 'completed', 'in_stock', 'installed', 'scrapped'
    completed_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_subassembly_serial ON subassembly_serial_numbers(serial_number);
CREATE INDEX idx_subassembly_type ON subassembly_serial_numbers(subassembly_type);
CREATE INDEX idx_subassembly_parent ON subassembly_serial_numbers(parent_robot_serial_id);

-- Kitting Operations (pick all parts for a robot/subassembly)
CREATE TABLE IF NOT EXISTS kit_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kit_number VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    bom_id UUID REFERENCES manufacturing_boms(id),
    kit_type VARCHAR(50), -- 'robot_complete', 'subassembly', 'service_kit'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS kit_picks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kit_definition_id UUID NOT NULL REFERENCES kit_definitions(id),
    work_order_id UUID REFERENCES manufacturing_work_orders(id),
    picked_by UUID,
    pick_date TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    status VARCHAR(50) DEFAULT 'in_progress', -- 'in_progress', 'completed', 'staged'
    location VARCHAR(100),
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_kit_picks_work_order ON kit_picks(work_order_id);

-- =====================================================
-- VIEWS FOR COMMON QUERIES
-- =====================================================

-- Robot Traceability View (complete genealogy)
CREATE OR REPLACE VIEW robot_complete_genealogy AS
SELECT
    rsn.serial_number as robot_serial,
    rsn.robot_model,
    rsn.status as robot_status,
    rcg.position,
    csn.serial_number as component_serial,
    csn.component_type,
    csn.component_sku,
    csn.supplier_lot_number,
    rcg.installed_at,
    rcg.removed_at
FROM robot_serial_numbers rsn
LEFT JOIN robot_component_genealogy rcg ON rsn.id = rcg.robot_serial_id
LEFT JOIN component_serial_numbers csn ON rcg.component_serial_id = csn.id
WHERE rcg.removed_at IS NULL OR rcg.removed_at > NOW();

-- Production Status Dashboard View
CREATE OR REPLACE VIEW production_status_dashboard AS
SELECT
    wo.work_order_number,
    wo.status as work_order_status,
    rsn.serial_number as robot_serial,
    rsn.robot_model,
    wo.quantity_to_build,
    wo.quantity_completed,
    wo.scheduled_start,
    wo.actual_start,
    wo.scheduled_end,
    COUNT(DISTINCT tr.id) as tests_completed,
    COUNT(DISTINCT tr.id) FILTER (WHERE tr.status = 'pass') as tests_passed,
    COUNT(DISTINCT ncr.id) as open_ncrs
FROM manufacturing_work_orders wo
LEFT JOIN robot_serial_numbers rsn ON wo.id = rsn.work_order_id
LEFT JOIN test_results tr ON rsn.id = tr.robot_serial_id
LEFT JOIN non_conformance_reports ncr ON rsn.id = ncr.robot_serial_id AND ncr.status IN ('open', 'investigating', 'action_required')
GROUP BY wo.id, rsn.id;

-- Quality Metrics View
CREATE OR REPLACE VIEW quality_metrics_summary AS
SELECT
    DATE_TRUNC('week', test_date) as week,
    tp.test_type,
    COUNT(*) as total_tests,
    COUNT(*) FILTER (WHERE status = 'pass') as passed,
    COUNT(*) FILTER (WHERE status = 'fail') as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE status = 'pass') / NULLIF(COUNT(*), 0), 2) as pass_rate
FROM test_results tr
JOIN test_protocols tp ON tr.test_protocol_id = tp.id
GROUP BY DATE_TRUNC('week', test_date), tp.test_type;

-- =====================================================
-- TRIGGERS FOR AUTO-UPDATES
-- =====================================================

-- Update robot_serial_numbers.updated_at on change
CREATE OR REPLACE FUNCTION update_robot_serial_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER robot_serial_update_timestamp
    BEFORE UPDATE ON robot_serial_numbers
    FOR EACH ROW
    EXECUTE FUNCTION update_robot_serial_timestamp();

-- Similar triggers for other tables
CREATE TRIGGER component_serial_update_timestamp
    BEFORE UPDATE ON component_serial_numbers
    FOR EACH ROW
    EXECUTE FUNCTION update_robot_serial_timestamp();

CREATE TRIGGER test_equipment_update_timestamp
    BEFORE UPDATE ON test_equipment
    FOR EACH ROW
    EXECUTE FUNCTION update_robot_serial_timestamp();

-- =====================================================
-- INDEXES FOR PERFORMANCE
-- =====================================================

-- Additional composite indexes for common query patterns
CREATE INDEX idx_robot_serial_date_status ON robot_serial_numbers(manufacturing_date, status);
CREATE INDEX idx_test_results_robot_date ON test_results(robot_serial_id, test_date);
CREATE INDEX idx_ncr_severity_status ON non_conformance_reports(severity, status);
CREATE INDEX idx_service_history_date ON robot_service_history(service_date DESC);

-- =====================================================
-- COMMENTS FOR DOCUMENTATION
-- =====================================================

COMMENT ON TABLE robot_serial_numbers IS 'Unique serial numbers for finished robots with full lifecycle tracking';
COMMENT ON TABLE component_serial_numbers IS 'Serial numbers for critical components (motors, controllers, sensors)';
COMMENT ON TABLE robot_component_genealogy IS 'Traceability matrix linking components to robots';
COMMENT ON TABLE test_protocols IS 'Standardized test procedures for quality control';
COMMENT ON TABLE test_results IS 'Actual test execution results linked to robots and protocols';
COMMENT ON TABLE non_conformance_reports IS 'Quality issues, defects, and corrective actions';
COMMENT ON TABLE robot_configurations IS 'As-ordered and as-built configuration specifications';
COMMENT ON TABLE engineering_change_orders IS 'Engineering changes to BOMs and products';
COMMENT ON TABLE work_order_labor IS 'Labor time tracking for manufacturing operations';
COMMENT ON TABLE robot_certifications IS 'Safety and regulatory certifications per robot';
COMMENT ON TABLE robot_service_history IS 'Complete service and maintenance history per robot';
COMMENT ON TABLE supplier_performance IS 'Supplier quality scorecards and ratings';
COMMENT ON TABLE production_metrics IS 'Daily production statistics and OEE metrics';
COMMENT ON TABLE subassembly_serial_numbers IS 'Serial numbers for major subassemblies';

-- =====================================================
-- INITIAL DATA / SEED DATA
-- =====================================================

-- Insert some default test protocols for common robot tests
INSERT INTO test_protocols (protocol_number, name, description, test_type, estimated_duration_minutes, pass_criteria, procedure_steps, revision, status) VALUES
('TP-001', 'Joint Torque Test', 'Verify each joint meets torque specifications', 'mechanical', 30,
 '{"min_torque_nm": 50, "max_torque_nm": 200}',
 '{"steps": ["Power on robot", "Home all axes", "Apply load to each joint", "Measure torque", "Record results"]}',
 'A', 'active'),
('TP-002', 'Positioning Accuracy Test', 'Verify repeatability and accuracy of end effector positioning', 'mechanical', 45,
 '{"repeatability_mm": 0.05, "accuracy_mm": 0.1}',
 '{"steps": ["Set up laser tracker", "Program 10 test positions", "Execute 5 cycles", "Measure positions", "Calculate statistics"]}',
 'A', 'active'),
('TP-003', 'Safety System Test', 'Verify all emergency stops and safety interlocks', 'safety', 20,
 '{"response_time_ms": 100, "stop_distance_mm": 50}',
 '{"steps": ["Test E-stop buttons", "Test light curtains", "Test enabling device", "Verify safe torque off", "Document results"]}',
 'A', 'active'),
('TP-004', 'Controller Functional Test', 'Verify controller operation and communication', 'electrical', 25,
 '{"voltage_tolerance": 5, "communication_success_rate": 100}',
 '{"steps": ["Power on controller", "Test I/O signals", "Test fieldbus communication", "Test HMI", "Check error logs"]}',
 'A', 'active'),
('TP-005', 'Software Integration Test', 'Verify firmware and motion control software', 'software', 40,
 '{"trajectory_error_mm": 1, "cycle_time_variance_percent": 5}',
 '{"steps": ["Load test program", "Execute motion sequences", "Monitor performance", "Check error handling", "Verify data logging"]}',
 'A', 'active');

-- Insert default QA checkpoints
INSERT INTO qa_checkpoints (name, description, checkpoint_type, required, sequence, applicable_product_types) VALUES
('Incoming Component Inspection', 'Inspect components upon receipt from suppliers', 'incoming_inspection', true, 1, ARRAY['articulated_arm', 'cobot', 'amr', 'specialized']),
('Subassembly Inspection', 'Inspect completed subassemblies before final assembly', 'in_process', true, 2, ARRAY['articulated_arm', 'cobot', 'amr', 'specialized']),
('Pre-Test Inspection', 'Visual and dimensional inspection before functional testing', 'in_process', true, 3, ARRAY['articulated_arm', 'cobot', 'amr', 'specialized']),
('Final Functional Test', 'Complete functional and safety testing', 'final_inspection', true, 4, ARRAY['articulated_arm', 'cobot', 'amr', 'specialized']),
('Final QA Inspection', 'Final inspection before packaging and shipment', 'final_inspection', true, 5, ARRAY['articulated_arm', 'cobot', 'amr', 'specialized']);

-- Insert default maintenance schedules (example for industrial arms)
INSERT INTO maintenance_schedules (robot_model, maintenance_type, frequency_days, estimated_duration_hours, procedure_document_url) VALUES
('IR-6000', 'Lubrication Service', 180, 2.0, '/docs/lubrication-ir6000.pdf'),
('IR-6000', 'Brake Inspection', 365, 3.0, '/docs/brake-check-ir6000.pdf'),
('IR-6000', 'Calibration Verification', 365, 4.0, '/docs/calibration-ir6000.pdf'),
('IR-6000', 'Battery Replacement', 1825, 1.0, '/docs/battery-replacement-ir6000.pdf'),
('CR-5', 'Lubrication Service', 180, 1.5, '/docs/lubrication-cr5.pdf'),
('CR-5', 'Safety System Check', 180, 2.0, '/docs/safety-check-cr5.pdf'),
('CR-5', 'Calibration Verification', 365, 3.0, '/docs/calibration-cr5.pdf');

-- =====================================================
-- GRANTS (adjust based on your role structure)
-- =====================================================

-- Grant appropriate permissions to application role
-- GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO stateset_app;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO stateset_app;

-- =====================================================
-- END OF MIGRATION
-- =====================================================
