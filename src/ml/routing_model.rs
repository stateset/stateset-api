/*!
 * # Order Routing Model
 *
 * This module provides intelligent routing for orders to optimize fulfillment efficiency.
 * It takes into account inventory levels, facility capacity, shipping costs, and delivery times.
 */

use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for the routing model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingModelConfig {
    /// Weight factor for shipping cost (0.0 to 1.0)
    pub cost_weight: f64,
    /// Weight factor for delivery time (0.0 to 1.0)  
    pub time_weight: f64,
    /// Weight factor for inventory availability (0.0 to 1.0)
    pub inventory_weight: f64,
    /// Weight factor for facility capacity (0.0 to 1.0)
    pub capacity_weight: f64,
    /// Maximum routing candidates to consider
    pub max_candidates: u32,
}

impl Default for RoutingModelConfig {
    fn default() -> Self {
        Self {
            cost_weight: 0.3,
            time_weight: 0.3,
            inventory_weight: 0.3,
            capacity_weight: 0.1,
            max_candidates: 5,
        }
    }
}

/// Routing decision with scoring details
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// The recommended facility ID
    pub facility_id: Uuid,
    /// Overall routing score (0.0 to 1.0, higher is better)
    pub score: f64,
    /// Estimated shipping cost
    pub estimated_cost: f64,
    /// Estimated delivery time in hours
    pub estimated_delivery_hours: u32,
    /// Confidence in the routing decision
    pub confidence: f64,
    /// Detailed scoring breakdown
    pub scoring_details: RoutingScoreDetails,
}

/// Detailed scoring breakdown for routing decisions
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingScoreDetails {
    pub cost_score: f64,
    pub time_score: f64,
    pub inventory_score: f64,
    pub capacity_score: f64,
}

/// Input parameters for routing decisions
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingRequest {
    /// Order ID being routed
    pub order_id: Uuid,
    /// Items in the order with quantities
    pub items: Vec<OrderItem>,
    /// Delivery address
    pub delivery_address: DeliveryAddress,
    /// Priority level (1-5, higher is more urgent)
    pub priority: u8,
    /// Required delivery date (if any)
    pub required_delivery_date: Option<chrono::DateTime<chrono::Utc>>,
}

/// Order item for routing
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: Uuid,
    pub quantity: u32,
    pub weight: Option<f64>,
    pub dimensions: Option<Dimensions>,
}

/// Product dimensions
#[derive(Debug, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
}

/// Delivery address
#[derive(Debug, Serialize, Deserialize)]
pub struct DeliveryAddress {
    pub street: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// Facility information for routing
#[derive(Debug, Serialize, Deserialize)]
pub struct FacilityInfo {
    pub id: Uuid,
    pub name: String,
    pub address: DeliveryAddress,
    pub capacity_utilization: f64, // 0.0 to 1.0
    pub inventory_levels: HashMap<Uuid, u32>, // product_id -> quantity
    pub processing_time_hours: u32,
    pub shipping_zones: Vec<String>,
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius_km = 6371.0_f64;
    let (lat1_rad, lon1_rad) = (lat1.to_radians(), lon1.to_radians());
    let (lat2_rad, lon2_rad) = (lat2.to_radians(), lon2.to_radians());
    let dlat = lat2_rad - lat1_rad;
    let dlon = lon2_rad - lon1_rad;
    let a = (dlat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    radius_km * c
}

fn maybe_distance_km(request: &RoutingRequest, facility: &FacilityInfo) -> Option<f64> {
    let lat1 = request.delivery_address.latitude?;
    let lon1 = request.delivery_address.longitude?;
    let lat2 = facility.address.latitude?;
    let lon2 = facility.address.longitude?;
    Some(haversine_km(lat1, lon1, lat2, lon2))
}

/// Main routing model
pub struct RoutingModel {
    config: RoutingModelConfig,
}

impl RoutingModel {
    /// Create a new routing model with default configuration
    pub fn new() -> Self {
        Self {
            config: RoutingModelConfig::default(),
        }
    }

    /// Create a new routing model with custom configuration
    pub fn with_config(config: RoutingModelConfig) -> Self {
        Self { config }
    }

    /// Route an order to the optimal facility
    pub async fn route_order(
        &self,
        request: &RoutingRequest,
        available_facilities: &[FacilityInfo],
    ) -> Result<RoutingDecision, ServiceError> {
        if available_facilities.is_empty() {
            return Err(ServiceError::ValidationError(
                "No facilities available for routing".to_string(),
            ));
        }

        let mut candidates = Vec::new();

        // Score each facility
        for facility in available_facilities.iter().take(self.config.max_candidates as usize) {
            if let Ok(decision) = self.score_facility(request, facility).await {
                candidates.push(decision);
            }
        }

        if candidates.is_empty() {
            return Err(ServiceError::InternalError(
                "No valid routing candidates found".to_string(),
            ));
        }

        // Sort by score and return the best option
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let best = candidates
            .into_iter()
            .next()
            .ok_or_else(|| {
                ServiceError::InternalError("No valid routing candidates found".to_string())
            })?;
        Ok(best)
    }

