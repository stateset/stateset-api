use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    dto::manufacturing::{
        certification::{CreateCertificationRequest, CertificationResponse},
        component_serial::{
            ComponentSerialResponse, CreateComponentSerialRequest, InstallComponentRequest,
        },
        ncr::{CloseNcrRequest, CreateNcrRequest, ListNcrQuery, NcrResponse},
        production::{CreateProductionMetricsRequest, ProductionMetricsResponse, ProductionMetricsQuery},
        robot_serial::{
            CreateRobotSerialRequest, ListRobotSerialsQuery, RobotGenealogyResponse,
            RobotSerialResponse, UpdateRobotSerialRequest,
        },
        service::{
            CompleteServiceRequest, CreateServiceRecordRequest, ServiceRecordResponse,
        },
        test_protocol::{CreateTestProtocolRequest, TestProtocolResponse},
        test_result::{CreateTestResultRequest, TestResultResponse},
    },
    entities::manufacturing::{
        robot_serial_number, component_serial_number, robot_component_genealogy,
        test_protocol, test_result, non_conformance_report, robot_certification,
        robot_service_history, production_metrics, production_line,
    },
    AppState,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, PaginatorTrait,
};

// ============================================================================
// ROBOT SERIAL NUMBER HANDLERS
// ============================================================================

/// Create a new robot serial number
pub async fn create_robot_serial(
    State(state): State<AppState>,
    Json(payload): Json<CreateRobotSerialRequest>,
) -> Result<Json<RobotSerialResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let robot_serial = robot_serial_number::ActiveModel {
        serial_number: Set(payload.serial_number),
        product_id: Set(payload.product_id),
        work_order_id: Set(payload.work_order_id),
        robot_model: Set(payload.robot_model),
        robot_type: Set(payload.robot_type),
        manufacturing_date: Set(payload.manufacturing_date),
        customer_id: Set(payload.customer_id),
        order_id: Set(payload.order_id),
        ..Default::default()
    };

    let robot = robot_serial
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RobotSerialResponse {
        id: robot.id,
        serial_number: robot.serial_number.clone(),
        product_id: robot.product_id.clone(),
        work_order_id: robot.work_order_id.clone(),
        robot_model: robot.robot_model.clone(),
        robot_type: robot.robot_type.clone(),
        manufacturing_date: robot.manufacturing_date,
        ship_date: robot.ship_date,
        customer_id: robot.customer_id.clone(),
        order_id: robot.order_id.clone(),
        status: robot.status.clone(),
        warranty_start_date: robot.warranty_start_date,
        warranty_end_date: robot.warranty_end_date,
        is_under_warranty: robot.is_under_warranty(),
        warranty_remaining_days: robot.warranty_remaining_days(),
        created_at: robot.created_at,
        updated_at: robot.updated_at,
    }))
}

/// Get robot serial by ID
pub async fn get_robot_serial(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RobotSerialResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let robot = robot_serial_number::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Robot serial not found".to_string()))?;

    Ok(Json(RobotSerialResponse {
        id: robot.id,
        serial_number: robot.serial_number.clone(),
        product_id: robot.product_id,
        work_order_id: robot.work_order_id,
        robot_model: robot.robot_model.clone(),
        robot_type: robot.robot_type.clone(),
        manufacturing_date: robot.manufacturing_date,
        ship_date: robot.ship_date,
        customer_id: robot.customer_id,
        order_id: robot.order_id,
        status: robot.status.clone(),
        warranty_start_date: robot.warranty_start_date,
        warranty_end_date: robot.warranty_end_date,
        is_under_warranty: robot.is_under_warranty(),
        warranty_remaining_days: robot.warranty_remaining_days(),
        created_at: robot.created_at,
        updated_at: robot.updated_at,
    }))
}

