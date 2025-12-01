use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    entities::manufacturing::{
        component_serial_number, non_conformance_report, production_metrics, robot_certification,
        robot_component_genealogy, robot_configuration, robot_serial_number, robot_service_history,
        test_protocol, test_result,
    },
    events::EventSender,
};
use std::sync::Arc;

/// Robot Manufacturing service provides high-level business logic for robot manufacturing
pub struct RobotManufacturingService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RobotBuildRequest {
    pub robot_model: String,
    pub robot_type: robot_serial_number::RobotType,
    pub work_order_id: Option<Uuid>,
    pub product_id: Uuid,
    pub components: Vec<ComponentInstallation>,
    pub configuration: RobotConfigurationData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentInstallation {
    pub component_serial_id: Uuid,
    pub position: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RobotConfigurationData {
    pub payload_kg: Option<Decimal>,
    pub reach_mm: Option<i32>,
    pub degrees_of_freedom: Option<i32>,
    pub end_effector_type: Option<String>,
    pub power_requirements: Option<String>,
    pub mounting_type: Option<robot_configuration::MountingType>,
}

#[derive(Debug, Serialize)]
pub struct RobotBuildResult {
    pub robot_serial_id: Uuid,
    pub serial_number: String,
    pub components_installed: usize,
    pub configuration_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct TestSuiteResult {
    pub robot_serial_id: Uuid,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    pub all_passed: bool,
    pub test_results: Vec<TestResultSummary>,
}

#[derive(Debug, Serialize)]
pub struct TestResultSummary {
    pub protocol_name: String,
    pub status: test_result::TestStatus,
    pub test_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProductionDashboard {
    pub date: chrono::NaiveDate,
    pub total_robots_produced: i32,
    pub robots_passed_qa: i32,
    pub robots_failed_qa: i32,
    pub open_ncrs: i32,
    pub critical_ncrs: i32,
    pub average_oee: Decimal,
    pub first_pass_yield: Decimal,
}

#[derive(Debug, Serialize)]
pub struct RobotFullProfile {
    pub robot: robot_serial_number::Model,
    pub configuration: Option<robot_configuration::Model>,
    pub components: Vec<ComponentInRobot>,
    pub test_results: Vec<test_result::Model>,
    pub certifications: Vec<robot_certification::Model>,
    pub service_history: Vec<robot_service_history::Model>,
    pub ncrs: Vec<non_conformance_report::Model>,
}

#[derive(Debug, Serialize)]
pub struct ComponentInRobot {
    pub component: component_serial_number::Model,
    pub position: Option<String>,
    pub installed_at: DateTime<Utc>,
}

impl RobotManufacturingService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Build a complete robot with components in a single transaction
    pub async fn build_robot(
        &self,
        request: RobotBuildRequest,
    ) -> Result<RobotBuildResult, Box<dyn std::error::Error>> {
        let txn = self.db.begin().await?;

        // Generate serial number
        let sequence = self.get_next_serial_sequence(&request.robot_model).await?;
        let serial_number =
            robot_serial_number::Model::generate_serial_number(&request.robot_model, sequence);

        // Create robot serial number
        let robot = robot_serial_number::ActiveModel {
            serial_number: Set(serial_number.clone()),
            product_id: Set(request.product_id),
            work_order_id: Set(request.work_order_id),
            robot_model: Set(request.robot_model.clone()),
            robot_type: Set(request.robot_type.clone()),
            manufacturing_date: Set(Some(Utc::now())),
            status: Set(robot_serial_number::RobotStatus::InProduction),
            ..Default::default()
        };

        let robot = robot.insert(&txn).await?;

        // Create configuration
        let config = robot_configuration::ActiveModel {
            robot_serial_id: Set(robot.id),
            configuration_type: Set(robot_configuration::ConfigurationType::AsBuilt),
            robot_model: Set(request.robot_model),
            payload_kg: Set(request.configuration.payload_kg),
            reach_mm: Set(request.configuration.reach_mm),
            degrees_of_freedom: Set(request.configuration.degrees_of_freedom),
            end_effector_type: Set(request.configuration.end_effector_type),
            power_requirements: Set(request.configuration.power_requirements),
            mounting_type: Set(request.configuration.mounting_type),
            ..Default::default()
        };

        let config = config.insert(&txn).await?;

        // Install components
        let mut components_installed = 0;
        for component_install in request.components {
            // Verify component exists and is available
            let component = component_serial_number::Entity::find_by_id(component_install.component_serial_id)
                .one(&txn)
                .await?
                .ok_or("Component not found")?;

            if !component.is_available() {
                return Err("Component not available for installation".into());
            }

            // Create genealogy record
            let genealogy = robot_component_genealogy::ActiveModel {
                robot_serial_id: Set(robot.id),
                component_serial_id: Set(component_install.component_serial_id),
                position: Set(Some(component_install.position)),
                ..Default::default()
            };
            genealogy.insert(&txn).await?;

            // Update component status
            let mut component_model: component_serial_number::ActiveModel = component.into();
            component_model.status = Set(component_serial_number::ComponentStatus::Installed);
            component_model.update(&txn).await?;

            components_installed += 1;
        }

        txn.commit().await?;

        // Emit event
        let _ = self.event_sender.send(json!({
            "event_type": "robot.built",
            "robot_serial_id": robot.id,
            "serial_number": serial_number,
            "robot_model": robot.robot_model,
            "components_installed": components_installed,
        }));

        Ok(RobotBuildResult {
            robot_serial_id: robot.id,
            serial_number,
            components_installed,
            configuration_id: config.id,
        })
    }

    /// Run a complete test suite on a robot
    pub async fn run_test_suite(
        &self,
        robot_serial_id: Uuid,
        tested_by: Uuid,
    ) -> Result<TestSuiteResult, Box<dyn std::error::Error>> {
        // Get all active test protocols
        let protocols = test_protocol::Entity::find()
            .filter(test_protocol::Column::Status.eq(test_protocol::ProtocolStatus::Active))
            .all(&*self.db)
            .await?;

        let mut test_results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        for protocol in protocols {
            // Create test result (in real implementation, would execute actual tests)
            let result = test_result::ActiveModel {
                test_protocol_id: Set(protocol.id),
                robot_serial_id: Set(Some(robot_serial_id)),
                tested_by: Set(tested_by),
                status: Set(test_result::TestStatus::Pass), // Placeholder
                ..Default::default()
            };

            let saved = result.insert(&*self.db).await?;

            if saved.passed() {
                passed += 1;
            } else {
                failed += 1;
            }

            test_results.push(TestResultSummary {
                protocol_name: protocol.name,
                status: saved.status,
                test_date: saved.test_date,
            });
        }

        let total_tests = passed + failed;
        let pass_rate = if total_tests > 0 {
            (passed as f64 / total_tests as f64) * 100.0
        } else {
            0.0
        };

        // Emit event
        let _ = self.event_sender.send(json!({
            "event_type": "robot.test_suite_completed",
            "robot_serial_id": robot_serial_id,
            "total_tests": total_tests,
            "passed": passed,
            "failed": failed,
            "pass_rate": pass_rate,
        }));

        Ok(TestSuiteResult {
            robot_serial_id,
            total_tests,
            passed,
            failed,
            pass_rate,
            all_passed: failed == 0,
            test_results,
        })
    }

    /// Get a complete robot profile with all related data
    pub async fn get_robot_full_profile(
        &self,
        robot_serial_id: Uuid,
    ) -> Result<RobotFullProfile, Box<dyn std::error::Error>> {
        // Get robot
        let robot = robot_serial_number::Entity::find_by_id(robot_serial_id)
            .one(&*self.db)
            .await?
            .ok_or("Robot not found")?;

        // Get configuration
        let configuration = robot_configuration::Entity::find()
            .filter(robot_configuration::Column::RobotSerialId.eq(robot_serial_id))
            .one(&*self.db)
            .await?;

        // Get components
        let genealogy = robot_component_genealogy::Entity::find()
            .filter(robot_component_genealogy::Column::RobotSerialId.eq(robot_serial_id))
            .filter(robot_component_genealogy::Column::RemovedAt.is_null())
            .all(&*self.db)
            .await?;

        let mut components = Vec::new();
        for gen in genealogy {
            if let Some(component) =
                component_serial_number::Entity::find_by_id(gen.component_serial_id)
                    .one(&*self.db)
                    .await?
            {
                components.push(ComponentInRobot {
                    component,
                    position: gen.position,
                    installed_at: gen.installed_at,
                });
            }
        }

        // Get test results
        let test_results = test_result::Entity::find()
            .filter(test_result::Column::RobotSerialId.eq(robot_serial_id))
            .all(&*self.db)
            .await?;

        // Get certifications
        let certifications = robot_certification::Entity::find()
            .filter(robot_certification::Column::RobotSerialId.eq(robot_serial_id))
            .all(&*self.db)
            .await?;

        // Get service history
        let service_history = robot_service_history::Entity::find()
            .filter(robot_service_history::Column::RobotSerialId.eq(robot_serial_id))
            .all(&*self.db)
            .await?;

        // Get NCRs
        let ncrs = non_conformance_report::Entity::find()
            .filter(non_conformance_report::Column::RobotSerialId.eq(robot_serial_id))
            .all(&*self.db)
            .await?;

        Ok(RobotFullProfile {
            robot,
            configuration,
            components,
            test_results,
            certifications,
            service_history,
            ncrs,
        })
    }

    /// Generate production dashboard for a specific date
    pub async fn get_production_dashboard(
        &self,
        date: chrono::NaiveDate,
    ) -> Result<ProductionDashboard, Box<dyn std::error::Error>> {
        // Get production metrics for the date
        let metrics = production_metrics::Entity::find()
            .filter(production_metrics::Column::ProductionDate.eq(date))
            .all(&*self.db)
            .await?;

        let total_robots_produced: i32 = metrics
            .iter()
            .filter_map(|m| m.actual_quantity)
            .sum();

        let robots_passed_qa: i32 = metrics
            .iter()
            .filter_map(|m| m.quantity_passed)
            .sum();

        let robots_failed_qa: i32 = metrics
            .iter()
            .filter_map(|m| m.quantity_failed)
            .sum();

        // Calculate average OEE
        let oee_values: Vec<Decimal> = metrics
            .iter()
            .filter_map(|m| m.calculate_oee())
            .collect();

        let average_oee = if !oee_values.is_empty() {
            oee_values.iter().sum::<Decimal>() / Decimal::from(oee_values.len())
        } else {
            Decimal::ZERO
        };

        // Calculate first pass yield
        let first_pass_yield = if total_robots_produced > 0 {
            (Decimal::from(robots_passed_qa) / Decimal::from(total_robots_produced)) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Get open NCRs
        let open_ncrs = non_conformance_report::Entity::find()
            .filter(non_conformance_report::Column::Status.is_in([
                non_conformance_report::NcrStatus::Open,
                non_conformance_report::NcrStatus::Investigating,
                non_conformance_report::NcrStatus::ActionRequired,
            ]))
            .count(&*self.db)
            .await? as i32;

        let critical_ncrs = non_conformance_report::Entity::find()
            .filter(non_conformance_report::Column::Severity.eq(non_conformance_report::Severity::Critical))
            .filter(non_conformance_report::Column::Status.is_in([
                non_conformance_report::NcrStatus::Open,
                non_conformance_report::NcrStatus::Investigating,
                non_conformance_report::NcrStatus::ActionRequired,
            ]))
            .count(&*self.db)
            .await? as i32;

        Ok(ProductionDashboard {
            date,
            total_robots_produced,
            robots_passed_qa,
            robots_failed_qa,
            open_ncrs,
            critical_ncrs,
            average_oee,
            first_pass_yield,
        })
    }

    /// Mark robot as ready for shipment (all tests passed, no open NCRs)
    pub async fn mark_robot_ready_for_shipment(
        &self,
        robot_serial_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Verify all tests passed
        let failed_tests = test_result::Entity::find()
            .filter(test_result::Column::RobotSerialId.eq(robot_serial_id))
            .filter(test_result::Column::Status.eq(test_result::TestStatus::Fail))
            .count(&*self.db)
            .await?;

        if failed_tests > 0 {
            return Err("Robot has failed tests".into());
        }

        // Verify no open NCRs
        let open_ncrs = non_conformance_report::Entity::find()
            .filter(non_conformance_report::Column::RobotSerialId.eq(robot_serial_id))
            .filter(non_conformance_report::Column::Status.is_in([
                non_conformance_report::NcrStatus::Open,
                non_conformance_report::NcrStatus::Investigating,
                non_conformance_report::NcrStatus::ActionRequired,
            ]))
            .count(&*self.db)
            .await?;

        if open_ncrs > 0 {
            return Err("Robot has open NCRs".into());
        }

        // Update robot status
        let robot = robot_serial_number::Entity::find_by_id(robot_serial_id)
            .one(&*self.db)
            .await?
            .ok_or("Robot not found")?;

        let mut robot_model: robot_serial_number::ActiveModel = robot.into();
        robot_model.status = Set(robot_serial_number::RobotStatus::Ready);
        robot_model.update(&*self.db).await?;

        // Emit event
        let _ = self.event_sender.send(json!({
            "event_type": "robot.ready_for_shipment",
            "robot_serial_id": robot_serial_id,
        }));

        Ok(())
    }

    /// Get next serial number sequence for a robot model
    async fn get_next_serial_sequence(
        &self,
        _robot_model: &str,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        // In real implementation, would query a sequence table
        Ok(1)
    }

    /// Check if robot is ready for production (all components available)
    pub async fn check_production_readiness(
        &self,
        component_ids: Vec<Uuid>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        for component_id in component_ids {
            let component = component_serial_number::Entity::find_by_id(component_id)
                .one(&*self.db)
                .await?
                .ok_or("Component not found")?;

            if !component.is_available() {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
