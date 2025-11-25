/// Production-Ready Tax Service with Multiple Provider Support
///
/// Supports:
/// - TaxJar API integration
/// - Avalara AvaTax integration
/// - Fallback to rule-based calculation
/// - Tax calculation caching
/// - Multi-jurisdiction support
use crate::errors::ServiceError;
use crate::models::Address;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Tax calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxResult {
    /// Tax amount in minor units (cents)
    pub tax_amount: i64,
    /// Effective tax rate as decimal (0.0875 = 8.75%)
    pub tax_rate: f64,
    /// Breakdown by jurisdiction
    pub breakdown: Vec<TaxBreakdown>,
    /// Whether this was a cached result
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxBreakdown {
    /// Jurisdiction name (e.g., "California", "Los Angeles County")
    pub jurisdiction: String,
    /// Tax rate for this jurisdiction
    pub rate: f64,
    /// Tax amount for this jurisdiction
    pub amount: i64,
    /// Type: state, county, city, district
    pub tax_type: String,
}

/// Tax service configuration
#[derive(Clone)]
pub struct TaxServiceConfig {
    /// Primary provider: taxjar, avalara, or fallback
    pub provider: TaxProvider,
    /// API key for the tax provider
    pub api_key: Option<String>,
    /// Account ID (for Avalara)
    pub account_id: Option<String>,
    /// Company code (for Avalara)
    pub company_code: Option<String>,
    /// Enable caching
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
}

#[derive(Clone, Debug)]
pub enum TaxProvider {
    TaxJar,
    Avalara,
    Fallback,
}

impl TaxServiceConfig {
    pub fn from_env() -> Self {
        let provider_str = std::env::var("TAX_PROVIDER")
            .unwrap_or_else(|_| "fallback".to_string())
            .to_lowercase();

        let provider = match provider_str.as_str() {
            "taxjar" => TaxProvider::TaxJar,
            "avalara" => TaxProvider::Avalara,
            _ => TaxProvider::Fallback,
        };

        let api_key = std::env::var("TAX_API_KEY").ok();
        let account_id = std::env::var("AVALARA_ACCOUNT_ID").ok();
        let company_code = std::env::var("AVALARA_COMPANY_CODE").ok();

        let enable_cache = std::env::var("TAX_CACHE_ENABLED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        let cache_ttl_secs = std::env::var("TAX_CACHE_TTL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600); // 1 hour default

        Self {
            provider,
            api_key,
            account_id,
            company_code,
            enable_cache,
            cache_ttl_secs,
        }
    }
}

/// Enhanced tax service with multiple providers
pub struct TaxService {
    config: TaxServiceConfig,
    client: Client,
    /// Simple in-memory cache (use Redis in production)
    cache: Arc<RwLock<HashMap<String, (TaxResult, std::time::Instant)>>>,
}

impl TaxService {
    pub fn new(config: TaxServiceConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            config,
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Calculate tax for an order
    ///
    /// # Arguments
    /// * `subtotal` - Order subtotal in minor units (cents)
    /// * `address` - Shipping/billing address
    /// * `include_shipping` - Whether to tax shipping
    /// * `shipping_amount` - Shipping amount in minor units
    ///
    /// # Returns
    /// Tax calculation result with breakdown
    #[instrument(skip(self))]
    pub async fn calculate_tax(
        &self,
        subtotal: i64,
        address: &Address,
        include_shipping: bool,
        shipping_amount: i64,
    ) -> Result<TaxResult, ServiceError> {
        // Check cache first
        if self.config.enable_cache {
            let cache_key = self.generate_cache_key(subtotal, address, include_shipping, shipping_amount);

            let cache = self.cache.read().await;
            if let Some((result, cached_at)) = cache.get(&cache_key) {
                let age = cached_at.elapsed().as_secs();
                if age < self.config.cache_ttl_secs {
                    debug!(cache_key = %cache_key, age_secs = age, "Tax cache hit");
                    let mut cached_result = result.clone();
                    cached_result.cached = true;
                    return Ok(cached_result);
                }
            }
            drop(cache); // Release read lock
        }

        // Calculate tax based on provider
        let result = match self.config.provider {
            TaxProvider::TaxJar => {
                self.calculate_tax_jar(subtotal, address, include_shipping, shipping_amount)
                    .await?
            }
            TaxProvider::Avalara => {
                self.calculate_avalara(subtotal, address, include_shipping, shipping_amount)
                    .await?
            }
            TaxProvider::Fallback => {
                self.calculate_fallback(subtotal, address, include_shipping, shipping_amount)?
            }
        };

        // Cache the result
        if self.config.enable_cache {
            let cache_key = self.generate_cache_key(subtotal, address, include_shipping, shipping_amount);
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, (result.clone(), std::time::Instant::now()));
        }

        Ok(result)
    }

    /// Calculate tax using TaxJar API
    #[instrument(skip(self))]
    async fn calculate_tax_jar(
        &self,
        subtotal: i64,
        address: &Address,
        include_shipping: bool,
        shipping_amount: i64,
    ) -> Result<TaxResult, ServiceError> {
        let api_key = self.config.api_key.as_ref().ok_or_else(|| {
            ServiceError::InternalError("TaxJar API key not configured".to_string())
        })?;

        let taxable_amount = if include_shipping {
            subtotal + shipping_amount
        } else {
            subtotal
        };

        // Convert cents to dollars
        let amount_dollars = taxable_amount as f64 / 100.0;
        let shipping_dollars = shipping_amount as f64 / 100.0;

        let request_body = serde_json::json!({
            "to_country": address.country,
            "to_zip": address.postal_code,
            "to_state": address.region.as_ref().unwrap_or(&"".to_string()),
            "to_city": address.city,
            "to_street": address.line1,
            "amount": amount_dollars,
            "shipping": shipping_dollars,
        });

        info!(amount = amount_dollars, "Calculating tax via TaxJar");

        let response = self
            .client
            .post("https://api.taxjar.com/v2/taxes")
            .bearer_auth(api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("TaxJar API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!(error = %error_text, "TaxJar API error");
            // Fall back to rule-based calculation
            return self.calculate_fallback(subtotal, address, include_shipping, shipping_amount);
        }

        let tax_response: TaxJarResponse = response.json().await.map_err(|e| {
            ServiceError::ParseError(format!("Failed to parse TaxJar response: {}", e))
        })?;

        let tax_amount = (tax_response.tax.amount_to_collect * 100.0).round() as i64;
        let tax_rate = tax_response.tax.rate;

        let mut breakdown = Vec::new();

        // Add state tax
        if let Some(state_rate) = tax_response.tax.breakdown.and_then(|b| b.state_taxable_amount) {
            breakdown.push(TaxBreakdown {
                jurisdiction: address.region.clone().unwrap_or_default(),
                rate: tax_response.tax.breakdown.and_then(|b| b.state_tax_rate).unwrap_or(0.0),
                amount: (state_rate * 100.0).round() as i64,
                tax_type: "state".to_string(),
            });
        }

        info!(tax_amount = tax_amount, rate = tax_rate, "Tax calculated via TaxJar");

        Ok(TaxResult {
            tax_amount,
            tax_rate,
            breakdown,
            cached: false,
        })
    }

    /// Calculate tax using Avalara AvaTax
    #[instrument(skip(self))]
    async fn calculate_avalara(
        &self,
        subtotal: i64,
        address: &Address,
        include_shipping: bool,
        shipping_amount: i64,
    ) -> Result<TaxResult, ServiceError> {
        let account_id = self.config.account_id.as_ref().ok_or_else(|| {
            ServiceError::InternalError("Avalara account ID not configured".to_string())
        })?;

        let api_key = self.config.api_key.as_ref().ok_or_else(|| {
            ServiceError::InternalError("Avalara API key not configured".to_string())
        })?;

        let company_code = self.config.company_code.as_ref()
            .ok_or_else(|| ServiceError::InternalError("Avalara company code not configured".to_string()))?;

        let taxable_amount = if include_shipping {
            subtotal + shipping_amount
        } else {
            subtotal
        };

        // Convert cents to dollars
        let amount_dollars = taxable_amount as f64 / 100.0;

        let request_body = serde_json::json!({
            "companyCode": company_code,
            "type": "SalesOrder",
            "customerCode": "GUEST",
            "date": chrono::Utc::now().format("%Y-%m-%d").to_string(),
            "addresses": {
                "shipTo": {
                    "line1": address.line1,
                    "city": address.city,
                    "region": address.region.as_ref().unwrap_or(&"".to_string()),
                    "country": address.country,
                    "postalCode": address.postal_code,
                }
            },
            "lines": [
                {
                    "number": "1",
                    "quantity": 1,
                    "amount": amount_dollars,
                    "taxCode": "P0000000", // Physical goods
                }
            ],
            "commit": false,
        });

        info!(amount = amount_dollars, "Calculating tax via Avalara");

        let response = self
            .client
            .post(format!("https://rest.avatax.com/api/v2/transactions/create"))
            .basic_auth(account_id, Some(api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Avalara API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!(error = %error_text, "Avalara API error");
            // Fall back to rule-based calculation
            return self.calculate_fallback(subtotal, address, include_shipping, shipping_amount);
        }

        let avalara_response: AvalaraResponse = response.json().await.map_err(|e| {
            ServiceError::ParseError(format!("Failed to parse Avalara response: {}", e))
        })?;

        let tax_amount = (avalara_response.total_tax * 100.0).round() as i64;
        let tax_rate = avalara_response.total_tax_calculated / (amount_dollars + 0.01); // Avoid div by zero

        // Build breakdown
        let breakdown = avalara_response
            .summary
            .into_iter()
            .map(|s| TaxBreakdown {
                jurisdiction: s.jurisdiction_name.unwrap_or_default(),
                rate: s.rate,
                amount: (s.tax * 100.0).round() as i64,
                tax_type: s.jurisdiction_type.unwrap_or_else(|| "unknown".to_string()),
            })
            .collect();

        info!(tax_amount = tax_amount, rate = tax_rate, "Tax calculated via Avalara");

        Ok(TaxResult {
            tax_amount,
            tax_rate,
            breakdown,
            cached: false,
        })
    }

    /// Fallback rule-based tax calculation
    fn calculate_fallback(
        &self,
        subtotal: i64,
        address: &Address,
        include_shipping: bool,
        shipping_amount: i64,
    ) -> Result<TaxResult, ServiceError> {
        let taxable_amount = if include_shipping {
            subtotal + shipping_amount
        } else {
            subtotal
        };

        // Simple jurisdiction-based rates (expand as needed)
        let (tax_rate, jurisdiction) = match (address.country.as_str(), address.region.as_deref()) {
            ("US", Some("CA")) => (0.0875, "California"),      // CA average
            ("US", Some("NY")) => (0.08875, "New York"),       // NYC rate
            ("US", Some("TX")) => (0.0625, "Texas"),
            ("US", Some("FL")) => (0.06, "Florida"),
            ("US", Some("WA")) => (0.0865, "Washington"),
            ("US", _) => (0.07, "United States"),              // US average
            ("CA", _) => (0.13, "Canada"),                     // HST/GST
            ("GB", _) => (0.20, "United Kingdom"),             // VAT
            ("DE", _) => (0.19, "Germany"),                    // VAT
            ("FR", _) => (0.20, "France"),                     // VAT
            _ => (0.0, "Unknown"),
        };

        let tax_amount = (taxable_amount as f64 * tax_rate).round() as i64;

        let breakdown = vec![TaxBreakdown {
            jurisdiction: jurisdiction.to_string(),
            rate: tax_rate,
            amount: tax_amount,
            tax_type: "combined".to_string(),
        }];

        Ok(TaxResult {
            tax_amount,
            tax_rate,
            breakdown,
            cached: false,
        })
    }

    fn generate_cache_key(&self, subtotal: i64, address: &Address, include_shipping: bool, shipping: i64) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            subtotal,
            address.country,
            address.region.as_ref().unwrap_or(&"".to_string()),
            address.postal_code,
            include_shipping,
            shipping
        )
    }
}

// API response types

#[derive(Debug, Deserialize)]
struct TaxJarResponse {
    tax: TaxJarTax,
}

#[derive(Debug, Deserialize)]
struct TaxJarTax {
    amount_to_collect: f64,
    rate: f64,
    breakdown: Option<TaxJarBreakdown>,
}

#[derive(Debug, Deserialize)]
struct TaxJarBreakdown {
    state_taxable_amount: Option<f64>,
    state_tax_rate: Option<f64>,
    county_taxable_amount: Option<f64>,
    county_tax_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AvalaraResponse {
    #[serde(rename = "totalTax")]
    total_tax: f64,
    #[serde(rename = "totalTaxCalculated")]
    total_tax_calculated: f64,
    summary: Vec<AvajaraSummary>,
}

#[derive(Debug, Deserialize)]
struct AvajaraSummary {
    #[serde(rename = "jurisName")]
    jurisdiction_name: Option<String>,
    #[serde(rename = "jurisType")]
    jurisdiction_type: Option<String>,
    rate: f64,
    tax: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fallback_tax_calculation() {
        let config = TaxServiceConfig {
            provider: TaxProvider::Fallback,
            api_key: None,
            account_id: None,
            company_code: None,
            enable_cache: false,
            cache_ttl_secs: 3600,
        };

        let service = TaxService::new(config);

        let address = Address {
            name: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "San Francisco".to_string(),
            region: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            email: None,
        };

        let result = service
            .calculate_tax(10000, &address, false, 0)
            .await
            .unwrap();

        assert!(result.tax_amount > 0);
        assert!(result.tax_rate > 0.08); // CA rate
        assert_eq!(result.breakdown.len(), 1);
    }

    #[tokio::test]
    async fn test_cache() {
        let config = TaxServiceConfig {
            provider: TaxProvider::Fallback,
            api_key: None,
            account_id: None,
            company_code: None,
            enable_cache: true,
            cache_ttl_secs: 3600,
        };

        let service = TaxService::new(config);

        let address = Address {
            name: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "New York".to_string(),
            region: Some("NY".to_string()),
            postal_code: "10001".to_string(),
            country: "US".to_string(),
            phone: None,
            email: None,
        };

        // First call - not cached
        let result1 = service.calculate_tax(10000, &address, false, 0).await.unwrap();
        assert!(!result1.cached);

        // Second call - should be cached
        let result2 = service.calculate_tax(10000, &address, false, 0).await.unwrap();
        assert!(result2.cached);
        assert_eq!(result1.tax_amount, result2.tax_amount);
    }
}