/// List robot serials with optional filters
pub async fn list_robot_serials(
    State(state): State<AppState>,
    Query(query): Query<ListRobotSerialsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let mut select = robot_serial_number::Entity::find();

    if let Some(status) = query.status {
        select = select.filter(robot_serial_number::Column::Status.eq(status));
    }
    if let Some(robot_type) = query.robot_type {
        select = select.filter(robot_serial_number::Column::RobotType.eq(robot_type));
    }
    if let Some(robot_model) = query.robot_model {
        select = select.filter(robot_serial_number::Column::RobotModel.eq(robot_model));
    }
    if let Some(customer_id) = query.customer_id {
        select = select.filter(robot_serial_number::Column::CustomerId.eq(customer_id));
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let robots = select
        .order_by_desc(robot_serial_number::Column::CreatedAt)
        .paginate(db, limit)
        .fetch_page(offset / limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<RobotSerialResponse> = robots
        .into_iter()
        .map(|robot| RobotSerialResponse {
            id: robot.id,
            serial_number: robot.serial_number.clone(),
            product_id: robot.product_id,
            work_order_id: robot.work_order_id,
            robot_model: robot.robot_model.clone(),
            robot_type: robot.robot_type.clone(),
            manufacturing_date: robot.manufacturing_date,
            ship_date: robot.ship_date,
            customer_id: robot.customer_id,
            order_id: robot.order_id,
            status: robot.status.clone(),
            warranty_start_date: robot.warranty_start_date,
            warranty_end_date: robot.warranty_end_date,
            is_under_warranty: robot.is_under_warranty(),
            warranty_remaining_days: robot.warranty_remaining_days(),
            created_at: robot.created_at,
            updated_at: robot.updated_at,
        })
        .collect();

    Ok(Json(json!({
        "data": response,
        "total": response.len(),
    })))
}

/// Update robot serial
pub async fn update_robot_serial(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRobotSerialRequest>,
) -> Result<Json<RobotSerialResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let robot = robot_serial_number::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Robot serial not found".to_string()))?;

    let mut robot: robot_serial_number::ActiveModel = robot.into();

    if let Some(status) = payload.status {
        robot.status = Set(status);
    }
    if let Some(manufacturing_date) = payload.manufacturing_date {
        robot.manufacturing_date = Set(Some(manufacturing_date));
    }
    if let Some(ship_date) = payload.ship_date {
        robot.ship_date = Set(Some(ship_date));
    }
    if let Some(customer_id) = payload.customer_id {
        robot.customer_id = Set(Some(customer_id));
    }
    if let Some(order_id) = payload.order_id {
        robot.order_id = Set(Some(order_id));
    }
    if let Some(warranty_start) = payload.warranty_start_date {
        robot.warranty_start_date = Set(Some(warranty_start));
    }
    if let Some(warranty_end) = payload.warranty_end_date {
        robot.warranty_end_date = Set(Some(warranty_end));
    }

    let updated = robot
        .update(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RobotSerialResponse {
        id: updated.id,
        serial_number: updated.serial_number.clone(),
        product_id: updated.product_id.clone(),
        work_order_id: updated.work_order_id.clone(),
        robot_model: updated.robot_model.clone(),
        robot_type: updated.robot_type.clone(),
        manufacturing_date: updated.manufacturing_date,
        ship_date: updated.ship_date,
        customer_id: updated.customer_id.clone(),
        order_id: updated.order_id.clone(),
        status: updated.status.clone(),
        warranty_start_date: updated.warranty_start_date,
        warranty_end_date: updated.warranty_end_date,
        is_under_warranty: updated.is_under_warranty(),
        warranty_remaining_days: updated.warranty_remaining_days(),
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

/// Get robot genealogy (component traceability)
pub async fn get_robot_genealogy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RobotGenealogyResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    // Get robot
    let robot = robot_serial_number::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Robot serial not found".to_string()))?;

    // Get component genealogy
    let genealogy = robot_component_genealogy::Entity::find()
        .filter(robot_component_genealogy::Column::RobotSerialId.eq(id))
        .filter(robot_component_genealogy::Column::RemovedAt.is_null())
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut components = Vec::new();

    for gen in genealogy {
        if let Some(component) = component_serial_number::Entity::find_by_id(gen.component_serial_id)
            .one(db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        {
            components.push(crate::dto::manufacturing::robot_serial::ComponentInRobot {
                component_serial_number: component.serial_number,
                component_type: component.component_type,
                component_sku: component.component_sku,
                position: gen.position,
                installed_at: gen.installed_at,
                supplier_lot_number: component.supplier_lot_number,
            });
        }
    }

    Ok(Json(RobotGenealogyResponse {
        robot_serial_number: robot.serial_number,
        robot_model: robot.robot_model,
        robot_status: robot.status,
        components,
    }))
}

// ============================================================================
// COMPONENT SERIAL NUMBER HANDLERS
// ============================================================================

/// Create component serial
pub async fn create_component_serial(
    State(state): State<AppState>,
    Json(payload): Json<CreateComponentSerialRequest>,
) -> Result<Json<ComponentSerialResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let component = component_serial_number::ActiveModel {
        serial_number: Set(payload.serial_number),
        component_type: Set(payload.component_type),
        component_sku: Set(payload.component_sku),
        supplier_id: Set(payload.supplier_id),
        supplier_lot_number: Set(payload.supplier_lot_number),
        manufacture_date: Set(payload.manufacture_date),
        receive_date: Set(payload.receive_date),
        location: Set(payload.location),
        ..Default::default()
    };

    let saved = component
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ComponentSerialResponse {
        id: saved.id,
        serial_number: saved.serial_number.clone(),
        component_type: saved.component_type.clone(),
        component_sku: saved.component_sku.clone(),
        supplier_id: saved.supplier_id.clone(),
        supplier_lot_number: saved.supplier_lot_number.clone(),
        manufacture_date: saved.manufacture_date,
        receive_date: saved.receive_date,
        status: saved.status.clone(),
        location: saved.location.clone(),
        age_in_days: saved.age_in_days(),
        created_at: saved.created_at,
        updated_at: saved.updated_at,
    }))
}

/// Install component into robot
pub async fn install_component(
    State(state): State<AppState>,
    Json(payload): Json<InstallComponentRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state.db.as_ref();

    // Verify component exists and is available
    let component = component_serial_number::Entity::find_by_id(payload.component_serial_id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Component not found".to_string()))?;

    if !component.is_available() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Component is not available for installation".to_string(),
        ));
    }

    // Create genealogy record
    let genealogy = robot_component_genealogy::ActiveModel {
        robot_serial_id: Set(payload.robot_serial_id),
        component_serial_id: Set(payload.component_serial_id),
        position: Set(payload.position),
        installed_by: Set(payload.installed_by),
        ..Default::default()
    };

    genealogy
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update component status
    let mut component: component_serial_number::ActiveModel = component.into();
    component.status = Set(component_serial_number::ComponentStatus::Installed);
    component
        .update(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "message": "Component installed successfully",
        "robot_serial_id": payload.robot_serial_id,
        "component_serial_id": payload.component_serial_id,
    })))
}

