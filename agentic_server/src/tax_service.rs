use crate::errors::ServiceError;
use crate::models::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Tax calculation service
#[derive(Clone)]
pub struct TaxService {
    tax_rates: Arc<RwLock<HashMap<String, TaxRate>>>,
}

use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRate {
    pub state: String,
    pub rate: f64, // Percentage (e.g., 8.75 for 8.75%)
    pub county_rate: Option<f64>,
    pub city_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaxCalculation {
    pub subtotal: i64,
    pub tax_amount: i64,
    pub tax_rate: f64,
    pub jurisdiction: String,
}

impl TaxService {
    pub fn new() -> Self {
        let mut tax_rates = HashMap::new();

        // US state tax rates (simplified)
        tax_rates.insert(
            "CA".to_string(),
            TaxRate {
                state: "CA".to_string(),
                rate: 7.25,
                county_rate: Some(1.0),
                city_rate: Some(0.5),
            },
        );

        tax_rates.insert(
            "NY".to_string(),
            TaxRate {
                state: "NY".to_string(),
                rate: 4.0,
                county_rate: Some(4.0),
                city_rate: Some(0.375),
            },
        );

        tax_rates.insert(
            "TX".to_string(),
            TaxRate {
                state: "TX".to_string(),
                rate: 6.25,
                county_rate: Some(1.0),
                city_rate: Some(1.0),
            },
        );

        tax_rates.insert(
            "FL".to_string(),
            TaxRate {
                state: "FL".to_string(),
                rate: 6.0,
                county_rate: Some(1.0),
                city_rate: Some(0.5),
            },
        );

        // Default rate for other states
        tax_rates.insert(
            "DEFAULT".to_string(),
            TaxRate {
                state: "DEFAULT".to_string(),
                rate: 5.0,
                county_rate: None,
                city_rate: None,
            },
        );

        Self {
            tax_rates: Arc::new(RwLock::new(tax_rates)),
        }
    }

    /// Calculate tax based on address and amount
    pub fn calculate_tax(
        &self,
        subtotal: i64,
        address: &Address,
        include_shipping: bool,
        shipping_amount: i64,
    ) -> Result<TaxCalculation, ServiceError> {
        let tax_rates = self.tax_rates.read().unwrap();

        // Get tax rate for state
        let tax_rate = tax_rates
            .get(&address.state)
            .or_else(|| tax_rates.get("DEFAULT"))
            .ok_or_else(|| ServiceError::InternalError("No tax rate configured".to_string()))?;

        // Calculate total rate
        let mut total_rate = tax_rate.rate;
        if let Some(county) = tax_rate.county_rate {
            total_rate += county;
        }
        if let Some(city) = tax_rate.city_rate {
            total_rate += city;
        }

        // Calculate taxable amount
        let taxable_amount = if include_shipping {
            subtotal + shipping_amount
        } else {
            subtotal
        };

        // Calculate tax (amount * rate / 100)
        let tax_amount = (taxable_amount as f64 * total_rate / 100.0).round() as i64;

        debug!(
            "Tax calculation: ${} @ {}% = ${}",
            taxable_amount as f64 / 100.0,
            total_rate,
            tax_amount as f64 / 100.0
        );

        Ok(TaxCalculation {
            subtotal,
            tax_amount,
            tax_rate: total_rate,
            jurisdiction: format!("{}, US", address.state),
        })
    }

    /// Get tax rate for a state
    pub fn get_tax_rate(&self, state: &str) -> Option<TaxRate> {
        let tax_rates = self.tax_rates.read().unwrap();
        tax_rates
            .get(state)
            .or_else(|| tax_rates.get("DEFAULT"))
            .cloned()
    }

    /// Add or update tax rate
    pub fn set_tax_rate(&self, tax_rate: TaxRate) {
        let mut tax_rates = self.tax_rates.write().unwrap();
        tax_rates.insert(tax_rate.state.clone(), tax_rate);
        info!("Updated tax rate for state");
    }
}

impl Default for TaxService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address() -> Address {
        Address {
            name: "Test User".to_string(),
            line_one: "123 Main St".to_string(),
            line_two: None,
            city: "San Francisco".to_string(),
            state: "CA".to_string(),
            country: "US".to_string(),
            postal_code: "94102".to_string(),
        }
    }

    #[test]
    fn test_tax_calculation() {
        let service = TaxService::new();
        let address = test_address();

        let result = service.calculate_tax(10000, &address, false, 0).unwrap();

        // CA rate: 7.25 + 1.0 + 0.5 = 8.75%
        // $100.00 * 8.75% = $8.75 = 875 cents
        assert_eq!(result.tax_amount, 875);
        assert_eq!(result.tax_rate, 8.75);
    }

    #[test]
    fn test_tax_with_shipping() {
        let service = TaxService::new();
        let address = test_address();

        let result = service.calculate_tax(10000, &address, true, 1000).unwrap();

        // $110.00 * 8.75% = $9.625 = 963 cents (rounded)
        assert_eq!(result.tax_amount, 963);
    }

    #[test]
    fn test_different_states() {
        let service = TaxService::new();

        let ca_addr = Address {
            name: "Test".to_string(),
            line_one: "123 St".to_string(),
            line_two: None,
            city: "San Francisco".to_string(),
            state: "CA".to_string(),
            country: "US".to_string(),
            postal_code: "94102".to_string(),
        };

        let tx_addr = Address {
            name: "Test".to_string(),
            line_one: "456 St".to_string(),
            line_two: None,
            city: "Austin".to_string(),
            state: "TX".to_string(),
            country: "US".to_string(),
            postal_code: "78701".to_string(),
        };

        let ca_tax = service.calculate_tax(10000, &ca_addr, false, 0).unwrap();
        let tx_tax = service.calculate_tax(10000, &tx_addr, false, 0).unwrap();

        // CA has higher rate than TX
        assert!(ca_tax.tax_amount > tx_tax.tax_amount);
    }
}