    /// Score a facility for a routing request
    async fn score_facility(
        &self,
        request: &RoutingRequest,
        facility: &FacilityInfo,
    ) -> Result<RoutingDecision, ServiceError> {
        // Calculate individual scores
        let cost_score = self.calculate_cost_score(request, facility).await?;
        let time_score = self.calculate_time_score(request, facility).await?;
        let inventory_score = self.calculate_inventory_score(request, facility).await?;
        let capacity_score = self.calculate_capacity_score(facility).await?;

        // Calculate weighted overall score
        let overall_score = (cost_score * self.config.cost_weight)
            + (time_score * self.config.time_weight)
            + (inventory_score * self.config.inventory_weight)
            + (capacity_score * self.config.capacity_weight);

        let (estimated_cost, estimated_delivery_hours) =
            if let Some(distance_km) = maybe_distance_km(request, facility) {
                let transit_hours = (distance_km / 60.0).ceil() as u32;
                let estimated_cost = 5.0 + distance_km * 0.12;
                let estimated_delivery_hours = facility.processing_time_hours + transit_hours;
                (estimated_cost, estimated_delivery_hours)
            } else {
                let estimated_cost = 10.0 + (1.0 - cost_score) * 50.0;
                let estimated_delivery_hours = 24 + ((1.0 - time_score) * 120.0) as u32;
                (estimated_cost, estimated_delivery_hours)
            };

        Ok(RoutingDecision {
            facility_id: facility.id,
            score: overall_score,
            estimated_cost,
            estimated_delivery_hours,
            confidence: (0.5 + overall_score / 2.0).clamp(0.0, 1.0),
            scoring_details: RoutingScoreDetails {
                cost_score,
                time_score,
                inventory_score,
                capacity_score,
            },
        })
    }

    /// Calculate cost score for a facility (0.0 to 1.0, higher is better)
    async fn calculate_cost_score(
        &self,
        request: &RoutingRequest,
        facility: &FacilityInfo,
    ) -> Result<f64, ServiceError> {
        if let Some(distance_km) = maybe_distance_km(request, facility) {
            let score = 1.0 / (1.0 + distance_km / 500.0);
            Ok(score.clamp(0.0, 1.0))
        } else {
            Ok(0.7)
        }
    }

    /// Calculate time score for a facility (0.0 to 1.0, higher is better)
    async fn calculate_time_score(
        &self,
        request: &RoutingRequest,
        facility: &FacilityInfo,
    ) -> Result<f64, ServiceError> {
        let processing_factor = 1.0 - (facility.processing_time_hours as f64 / 72.0).min(1.0);
        let transit_factor = if let Some(distance_km) = maybe_distance_km(request, facility) {
            let transit_hours = distance_km / 60.0;
            (1.0 / (1.0 + transit_hours / 48.0)).clamp(0.0, 1.0)
        } else {
            0.8
        };
        Ok((processing_factor * transit_factor).clamp(0.0, 1.0))
    }

    /// Calculate inventory score for a facility (0.0 to 1.0, higher is better)
    async fn calculate_inventory_score(
        &self,
        request: &RoutingRequest,
        facility: &FacilityInfo,
    ) -> Result<f64, ServiceError> {
        let mut total_score = 0.0;
        let mut item_count = 0;

        // Check inventory availability for each item
        for item in &request.items {
            if let Some(&available_quantity) = facility.inventory_levels.get(&item.product_id) {
                let availability_ratio = (available_quantity as f64 / item.quantity as f64).min(1.0);
                total_score += availability_ratio;
            }
            item_count += 1;
        }

        if item_count == 0 {
            return Ok(0.0);
        }

        Ok(total_score / item_count as f64)
    }

    /// Calculate capacity score for a facility (0.0 to 1.0, higher is better)
    async fn calculate_capacity_score(
        &self,
        facility: &FacilityInfo,
    ) -> Result<f64, ServiceError> {
        // Higher score for facilities with lower capacity utilization
        Ok(1.0 - facility.capacity_utilization)
    }

    /// Update model configuration
    pub fn update_config(&mut self, config: RoutingModelConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &RoutingModelConfig {
        &self.config
    }
}

impl Default for RoutingModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_routing_model_creation() {
        let model = RoutingModel::new();
        assert_eq!(model.config.cost_weight, 0.3);
        assert_eq!(model.config.time_weight, 0.3);
        assert_eq!(model.config.inventory_weight, 0.3);
        assert_eq!(model.config.capacity_weight, 0.1);
    }

    #[tokio::test]
    async fn test_routing_with_no_facilities() {
        let model = RoutingModel::new();
        let request = RoutingRequest {
            order_id: Uuid::new_v4(),
            items: vec![],
            delivery_address: DeliveryAddress {
                street: "123 Main St".to_string(),
                city: "Anytown".to_string(),
                state: "CA".to_string(),
                country: "USA".to_string(),
                postal_code: "12345".to_string(),
                latitude: None,
                longitude: None,
            },
            priority: 1,
            required_delivery_date: None,
        };

        let result = model.route_order(&request, &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_facility_scoring() {
        let model = RoutingModel::new();
        let facility = FacilityInfo {
            id: Uuid::new_v4(),
            name: "Test Facility".to_string(),
            address: DeliveryAddress {
                street: "456 Warehouse Ave".to_string(),
                city: "Facility City".to_string(),
                state: "CA".to_string(),
                country: "USA".to_string(),
                postal_code: "54321".to_string(),
                latitude: None,
                longitude: None,
            },
            capacity_utilization: 0.5,
            inventory_levels: HashMap::new(),
            processing_time_hours: 24,
            shipping_zones: vec!["Zone1".to_string()],
        };

        let request = RoutingRequest {
            order_id: Uuid::new_v4(),
            items: vec![],
            delivery_address: DeliveryAddress {
                street: "123 Main St".to_string(),
                city: "Anytown".to_string(),
                state: "CA".to_string(),
                country: "USA".to_string(),
                postal_code: "12345".to_string(),
                latitude: None,
                longitude: None,
            },
            priority: 1,
            required_delivery_date: None,
        };

        let result = model.score_facility(&request, &facility).await;
        assert!(result.is_ok());
        let decision = result.unwrap();
        assert_eq!(decision.facility_id, facility.id);
        assert!(decision.score >= 0.0 && decision.score <= 1.0);
    }
}