// ============================================================================
// TEST PROTOCOL HANDLERS
// ============================================================================

/// Create test protocol
pub async fn create_test_protocol(
    State(state): State<AppState>,
    Json(payload): Json<CreateTestProtocolRequest>,
) -> Result<Json<TestProtocolResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let protocol = test_protocol::ActiveModel {
        protocol_number: Set(payload.protocol_number),
        name: Set(payload.name),
        description: Set(payload.description),
        test_type: Set(payload.test_type),
        applicable_models: Set(payload.applicable_models),
        test_equipment_required: Set(payload.test_equipment_required),
        estimated_duration_minutes: Set(payload.estimated_duration_minutes),
        pass_criteria: Set(payload.pass_criteria),
        procedure_steps: Set(payload.procedure_steps),
        revision: Set(payload.revision),
        ..Default::default()
    };

    let saved = protocol
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TestProtocolResponse {
        id: saved.id,
        protocol_number: saved.protocol_number.clone(),
        name: saved.name.clone(),
        description: saved.description.clone(),
        test_type: saved.test_type.clone(),
        applicable_models: saved.applicable_models.clone(),
        test_equipment_required: saved.test_equipment_required.clone(),
        estimated_duration_minutes: saved.estimated_duration_minutes,
        pass_criteria: saved.pass_criteria.clone(),
        procedure_steps: saved.procedure_steps.clone(),
        revision: saved.revision.clone(),
        status: saved.status.clone(),
        is_active: saved.is_active(),
        created_at: saved.created_at,
        updated_at: saved.updated_at,
    }))
}

/// List test protocols
pub async fn list_test_protocols(
    State(state): State<AppState>,
) -> Result<Json<Vec<TestProtocolResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let protocols = test_protocol::Entity::find()
        .order_by_asc(test_protocol::Column::ProtocolNumber)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<TestProtocolResponse> = protocols
        .into_iter()
        .map(|p| TestProtocolResponse {
            id: p.id,
            protocol_number: p.protocol_number.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            test_type: p.test_type.clone(),
            applicable_models: p.applicable_models.clone(),
            test_equipment_required: p.test_equipment_required.clone(),
            estimated_duration_minutes: p.estimated_duration_minutes,
            pass_criteria: p.pass_criteria.clone(),
            procedure_steps: p.procedure_steps.clone(),
            revision: p.revision.clone(),
            status: p.status.clone(),
            is_active: p.is_active(),
            created_at: p.created_at,
            updated_at: p.updated_at,
        })
        .collect();

    Ok(Json(response))
}

// ============================================================================
// TEST RESULT HANDLERS
// ============================================================================

/// Create test result
pub async fn create_test_result(
    State(state): State<AppState>,
    Json(payload): Json<CreateTestResultRequest>,
) -> Result<Json<TestResultResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let result = test_result::ActiveModel {
        test_protocol_id: Set(payload.test_protocol_id),
        robot_serial_id: Set(payload.robot_serial_id),
        work_order_id: Set(payload.work_order_id),
        tested_by: Set(payload.tested_by),
        status: Set(payload.status),
        measurements: Set(payload.measurements),
        notes: Set(payload.notes),
        attachments: Set(payload.attachments),
        ..Default::default()
    };

    let saved = result
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TestResultResponse {
        id: saved.id,
        test_protocol_id: saved.test_protocol_id,
        test_protocol_name: None,
        robot_serial_id: saved.robot_serial_id,
        robot_serial_number: None,
        work_order_id: saved.work_order_id.clone(),
        tested_by: saved.tested_by.clone(),
        test_date: saved.test_date,
        status: saved.status.clone(),
        measurements: saved.measurements.clone(),
        notes: saved.notes.clone(),
        passed: saved.passed(),
        needs_retest: saved.needs_retest(),
        created_at: saved.created_at,
    }))
}

/// Get test results for a robot
pub async fn get_robot_test_results(
    State(state): State<AppState>,
    Path(robot_id): Path<Uuid>,
) -> Result<Json<Vec<TestResultResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let results = test_result::Entity::find()
        .filter(test_result::Column::RobotSerialId.eq(robot_id))
        .order_by_desc(test_result::Column::TestDate)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<TestResultResponse> = results
        .into_iter()
        .map(|r| TestResultResponse {
            id: r.id,
            test_protocol_id: r.test_protocol_id,
            test_protocol_name: None,
            robot_serial_id: r.robot_serial_id,
            robot_serial_number: None,
            work_order_id: r.work_order_id.clone(),
            tested_by: r.tested_by.clone(),
            test_date: r.test_date,
            status: r.status.clone(),
            measurements: r.measurements.clone(),
            notes: r.notes.clone(),
            passed: r.passed(),
            needs_retest: r.needs_retest(),
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(response))
}

// ============================================================================
// NON-CONFORMANCE REPORT (NCR) HANDLERS
// ============================================================================

/// Create NCR
pub async fn create_ncr(
    State(state): State<AppState>,
    Json(payload): Json<CreateNcrRequest>,
) -> Result<Json<NcrResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let ncr = non_conformance_report::ActiveModel {
        ncr_number: Set(payload.ncr_number),
        robot_serial_id: Set(payload.robot_serial_id),
        work_order_id: Set(payload.work_order_id),
        component_serial_id: Set(payload.component_serial_id),
        reported_by: Set(payload.reported_by),
        issue_type: Set(payload.issue_type),
        severity: Set(payload.severity),
        description: Set(payload.description),
        assigned_to: Set(payload.assigned_to),
        ..Default::default()
    };

    let saved = ncr
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(NcrResponse {
        id: saved.id,
        ncr_number: saved.ncr_number.clone(),
        robot_serial_id: saved.robot_serial_id,
        work_order_id: saved.work_order_id.clone(),
        component_serial_id: saved.component_serial_id,
        reported_by: saved.reported_by.clone(),
        reported_at: saved.reported_at,
        issue_type: saved.issue_type.clone(),
        severity: saved.severity.clone(),
        description: saved.description.clone(),
        root_cause: saved.root_cause.clone(),
        corrective_action: saved.corrective_action.clone(),
        preventive_action: saved.preventive_action.clone(),
        assigned_to: saved.assigned_to.clone(),
        status: saved.status.clone(),
        resolution_date: saved.resolution_date,
        disposition: saved.disposition.clone(),
        is_open: saved.is_open(),
        is_critical: saved.is_critical(),
        created_at: saved.created_at,
        updated_at: saved.updated_at,
    }))
}

/// List NCRs with filters
pub async fn list_ncrs(
    State(state): State<AppState>,
    Query(query): Query<ListNcrQuery>,
) -> Result<Json<Vec<NcrResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let mut select = non_conformance_report::Entity::find();

    if let Some(status) = query.status {
        select = select.filter(non_conformance_report::Column::Status.eq(status));
    }
    if let Some(severity) = query.severity {
        select = select.filter(non_conformance_report::Column::Severity.eq(severity));
    }
    if let Some(robot_id) = query.robot_serial_id {
        select = select.filter(non_conformance_report::Column::RobotSerialId.eq(robot_id));
    }
    if let Some(assigned_to) = query.assigned_to {
        select = select.filter(non_conformance_report::Column::AssignedTo.eq(assigned_to));
    }

    let ncrs = select
        .order_by_desc(non_conformance_report::Column::ReportedAt)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<NcrResponse> = ncrs
        .into_iter()
        .map(|ncr| NcrResponse {
            id: ncr.id,
            ncr_number: ncr.ncr_number.clone(),
            robot_serial_id: ncr.robot_serial_id,
            work_order_id: ncr.work_order_id.clone(),
            component_serial_id: ncr.component_serial_id,
            reported_by: ncr.reported_by.clone(),
            reported_at: ncr.reported_at,
            issue_type: ncr.issue_type.clone(),
            severity: ncr.severity.clone(),
            description: ncr.description.clone(),
            root_cause: ncr.root_cause.clone(),
            corrective_action: ncr.corrective_action.clone(),
            preventive_action: ncr.preventive_action.clone(),
            assigned_to: ncr.assigned_to.clone(),
            status: ncr.status.clone(),
            resolution_date: ncr.resolution_date,
            disposition: ncr.disposition.clone(),
            is_open: ncr.is_open(),
            is_critical: ncr.is_critical(),
            created_at: ncr.created_at,
            updated_at: ncr.updated_at,
        })
        .collect();

    Ok(Json(response))
}

/// Close an NCR
pub async fn close_ncr(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CloseNcrRequest>,
) -> Result<Json<NcrResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let ncr = non_conformance_report::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "NCR not found".to_string()))?;

    let mut ncr_model: non_conformance_report::ActiveModel = ncr.into();
    ncr_model.status = Set(non_conformance_report::NcrStatus::Closed);
    ncr_model.disposition = Set(Some(payload.disposition));
    ncr_model.resolution_date = Set(Some(chrono::Utc::now()));

    // Add resolution notes to corrective action if not already set
    if matches!(ncr_model.corrective_action, Set(None)) {
        ncr_model.corrective_action = Set(Some(payload.resolution_notes));
    }

    let updated = ncr_model
        .update(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(NcrResponse {
        id: updated.id,
        ncr_number: updated.ncr_number.clone(),
        robot_serial_id: updated.robot_serial_id,
        work_order_id: updated.work_order_id.clone(),
        component_serial_id: updated.component_serial_id,
        reported_by: updated.reported_by.clone(),
        reported_at: updated.reported_at,
        issue_type: updated.issue_type.clone(),
        severity: updated.severity.clone(),
        description: updated.description.clone(),
        root_cause: updated.root_cause.clone(),
        corrective_action: updated.corrective_action.clone(),
        preventive_action: updated.preventive_action.clone(),
        assigned_to: updated.assigned_to.clone(),
        status: updated.status.clone(),
        resolution_date: updated.resolution_date,
        disposition: updated.disposition.clone(),
        is_open: updated.is_open(),
        is_critical: updated.is_critical(),
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

// ============================================================================
// CERTIFICATION HANDLERS
// ============================================================================

/// Create certification
pub async fn create_certification(
    State(state): State<AppState>,
    Json(payload): Json<CreateCertificationRequest>,
) -> Result<Json<CertificationResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let cert = robot_certification::ActiveModel {
        robot_serial_id: Set(payload.robot_serial_id),
        certification_type: Set(payload.certification_type),
        certification_number: Set(payload.certification_number),
        issuing_authority: Set(payload.issuing_authority),
        issue_date: Set(payload.issue_date),
        expiration_date: Set(payload.expiration_date),
        certification_scope: Set(payload.certification_scope),
        certificate_document_url: Set(payload.certificate_document_url),
        ..Default::default()
    };

    let saved = cert
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CertificationResponse {
        id: saved.id,
        robot_serial_id: saved.robot_serial_id,
        robot_serial_number: None,
        certification_type: saved.certification_type.clone(),
        certification_number: saved.certification_number.clone(),
        issuing_authority: saved.issuing_authority.clone(),
        issue_date: saved.issue_date,
        expiration_date: saved.expiration_date,
        certification_scope: saved.certification_scope.clone(),
        certificate_document_url: saved.certificate_document_url.clone(),
        status: saved.status.clone(),
        is_valid: saved.is_valid(),
        days_until_expiration: saved.days_until_expiration(),
        needs_renewal: saved.needs_renewal(),
        created_at: saved.created_at,
        updated_at: saved.updated_at,
    }))
}

/// Get certifications for a robot
pub async fn get_robot_certifications(
    State(state): State<AppState>,
    Path(robot_id): Path<Uuid>,
) -> Result<Json<Vec<CertificationResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let certs = robot_certification::Entity::find()
        .filter(robot_certification::Column::RobotSerialId.eq(robot_id))
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<CertificationResponse> = certs
        .into_iter()
        .map(|c| CertificationResponse {
            id: c.id,
            robot_serial_id: c.robot_serial_id,
            robot_serial_number: None,
            certification_type: c.certification_type.clone(),
            certification_number: c.certification_number.clone(),
            issuing_authority: c.issuing_authority.clone(),
            issue_date: c.issue_date,
            expiration_date: c.expiration_date,
            certification_scope: c.certification_scope.clone(),
            certificate_document_url: c.certificate_document_url.clone(),
            status: c.status.clone(),
            is_valid: c.is_valid(),
            days_until_expiration: c.days_until_expiration(),
            needs_renewal: c.needs_renewal(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();

    Ok(Json(response))
}

// ============================================================================
// SERVICE HISTORY HANDLERS
// ============================================================================

/// Create service record
pub async fn create_service_record(
    State(state): State<AppState>,
    Json(payload): Json<CreateServiceRecordRequest>,
) -> Result<Json<ServiceRecordResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let service = robot_service_history::ActiveModel {
        robot_serial_id: Set(payload.robot_serial_id),
        service_ticket_number: Set(payload.service_ticket_number),
        service_type: Set(payload.service_type),
        service_date: Set(payload.service_date),
        technician_id: Set(payload.technician_id),
        description: Set(payload.description),
        ..Default::default()
    };

    let saved = service
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ServiceRecordResponse {
        id: saved.id,
        robot_serial_id: saved.robot_serial_id,
        robot_serial_number: None,
        service_ticket_number: saved.service_ticket_number.clone(),
        service_type: saved.service_type.clone(),
        service_date: saved.service_date,
        technician_id: saved.technician_id.clone(),
        description: saved.description.clone(),
        work_performed: saved.work_performed.clone(),
        parts_replaced: saved.parts_replaced.clone(),
        labor_hours: saved.labor_hours,
        service_cost: saved.service_cost,
        next_service_due: saved.next_service_due,
        status: saved.status.clone(),
        is_overdue: saved.is_overdue(),
        created_at: saved.created_at,
        updated_at: saved.updated_at,
    }))
}

/// Get service history for a robot
pub async fn get_robot_service_history(
    State(state): State<AppState>,
    Path(robot_id): Path<Uuid>,
) -> Result<Json<Vec<ServiceRecordResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let services = robot_service_history::Entity::find()
        .filter(robot_service_history::Column::RobotSerialId.eq(robot_id))
        .order_by_desc(robot_service_history::Column::ServiceDate)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<ServiceRecordResponse> = services
        .into_iter()
        .map(|s| ServiceRecordResponse {
            id: s.id,
            robot_serial_id: s.robot_serial_id,
            robot_serial_number: None,
            service_ticket_number: s.service_ticket_number.clone(),
            service_type: s.service_type.clone(),
            service_date: s.service_date,
            technician_id: s.technician_id.clone(),
            description: s.description.clone(),
            work_performed: s.work_performed.clone(),
            parts_replaced: s.parts_replaced.clone(),
            labor_hours: s.labor_hours,
            service_cost: s.service_cost,
            next_service_due: s.next_service_due,
            status: s.status.clone(),
            is_overdue: s.is_overdue(),
            created_at: s.created_at,
            updated_at: s.updated_at,
        })
        .collect();

    Ok(Json(response))
}

/// Complete service record
pub async fn complete_service_record(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CompleteServiceRequest>,
) -> Result<Json<ServiceRecordResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let service = robot_service_history::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Service record not found".to_string()))?;

    let mut service_model: robot_service_history::ActiveModel = service.into();
    service_model.work_performed = Set(Some(payload.work_performed));
    service_model.parts_replaced = Set(payload.parts_replaced);
    service_model.labor_hours = Set(payload.labor_hours);
    service_model.service_cost = Set(payload.service_cost);
    service_model.next_service_due = Set(payload.next_service_due);
    service_model.status = Set(robot_service_history::ServiceStatus::Completed);

    let updated = service_model
        .update(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ServiceRecordResponse {
        id: updated.id,
        robot_serial_id: updated.robot_serial_id,
        robot_serial_number: None,
        service_ticket_number: updated.service_ticket_number.clone(),
        service_type: updated.service_type.clone(),
        service_date: updated.service_date,
        technician_id: updated.technician_id.clone(),
        description: updated.description.clone(),
        work_performed: updated.work_performed.clone(),
        parts_replaced: updated.parts_replaced.clone(),
        labor_hours: updated.labor_hours,
        service_cost: updated.service_cost,
        next_service_due: updated.next_service_due,
        status: updated.status.clone(),
        is_overdue: updated.is_overdue(),
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

// ============================================================================
// PRODUCTION METRICS HANDLERS
// ============================================================================

/// Create production metrics
pub async fn create_production_metrics(
    State(state): State<AppState>,
    Json(payload): Json<CreateProductionMetricsRequest>,
) -> Result<Json<ProductionMetricsResponse>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let metrics = production_metrics::ActiveModel {
        production_date: Set(payload.production_date),
        shift: Set(payload.shift),
        production_line_id: Set(payload.production_line_id),
        product_id: Set(payload.product_id),
        robot_model: Set(payload.robot_model),
        planned_quantity: Set(payload.planned_quantity),
        actual_quantity: Set(payload.actual_quantity),
        quantity_passed: Set(payload.quantity_passed),
        quantity_failed: Set(payload.quantity_failed),
        quantity_rework: Set(payload.quantity_rework),
        planned_hours: Set(payload.planned_hours),
        actual_hours: Set(payload.actual_hours),
        downtime_hours: Set(payload.downtime_hours),
        downtime_reason: Set(payload.downtime_reason),
        ..Default::default()
    };

    let saved = metrics
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Calculate metrics
    let first_pass_yield = saved.calculate_first_pass_yield();
    let scrap_rate = saved.calculate_scrap_rate();
    let oee = saved.calculate_oee();

    Ok(Json(ProductionMetricsResponse {
        id: saved.id,
        production_date: saved.production_date,
        shift: saved.shift.clone(),
        production_line_id: saved.production_line_id,
        robot_model: saved.robot_model.clone(),
        planned_quantity: saved.planned_quantity,
        actual_quantity: saved.actual_quantity,
        quantity_passed: saved.quantity_passed,
        quantity_failed: saved.quantity_failed,
        quantity_rework: saved.quantity_rework,
        first_pass_yield,
        scrap_rate,
        planned_hours: saved.planned_hours,
        actual_hours: saved.actual_hours,
        downtime_hours: saved.downtime_hours,
        downtime_reason: saved.downtime_reason.clone(),
        oee,
        meets_target_oee: saved.meets_target_oee(),
    }))
}

/// Get production metrics with filters
pub async fn get_production_metrics(
    State(state): State<AppState>,
    Query(query): Query<ProductionMetricsQuery>,
) -> Result<Json<Vec<ProductionMetricsResponse>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let mut select = production_metrics::Entity::find();

    if let Some(start_date) = query.start_date {
        select = select.filter(production_metrics::Column::ProductionDate.gte(start_date));
    }
    if let Some(end_date) = query.end_date {
        select = select.filter(production_metrics::Column::ProductionDate.lte(end_date));
    }
    if let Some(line_id) = query.production_line_id {
        select = select.filter(production_metrics::Column::ProductionLineId.eq(line_id));
    }
    if let Some(robot_model) = query.robot_model {
        select = select.filter(production_metrics::Column::RobotModel.eq(robot_model));
    }
    if let Some(shift) = query.shift {
        select = select.filter(production_metrics::Column::Shift.eq(shift));
    }

    let metrics = select
        .order_by_desc(production_metrics::Column::ProductionDate)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<ProductionMetricsResponse> = metrics
        .into_iter()
        .map(|m| {
            let first_pass_yield = m.calculate_first_pass_yield();
            let scrap_rate = m.calculate_scrap_rate();
            let oee = m.calculate_oee();

            ProductionMetricsResponse {
                id: m.id,
                production_date: m.production_date,
                shift: m.shift.clone(),
                production_line_id: m.production_line_id,
                robot_model: m.robot_model.clone(),
                planned_quantity: m.planned_quantity,
                actual_quantity: m.actual_quantity,
                quantity_passed: m.quantity_passed,
                quantity_failed: m.quantity_failed,
                quantity_rework: m.quantity_rework,
                first_pass_yield,
                scrap_rate,
                planned_hours: m.planned_hours,
                actual_hours: m.actual_hours,
                downtime_hours: m.downtime_hours,
                downtime_reason: m.downtime_reason.clone(),
                oee,
                meets_target_oee: m.meets_target_oee(),
            }
        })
        .collect();

    Ok(Json(response))
}

// ============================================================================
// PRODUCTION LINE HANDLERS
// ============================================================================

/// Create production line
pub async fn create_production_line(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let line = production_line::ActiveModel {
        line_number: Set(payload["line_number"]
            .as_str()
            .ok_or((StatusCode::BAD_REQUEST, "line_number required".to_string()))?
            .to_string()),
        name: Set(payload["name"]
            .as_str()
            .ok_or((StatusCode::BAD_REQUEST, "name required".to_string()))?
            .to_string()),
        location: Set(payload["location"].as_str().map(|s| s.to_string())),
        ..Default::default()
    };

    let saved = line
        .insert(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "id": saved.id,
        "line_number": saved.line_number.clone(),
        "name": saved.name.clone(),
        "status": saved.status.clone(),
        "is_available": saved.is_available(),
        "created_at": saved.created_at,
    })))
}

/// List production lines
pub async fn list_production_lines(
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, (StatusCode, String)> {
    let db = state.db.as_ref();

    let lines = production_line::Entity::find()
        .order_by_asc(production_line::Column::LineNumber)
        .all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<Value> = lines
        .into_iter()
        .map(|line| {
            json!({
                "id": line.id,
                "line_number": line.line_number,
                "name": line.name,
                "line_type": line.line_type,
                "location": line.location,
                "capacity_units_per_day": line.capacity_units_per_day,
                "status": line.status,
                "is_available": line.is_available(),
                "created_at": line.created_at,
            })
        })
        .collect();

    Ok(Json(response))
}
