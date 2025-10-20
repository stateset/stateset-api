use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::commerce::{
    product_image, product_variant, variant_image, Product, ProductModel,
};
use crate::entities::inventory_items;
use crate::entities::product as catalog_product;

const SHOPIFY_API_VERSION: &str = "2025-04";
const SHOPIFY_PAGE_LIMIT: usize = 250;

/// Credentials required to access the Shopify Admin API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShopifyCredentials {
    pub shop_domain: String,
    pub access_token: String,
}

impl ShopifyCredentials {
    /// Load credentials from the canonical environment variables used by the Node.js helper.
    pub fn from_env() -> Result<Self> {
        let shop_domain = std::env::var("SHOPIFY_STORE").context(
            "SHOPIFY_STORE environment variable is required for Shopify product feed generation",
        )?;
        let access_token = std::env::var("SHOPIFY_ACCESS_TOKEN")
            .context("SHOPIFY_ACCESS_TOKEN environment variable is required for Shopify product feed generation")?;

        if shop_domain.trim().is_empty() {
            bail!("SHOPIFY_STORE may not be empty");
        }

        if access_token.trim().is_empty() {
            bail!("SHOPIFY_ACCESS_TOKEN may not be empty");
        }

        Ok(Self {
            shop_domain,
            access_token,
        })
    }
}

/// High level service that mirrors the behaviour exposed in `product_feed.js`.
pub struct ProductFeedService {
    client: Client,
    credentials: ShopifyCredentials,
}

impl ProductFeedService {
    /// Build a service using a default reqwest client with sensible timeouts.
    pub fn new(credentials: ShopifyCredentials) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to construct reqwest client for product feed service")?;

        Ok(Self::with_client(credentials, client))
    }

    /// Build a service from an existing client (useful for testing).
    pub fn with_client(credentials: ShopifyCredentials, client: Client) -> Self {
        Self {
            client,
            credentials,
        }
    }

    /// Generate an Agentic Commerce product feed and optionally push to OpenAI.
    pub async fn create_agentic_commerce_feed(
        &self,
        request: ProductFeedRequest,
    ) -> Result<GeneratedFeed> {
        let normalized = request
            .options
            .normalize(Some(&self.credentials.shop_domain))?;

        if normalized.enable_checkout
            && (normalized.seller_privacy_policy.is_none()
                || normalized.seller_tos.is_none()
                || normalized.return_policy.is_none())
        {
            bail!(
                "When enable_checkout is true, seller_privacy_policy, seller_tos, and return_policy must be provided"
            );
        }

        let products = self.fetch_shopify_products(SHOPIFY_PAGE_LIMIT).await?;
        let mut feed_items = Vec::new();

        for product in &products {
            for variant in &product.variants {
                let item = transform_shopify_variant(product, variant, &normalized);
                feed_items.push(item);

                if let Some(max) = request.max_products {
                    if feed_items.len() >= max {
                        break;
                    }
                }
            }

            if let Some(max) = request.max_products {
                if feed_items.len() >= max {
                    break;
                }
            }
        }

        let generated_at = Utc::now();
        let product_count = feed_items.len();

        let csv_payload = if request.format == ProductFeedFormat::Csv {
            Some(build_delimited_payload(&feed_items, ',')?)
        } else {
            None
        };

        let tsv_payload = if request.format == ProductFeedFormat::Tsv {
            Some(build_delimited_payload(&feed_items, '\t')?)
        } else {
            None
        };

        let payload = match request.format {
            ProductFeedFormat::Json => FeedPayload::Json(ProductFeedEnvelope {
                feed_metadata: FeedMetadata {
                    generated_at,
                    product_count,
                    merchant: self.credentials.shop_domain.clone(),
                    format: "agentic_commerce_v1".to_string(),
                    enable_checkout: normalized.enable_checkout,
                },
                products: feed_items,
            }),
            ProductFeedFormat::Csv => {
                let (body, content_type) = csv_payload.expect("computed for CSV format");
                FeedPayload::Text { body, content_type }
            }
            ProductFeedFormat::Tsv => {
                let (body, content_type) = tsv_payload.expect("computed for TSV format");
                FeedPayload::Text { body, content_type }
            }
        };

        let openai_response = if request.push_to_openai {
            let endpoint = request
                .openai_endpoint
                .as_deref()
                .context("openai_endpoint must be provided when push_to_openai is true")?;
            let api_key = request
                .openai_api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_FEED_API_KEY").ok())
                .ok_or_else(|| {
                    anyhow!(
                        "openai_api_key must be provided when push_to_openai is true and not available in OPENAI_FEED_API_KEY"
                    )
                })?;
            Some(
                push_feed_to_openai_with_client(&self.client, endpoint, &api_key, &payload)
                    .await
                    .context("failed to push feed to OpenAI")?,
            )
        } else {
            None
        };

        Ok(GeneratedFeed {
            payload,
            metadata: FeedGenerationMetadata {
                format: request.format,
                product_count,
                generated_at,
                merchant: self.credentials.shop_domain.clone(),
                enable_checkout: normalized.enable_checkout,
                pushed_to_openai: openai_response.is_some(),
            },
            openai_response,
        })
    }

    async fn fetch_shopify_products(&self, limit: usize) -> Result<Vec<ShopifyProduct>> {
        let mut products = Vec::new();
        let mut next_url = format!(
            "https://{}/admin/api/{}/products.json?limit={}",
            self.credentials.shop_domain, SHOPIFY_API_VERSION, limit
        );

        let headers = self.build_shopify_headers()?;

        while !next_url.is_empty() {
            let response = self
                .client
                .get(&next_url)
                .headers(headers.clone())
                .send()
                .await
                .with_context(|| format!("failed to fetch Shopify products from {}", next_url))?;

            let status = response.status();
            let link_header = response
                .headers()
                .get("link")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response
                .bytes()
                .await
                .context("failed to read Shopify products response body")?;

            if !status.is_success() {
                let text = String::from_utf8_lossy(&body);
                bail!("Shopify API error (status: {}): {}", status, text);
            }

            let mut page: ShopifyProductsResponse = serde_json::from_slice(&body)
                .context("failed to deserialize Shopify product response")?;

            products.append(&mut page.products);

            if let Some(link_header) = link_header {
                next_url = parse_next_link(&link_header).unwrap_or_default();
            } else {
                next_url.clear();
            }
        }

        Ok(products)
    }

    fn build_shopify_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "X-Shopify-Access-Token",
            HeaderValue::from_str(&self.credentials.access_token)
                .context("invalid characters in Shopify access token")?,
        );
        Ok(headers)
    }
}

/// Feed service that generates Agentic Commerce product feeds from the internal catalog.
pub struct CatalogProductFeedService {
    db: Arc<DatabaseConnection>,
    client: Client,
}

impl CatalogProductFeedService {
    pub fn new(db: Arc<DatabaseConnection>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to construct reqwest client for catalog product feed service")?;

        Ok(Self::with_client(db, client))
    }

    pub fn with_client(db: Arc<DatabaseConnection>, client: Client) -> Self {
        Self { db, client }
    }

    /// Build the Agentic Commerce feed from the Stateset product catalog.
    pub async fn create_agentic_commerce_feed(
        &self,
        request: ProductFeedRequest,
    ) -> Result<GeneratedFeed> {
        let normalized = request.options.clone().normalize(None)?;

        let products = Product::find()
            .filter(catalog_product::Column::IsActive.eq(true))
            .order_by_desc(catalog_product::Column::CreatedAt)
            .all(&*self.db)
            .await?;

        if products.is_empty() {
            return self
                .build_response(Vec::new(), &normalized, request, Utc::now())
                .await;
        }

        let product_ids: Vec<Uuid> = products.iter().map(|p| p.id).collect();

        let mut variants_by_product: HashMap<Uuid, Vec<product_variant::Model>> = HashMap::new();
        let mut variant_ids = Vec::new();
        let mut skus = Vec::new();

        if !product_ids.is_empty() {
            let variants = product_variant::Entity::find()
                .filter(product_variant::Column::ProductId.is_in(product_ids.clone()))
                .order_by_asc(product_variant::Column::Position)
                .all(&*self.db)
                .await?;

            for variant in variants {
                skus.push(variant.sku.clone());
                variant_ids.push(variant.id);
                variants_by_product
                    .entry(variant.product_id)
                    .or_default()
                    .push(variant);
            }
        }

        let mut product_images_by_product: HashMap<Uuid, Vec<product_image::Model>> =
            HashMap::new();
        if !product_ids.is_empty() {
            let product_images = product_image::Entity::find()
                .filter(product_image::Column::ProductId.is_in(product_ids.clone()))
                .order_by_asc(product_image::Column::SortOrder)
                .all(&*self.db)
                .await?;

            for image in product_images {
                product_images_by_product
                    .entry(image.product_id)
                    .or_default()
                    .push(image);
            }
        }

        let mut variant_images_by_variant: HashMap<Uuid, Vec<variant_image::Model>> =
            HashMap::new();
        if !variant_ids.is_empty() {
            let variant_images = variant_image::Entity::find()
                .filter(variant_image::Column::VariantId.is_in(variant_ids.clone()))
                .order_by_asc(variant_image::Column::SortOrder)
                .all(&*self.db)
                .await?;

            for image in variant_images {
                variant_images_by_variant
                    .entry(image.variant_id)
                    .or_default()
                    .push(image);
            }
        }

        let mut inventory_by_sku: HashMap<String, inventory_items::Model> = HashMap::new();
        if !skus.is_empty() {
            let inventory_items = inventory_items::Entity::find()
                .filter(inventory_items::Column::Sku.is_in(skus.clone()))
                .all(&*self.db)
                .await?;

            for item in inventory_items {
                inventory_by_sku.insert(item.sku.clone(), item);
            }
        }

        let mut feed_items = Vec::new();
        'outer: for product in products.iter() {
            if let Some(variants) = variants_by_product.get(&product.id) {
                for variant in variants {
                    let product_images = product_images_by_product
                        .get(&product.id)
                        .map(|imgs| imgs.as_slice())
                        .unwrap_or(&[]);
                    let variant_images = variant_images_by_variant
                        .get(&variant.id)
                        .map(|imgs| imgs.as_slice())
                        .unwrap_or(&[]);
                    let inventory_entry = inventory_by_sku.get(&variant.sku);

                    let item = transform_catalog_variant(
                        product,
                        variant,
                        product_images,
                        variant_images,
                        inventory_entry,
                        variants.len(),
                        &normalized,
                    );

                    feed_items.push(item);

                    if let Some(max) = request.max_products {
                        if feed_items.len() >= max {
                            break 'outer;
                        }
                    }
                }
            }
        }

        let generated_at = Utc::now();
        self.build_response(feed_items, &normalized, request, generated_at)
            .await
    }

    async fn build_response(
        &self,
        feed_items: Vec<ProductFeedItem>,
        options: &NormalizedFeedOptions,
        request: ProductFeedRequest,
        generated_at: DateTime<Utc>,
    ) -> Result<GeneratedFeed> {
        let product_count = feed_items.len();

        let csv_payload = if request.format == ProductFeedFormat::Csv {
            Some(build_delimited_payload(&feed_items, ',')?)
        } else {
            None
        };

        let tsv_payload = if request.format == ProductFeedFormat::Tsv {
            Some(build_delimited_payload(&feed_items, '\t')?)
        } else {
            None
        };

        let payload = match request.format {
            ProductFeedFormat::Json => FeedPayload::Json(ProductFeedEnvelope {
                feed_metadata: FeedMetadata {
                    generated_at,
                    product_count,
                    merchant: options.seller_name.clone(),
                    format: "agentic_commerce_v1".to_string(),
                    enable_checkout: options.enable_checkout,
                },
                products: feed_items,
            }),
            ProductFeedFormat::Csv => {
                let (body, content_type) = csv_payload.expect("computed for CSV format");
                FeedPayload::Text { body, content_type }
            }
            ProductFeedFormat::Tsv => {
                let (body, content_type) = tsv_payload.expect("computed for TSV format");
                FeedPayload::Text { body, content_type }
            }
        };

        let openai_response = if request.push_to_openai {
            let endpoint = request
                .openai_endpoint
                .as_deref()
                .context("openai_endpoint must be provided when push_to_openai is true")?;
            let api_key = request
                .openai_api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_FEED_API_KEY").ok())
                .ok_or_else(|| {
                    anyhow!(
                        "openai_api_key must be provided when push_to_openai is true and not available in OPENAI_FEED_API_KEY"
                    )
                })?;

            Some(
                push_feed_to_openai_with_client(&self.client, endpoint, &api_key, &payload)
                    .await
                    .context("failed to push feed to OpenAI")?,
            )
        } else {
            None
        };

        Ok(GeneratedFeed {
            payload,
            metadata: FeedGenerationMetadata {
                format: request.format,
                product_count,
                generated_at,
                merchant: options.seller_name.clone(),
                enable_checkout: options.enable_checkout,
                pushed_to_openai: openai_response.is_some(),
            },
            openai_response,
        })
    }
}

/// Request parameters accepted by the service.
#[derive(Clone, Debug)]
pub struct ProductFeedRequest {
    pub format: ProductFeedFormat,
    pub options: ProductFeedOptions,
    pub max_products: Option<usize>,
    pub push_to_openai: bool,
    pub openai_endpoint: Option<String>,
    pub openai_api_key: Option<String>,
}

impl Default for ProductFeedRequest {
    fn default() -> Self {
        Self {
            format: ProductFeedFormat::Json,
            options: ProductFeedOptions::default(),
            max_products: None,
            push_to_openai: false,
            openai_endpoint: None,
            openai_api_key: None,
        }
    }
}

/// Request level flags that mirror the Express handler options.
#[derive(Clone, Debug, Default)]
pub struct ProductFeedOptions {
    pub enable_search: Option<bool>,
    pub enable_checkout: Option<bool>,
    pub currency: Option<String>,
    pub default_shipping_cost: Option<String>,
    pub seller_name: Option<String>,
    pub seller_url: Option<String>,
    pub product_base_url: Option<String>,
    pub seller_privacy_policy: Option<String>,
    pub seller_tos: Option<String>,
    pub return_policy: Option<String>,
    pub return_window: Option<u32>,
}

impl ProductFeedOptions {
    fn normalize(self, domain_hint: Option<&str>) -> Result<NormalizedFeedOptions> {
        let enable_search = self.enable_search.unwrap_or(true);
        let enable_checkout = self.enable_checkout.unwrap_or(true);
        let currency = self.currency.unwrap_or_else(|| "USD".to_string());
        let default_shipping_cost = self
            .default_shipping_cost
            .unwrap_or_else(|| "0.00".to_string());

        let domain_hint = domain_hint.unwrap_or("merchant.local");
        let canonical_domain = canonicalize_domain(domain_hint);

        let seller_url = self
            .seller_url
            .clone()
            .unwrap_or_else(|| format!("https://{}", canonical_domain));
        let seller_name = self
            .seller_name
            .unwrap_or_else(|| derive_default_seller_name(&canonical_domain));

        let product_base_url = self
            .product_base_url
            .clone()
            .or_else(|| Some(format!("{}/products", seller_url.trim_end_matches('/'))));

        Ok(NormalizedFeedOptions {
            enable_search,
            enable_checkout,
            currency,
            default_shipping_cost,
            seller_name,
            seller_url,
            product_base_url,
            seller_privacy_policy: self.seller_privacy_policy,
            seller_tos: self.seller_tos,
            return_policy: self.return_policy,
            return_window: self.return_window.unwrap_or(30),
        })
    }
}

struct NormalizedFeedOptions {
    enable_search: bool,
    enable_checkout: bool,
    currency: String,
    default_shipping_cost: String,
    seller_name: String,
    seller_url: String,
    product_base_url: Option<String>,
    seller_privacy_policy: Option<String>,
    seller_tos: Option<String>,
    return_policy: Option<String>,
    return_window: u32,
}

/// Supported output encodings for the OpenAI Agentic Commerce feed.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProductFeedFormat {
    Json,
    Csv,
    Tsv,
}

impl Default for ProductFeedFormat {
    fn default() -> Self {
        ProductFeedFormat::Json
    }
}

impl fmt::Display for ProductFeedFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProductFeedFormat::Json => write!(f, "json"),
            ProductFeedFormat::Csv => write!(f, "csv"),
            ProductFeedFormat::Tsv => write!(f, "tsv"),
        }
    }
}

impl FromStr for ProductFeedFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ProductFeedFormat::Json),
            "csv" => Ok(ProductFeedFormat::Csv),
            "tsv" => Ok(ProductFeedFormat::Tsv),
            other => Err(anyhow!("unsupported product feed format: {}", other)),
        }
    }
}

/// Envelope returned when JSON output is requested.
#[derive(Debug, Serialize)]
pub struct ProductFeedEnvelope {
    pub feed_metadata: FeedMetadata,
    pub products: Vec<ProductFeedItem>,
}

/// Metadata describing the generated feed file.
#[derive(Debug, Serialize)]
pub struct FeedMetadata {
    pub generated_at: DateTime<Utc>,
    pub product_count: usize,
    pub merchant: String,
    pub format: String,
    pub enable_checkout: bool,
}

/// Representation of a single product variant entry in the Agentic Commerce feed.
#[derive(Debug, Serialize)]
pub struct ProductFeedItem {
    pub enable_search: bool,
    pub enable_checkout: bool,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gtin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mpn: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub link: String,
    pub condition: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_image_link: Option<String>,
    pub price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_price: Option<String>,
    pub availability: String,
    pub inventory_quantity: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_group_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_variant1_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_variant1_option: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<String>,
    pub seller_name: String,
    pub seller_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_privacy_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_tos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_review_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_review_rating: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopify_product_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopify_variant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Final output structure returned by the service.
pub struct GeneratedFeed {
    pub payload: FeedPayload,
    pub metadata: FeedGenerationMetadata,
    pub openai_response: Option<Value>,
}

/// Feed metadata that mirrors the Express handler response.
#[derive(Debug, Serialize)]
pub struct FeedGenerationMetadata {
    pub format: ProductFeedFormat,
    pub product_count: usize,
    pub generated_at: DateTime<Utc>,
    pub merchant: String,
    pub enable_checkout: bool,
    pub pushed_to_openai: bool,
}

/// Serialized payload returned to callers (JSON or text).
pub enum FeedPayload {
    Json(ProductFeedEnvelope),
    Text {
        body: String,
        content_type: &'static str,
    },
}

impl FeedPayload {
    pub fn content_type(&self) -> &'static str {
        match self {
            FeedPayload::Json(_) => "application/json",
            FeedPayload::Text { content_type, .. } => content_type,
        }
    }

    pub fn as_json(&self) -> Option<&ProductFeedEnvelope> {
        if let FeedPayload::Json(envelope) = self {
            Some(envelope)
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct ShopifyProductsResponse {
    products: Vec<ShopifyProduct>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ShopifyProduct {
    id: Option<i64>,
    title: Option<String>,
    handle: Option<String>,
    body_html: Option<String>,
    product_type: Option<String>,
    vendor: Option<String>,
    tags: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    #[serde(default)]
    variants: Vec<ShopifyVariant>,
    #[serde(default)]
    options: Vec<ShopifyOption>,
    #[serde(default)]
    images: Vec<ShopifyImage>,
    image: Option<ShopifyImage>,
}

#[derive(Debug, Deserialize)]
struct ShopifyVariant {
    id: Option<i64>,
    title: Option<String>,
    price: Option<String>,
    compare_at_price: Option<String>,
    weight: Option<f64>,
    weight_unit: Option<String>,
    sku: Option<String>,
    barcode: Option<String>,
    inventory_quantity: Option<i64>,
    option1: Option<String>,
    option2: Option<String>,
    option3: Option<String>,
    requires_shipping: Option<bool>,
    image_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ShopifyOption {
    name: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct ShopifyImage {
    id: Option<i64>,
    src: Option<String>,
}

fn transform_shopify_variant(
    product: &ShopifyProduct,
    variant: &ShopifyVariant,
    options: &NormalizedFeedOptions,
) -> ProductFeedItem {
    let variant_id = variant
        .id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown-variant".to_string());
    let product_id = product
        .id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown-product".to_string());

    let handle = product
        .handle
        .clone()
        .unwrap_or_else(|| "product".to_string());

    let (product_url, variant_url) = build_product_links(&handle, &variant_id, options);

    let availability = match variant.inventory_quantity.unwrap_or(0) {
        qty if qty > 0 => "in_stock".to_string(),
        _ => "out_of_stock".to_string(),
    };

    let options_lower: Vec<String> = product
        .options
        .iter()
        .filter_map(|option| option.name.as_ref().map(|name| name.to_lowercase()))
        .collect();

    let color = if options_lower.get(0).map(|s| s == "color").unwrap_or(false) {
        variant.option1.clone()
    } else {
        None
    };

    let size = if options_lower.get(1).map(|s| s == "size").unwrap_or(false) {
        variant.option2.clone()
    } else {
        None
    };

    let custom_variant_category = product.options.get(2).and_then(|opt| opt.name.clone());

    let custom_variant_option = variant.option3.clone();

    let image_link = variant
        .image_id
        .and_then(|image_id| {
            product
                .images
                .iter()
                .find(|img| img.id == Some(image_id))
                .and_then(|img| img.src.clone())
        })
        .or_else(|| product.image.as_ref().and_then(|img| img.src.clone()))
        .or_else(|| product.images.get(0).and_then(|img| img.src.clone()));

    let additional_image_link = if product.images.len() > 1 {
        let links: Vec<String> = product
            .images
            .iter()
            .skip(1)
            .take(4)
            .filter_map(|img| img.src.clone())
            .collect();
        if links.is_empty() {
            None
        } else {
            Some(links.join(","))
        }
    } else {
        None
    };

    let title = match variant.title.as_deref() {
        Some("Default Title") | None => product
            .title
            .clone()
            .unwrap_or_else(|| "Untitled Product".to_string()),
        Some(variant_title) => format!(
            "{} - {}",
            product
                .title
                .clone()
                .unwrap_or_else(|| "Untitled Product".to_string()),
            variant_title
        ),
    };

    let description = product
        .body_html
        .as_ref()
        .map(|body| strip_html(body).chars().take(5000).collect::<String>());

    let item_group_id = if product.variants.len() > 1 {
        Some(product_id.clone())
    } else {
        None
    };

    let item_group_title = if product.variants.len() > 1 {
        product.title.clone()
    } else {
        None
    };

    let weight = match (variant.weight, variant.weight_unit.as_ref()) {
        (Some(weight), Some(unit)) if weight > 0.0 => Some(format!("{weight} {unit}")),
        _ => None,
    };

    ProductFeedItem {
        enable_search: options.enable_search,
        enable_checkout: options.enable_checkout,
        id: variant_id.clone(),
        gtin: variant.barcode.clone(),
        mpn: variant.sku.clone(),
        title,
        description,
        link: if product.variants.len() > 1 {
            variant_url
        } else {
            product_url
        },
        condition: "new",
        product_category: product.product_type.clone(),
        brand: product.vendor.clone(),
        material: None,
        weight,
        image_link,
        additional_image_link,
        price: format!(
            "{} {}",
            variant.price.clone().unwrap_or_else(|| "0.00".to_string()),
            options.currency
        ),
        sale_price: variant.compare_at_price.as_ref().map(|_| {
            format!(
                "{} {}",
                variant.price.clone().unwrap_or_else(|| "0.00".to_string()),
                options.currency
            )
        }),
        availability,
        inventory_quantity: variant.inventory_quantity.unwrap_or(0),
        item_group_id,
        item_group_title,
        color,
        size,
        offer_id: Some(format!("{}-{}", handle, variant_id)),
        custom_variant1_category: custom_variant_category,
        custom_variant1_option: custom_variant_option,
        shipping: match variant.requires_shipping.unwrap_or(false) {
            true => Some(format!(
                "US::Standard:{} {}",
                options.default_shipping_cost, options.currency
            )),
            false => None,
        },
        seller_name: options.seller_name.clone(),
        seller_url: options.seller_url.clone(),
        seller_privacy_policy: options.seller_privacy_policy.clone(),
        seller_tos: options.seller_tos.clone(),
        return_policy: options.return_policy.clone(),
        return_window: Some(options.return_window),
        popularity_score: None,
        return_rate: None,
        product_review_count: None,
        product_review_rating: None,
        shopify_product_id: Some(product_id),
        shopify_variant_id: Some(variant_id),
        product_handle: Some(handle),
        tags: product.tags.clone(),
        created_at: product.created_at.clone(),
        updated_at: product.updated_at.clone(),
    }
}

fn transform_catalog_variant(
    product: &ProductModel,
    variant: &product_variant::Model,
    product_images: &[product_image::Model],
    variant_images: &[variant_image::Model],
    inventory_entry: Option<&inventory_items::Model>,
    variants_in_group: usize,
    options: &NormalizedFeedOptions,
) -> ProductFeedItem {
    let variant_id = variant.id.to_string();
    let product_id = product.id.to_string();
    let handle = generate_product_handle(&product.name, &product.id);
    let (product_url, variant_url) = build_product_links(&handle, &variant_id, options);

    let (availability, inventory_quantity) = if !variant.inventory_tracking {
        ("in_stock".to_string(), 9999)
    } else if let Some(entry) = inventory_entry {
        if entry.available > 0 {
            ("in_stock".to_string(), i64::from(entry.available))
        } else {
            ("out_of_stock".to_string(), 0)
        }
    } else {
        ("out_of_stock".to_string(), 0)
    };

    let (color, size, custom_category, custom_option) = extract_option_attributes(&variant.options);

    let (image_link, additional_image_link) =
        resolve_catalog_images(variant_images, product_images);

    let title =
        if variant.name.trim().eq_ignore_ascii_case("default") || variant.name.trim().is_empty() {
            product.name.clone()
        } else if variant.name.trim() == product.name.trim() {
            variant.name.clone()
        } else {
            format!("{} - {}", product.name, variant.name)
        };

    let description = product
        .description
        .as_ref()
        .map(|desc| {
            let text = strip_html(desc);
            let truncated = text.as_ref().chars().take(5000).collect::<String>();
            if truncated.is_empty() {
                None
            } else {
                Some(truncated)
            }
        })
        .flatten()
        .or_else(|| Some(product.name.clone()));

    let (price_string, sale_price) = match variant.compare_at_price.as_ref() {
        Some(compare) if compare > &variant.price => (
            format_decimal_money(compare, &options.currency),
            Some(format_decimal_money(&variant.price, &options.currency)),
        ),
        _ => (
            format_decimal_money(&variant.price, &options.currency),
            None,
        ),
    };

    let item_group_id = if variants_in_group > 1 {
        Some(product_id.clone())
    } else {
        None
    };
    let item_group_title = if variants_in_group > 1 {
        Some(product.name.clone())
    } else {
        None
    };

    let weight = variant
        .weight
        .filter(|w| *w > 0.0)
        .map(|w| format!("{:.2} kg", w))
        .or_else(|| {
            product
                .weight_kg
                .as_ref()
                .map(|w| format!("{} kg", decimal_to_string(w)))
        });

    let tags = extract_product_tags(product);
    let brand = extract_brand(product);

    let shipping = if product.is_digital {
        None
    } else {
        Some(format!(
            "US::Standard:{} {}",
            options.default_shipping_cost, options.currency
        ))
    };

    let offer_id = if !variant.sku.is_empty() {
        format!("{}-{}", handle, variant.sku)
    } else {
        format!("{}-{}", handle, variant_id)
    };

    ProductFeedItem {
        enable_search: options.enable_search,
        enable_checkout: options.enable_checkout,
        id: variant_id.clone(),
        gtin: None,
        mpn: if variant.sku.is_empty() {
            None
        } else {
            Some(variant.sku.clone())
        },
        title,
        description,
        link: if variants_in_group > 1 {
            variant_url
        } else {
            product_url
        },
        condition: "new",
        product_category: None,
        brand,
        material: None,
        weight,
        image_link,
        additional_image_link,
        price: price_string,
        sale_price,
        availability,
        inventory_quantity,
        item_group_id,
        item_group_title,
        color,
        size,
        offer_id: Some(offer_id),
        custom_variant1_category: custom_category,
        custom_variant1_option: custom_option,
        shipping,
        seller_name: options.seller_name.clone(),
        seller_url: options.seller_url.clone(),
        seller_privacy_policy: options.seller_privacy_policy.clone(),
        seller_tos: options.seller_tos.clone(),
        return_policy: options.return_policy.clone(),
        return_window: Some(options.return_window),
        popularity_score: None,
        return_rate: None,
        product_review_count: None,
        product_review_rating: None,
        shopify_product_id: Some(product_id),
        shopify_variant_id: Some(variant_id),
        product_handle: Some(handle),
        tags,
        created_at: Some(product.created_at.to_rfc3339()),
        updated_at: product.updated_at.map(|dt| dt.to_rfc3339()),
    }
}

fn format_decimal_money(value: &Decimal, currency: &str) -> String {
    format!("{} {}", decimal_to_string(value), currency)
}

fn decimal_to_string(value: &Decimal) -> String {
    let mut s = value.round_dp(2).to_string();
    if let Some(dot) = s.find('.') {
        let decimals = s.len() - dot - 1;
        if decimals == 0 {
            s.push_str("00");
        } else if decimals == 1 {
            s.push('0');
        }
    } else {
        s.push_str(".00");
    }
    s
}

fn generate_product_handle(name: &str, fallback: &Uuid) -> String {
    let mut handle = String::new();
    let mut prev_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            handle.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !prev_dash && !handle.is_empty() {
                handle.push('-');
                prev_dash = true;
            }
        }
    }

    let trimmed = handle.trim_matches('-').to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed
    }
}

fn extract_option_attributes(
    options: &serde_json::Value,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut color = None;
    let mut size = None;
    let mut custom_category = None;
    let mut custom_option = None;

    if let Ok(map) = serde_json::from_value::<HashMap<String, String>>(options.clone()) {
        for (key, value) in map {
            match key.to_lowercase().as_str() {
                "color" if color.is_none() => color = Some(value),
                "size" if size.is_none() => size = Some(value),
                _ => {
                    if custom_category.is_none() {
                        custom_category = Some(key);
                        custom_option = Some(value);
                    }
                }
            }
        }
    }

    (color, size, custom_category, custom_option)
}

fn resolve_catalog_images(
    variant_images: &[variant_image::Model],
    product_images: &[product_image::Model],
) -> (Option<String>, Option<String>) {
    let mut primary = variant_images
        .iter()
        .find(|img| img.is_primary)
        .map(|img| img.url.clone())
        .or_else(|| variant_images.first().map(|img| img.url.clone()))
        .or_else(|| {
            product_images
                .iter()
                .find(|img| img.is_primary)
                .map(|img| img.url.clone())
        })
        .or_else(|| product_images.first().map(|img| img.url.clone()));

    let mut seen = HashSet::new();
    if let Some(ref url) = primary {
        seen.insert(url.clone());
    }

    let mut extras = Vec::new();
    for url in variant_images
        .iter()
        .map(|img| img.url.clone())
        .chain(product_images.iter().map(|img| img.url.clone()))
    {
        if seen.contains(&url) {
            continue;
        }
        seen.insert(url.clone());
        extras.push(url);
    }

    let additional = if extras.is_empty() {
        None
    } else {
        Some(extras.into_iter().take(4).collect::<Vec<_>>().join(","))
    };

    (primary.take(), additional)
}

fn extract_product_tags(product: &ProductModel) -> Option<String> {
    product.tags.clone()
}

fn extract_brand(product: &ProductModel) -> Option<String> {
    product.brand.clone()
}

fn build_delimited_payload(
    items: &[ProductFeedItem],
    delimiter: char,
) -> Result<(String, &'static str)> {
    if items.is_empty() {
        let content_type = match delimiter {
            ',' => "text/csv",
            '\t' => "text/tab-separated-values",
            _ => "text/plain",
        };
        return Ok(("".to_string(), content_type));
    }

    let mut headers = BTreeSet::new();
    for item in items {
        let value = serde_json::to_value(item)?;
        if let Value::Object(map) = value {
            headers.extend(map.keys().cloned());
        }
    }

    let headers: Vec<String> = headers.into_iter().collect();

    let mut lines = Vec::with_capacity(items.len() + 1);
    lines.push(headers.join(&delimiter.to_string()));

    for item in items {
        let value = serde_json::to_value(item)?;
        let mut row = Vec::with_capacity(headers.len());

        for header in &headers {
            let field = value.get(header).cloned().unwrap_or(Value::Null);
            row.push(escape_field(&value_to_string(&field), delimiter));
        }

        lines.push(row.join(&delimiter.to_string()));
    }

    let body = lines.join("\n");
    let content_type = match delimiter {
        ',' => "text/csv",
        '\t' => "text/tab-separated-values",
        _ => "text/plain",
    };

    Ok((body, content_type))
}

fn escape_field(value: &str, delimiter: char) -> String {
    if value.contains(delimiter) || value.contains('"') || value.contains('\n') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn parse_next_link(link_header: &str) -> Option<String> {
    link_header.split(',').find_map(|segment| {
        let segment = segment.trim();
        if segment.ends_with("rel=\"next\"") {
            segment
                .split(';')
                .next()
                .and_then(|part| part.trim().strip_prefix('<'))
                .and_then(|part| part.strip_suffix('>'))
                .map(str::to_string)
        } else {
            None
        }
    })
}

fn build_product_links(
    handle: &str,
    variant_id: &str,
    options: &NormalizedFeedOptions,
) -> (String, String) {
    let base = options
        .product_base_url
        .as_ref()
        .map(|base| base.trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("{}/products", options.seller_url.trim_end_matches('/')));
    let product_url = format!("{}/{}", base, handle);
    let variant_url = format!("{}?variant={}", product_url, variant_id);
    (product_url, variant_url)
}

async fn push_feed_to_openai_with_client(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    payload: &FeedPayload,
) -> Result<Value> {
    let mut request = client.post(endpoint).bearer_auth(api_key);

    match payload {
        FeedPayload::Json(envelope) => {
            request = request.json(envelope);
        }
        FeedPayload::Text { body, content_type } => {
            request = request
                .header(CONTENT_TYPE, *content_type)
                .body(body.clone());
        }
    }

    let response = request
        .send()
        .await
        .context("failed to call OpenAI feed endpoint")?;

    let status = response.status();
    let body = response
        .bytes()
        .await
        .context("failed to read OpenAI response body")?;

    if !status.is_success() {
        let text = String::from_utf8_lossy(&body);
        bail!("OpenAI API error (status: {}): {}", status, text);
    }

    let json = serde_json::from_slice(&body).context("failed to parse OpenAI response as JSON")?;

    Ok(json)
}

fn canonicalize_domain(input: &str) -> String {
    let without_scheme = input
        .split("://")
        .nth(1)
        .unwrap_or(input)
        .trim_end_matches('/');
    without_scheme.to_string()
}

fn derive_default_seller_name(domain_hint: &str) -> String {
    let base = canonicalize_domain(domain_hint);
    let primary = base
        .split('.')
        .next()
        .unwrap_or(base.as_str())
        .replace('-', " ")
        .replace('_', " ");
    if primary.is_empty() {
        "merchant".to_string()
    } else {
        primary
    }
}

static HTML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new("<[^>]*>").unwrap());

fn strip_html(input: &str) -> Cow<'_, str> {
    if HTML_TAG_RE.is_match(input) {
        Cow::Owned(HTML_TAG_RE.replace_all(input, "").to_string())
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_product() -> ShopifyProduct {
        ShopifyProduct {
            id: Some(123),
            title: Some("Test Product".to_string()),
            handle: Some("test-product".to_string()),
            body_html: Some("<p>Great product</p>".to_string()),
            product_type: Some("General".to_string()),
            vendor: Some("Stateset".to_string()),
            tags: Some("tag1,tag2".to_string()),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            variants: vec![ShopifyVariant {
                id: Some(456),
                title: Some("Default Title".to_string()),
                price: Some("19.99".to_string()),
                compare_at_price: Some("29.99".to_string()),
                weight: Some(1.5),
                weight_unit: Some("lb".to_string()),
                sku: Some("SKU123".to_string()),
                barcode: Some("0123456789".to_string()),
                inventory_quantity: Some(10),
                option1: Some("Blue".to_string()),
                option2: Some("Large".to_string()),
                option3: Some("Gift Wrap".to_string()),
                requires_shipping: Some(true),
                image_id: Some(1),
            }],
            options: vec![
                ShopifyOption {
                    name: Some("Color".to_string()),
                },
                ShopifyOption {
                    name: Some("Size".to_string()),
                },
                ShopifyOption {
                    name: Some("Extras".to_string()),
                },
            ],
            images: vec![
                ShopifyImage {
                    id: Some(1),
                    src: Some("https://example.com/primary.jpg".to_string()),
                },
                ShopifyImage {
                    id: Some(2),
                    src: Some("https://example.com/secondary.jpg".to_string()),
                },
            ],
            image: Some(ShopifyImage {
                id: Some(1),
                src: Some("https://example.com/primary.jpg".to_string()),
            }),
        }
    }

    #[test]
    fn normalize_options_enforces_defaults() {
        let options = ProductFeedOptions::default();
        let normalized = options.normalize(Some("mystore.myshopify.com")).unwrap();

        assert!(normalized.enable_search);
        assert!(normalized.enable_checkout);
        assert_eq!(normalized.currency, "USD");
        assert_eq!(normalized.default_shipping_cost, "0.00");
        assert_eq!(normalized.seller_name, "mystore");
        assert_eq!(normalized.return_window, 30);
        assert_eq!(normalized.seller_url, "https://mystore.myshopify.com");
        assert_eq!(
            normalized.product_base_url,
            Some("https://mystore.myshopify.com/products".to_string())
        );
    }

    #[test]
    fn transform_variant_maps_fields() {
        let product = sample_product();
        let variant = product.variants.first().unwrap();
        let options = NormalizedFeedOptions {
            enable_search: true,
            enable_checkout: true,
            currency: "USD".to_string(),
            default_shipping_cost: "5.00".to_string(),
            seller_name: "Stateset".to_string(),
            seller_url: "https://mystore.myshopify.com".to_string(),
            product_base_url: Some("https://mystore.myshopify.com/products".to_string()),
            seller_privacy_policy: Some("https://example.com/privacy".to_string()),
            seller_tos: Some("https://example.com/tos".to_string()),
            return_policy: Some("https://example.com/returns".to_string()),
            return_window: 30,
        };

        let item = transform_shopify_variant(&product, variant, &options);

        assert_eq!(item.id, "456");
        assert_eq!(item.title, "Test Product");
        assert_eq!(
            item.link,
            "https://mystore.myshopify.com/products/test-product"
        );
        assert_eq!(item.availability, "in_stock");
        assert_eq!(item.image_link.unwrap(), "https://example.com/primary.jpg");
        assert_eq!(
            item.additional_image_link.unwrap(),
            "https://example.com/secondary.jpg"
        );
        assert_eq!(item.price, "29.99 USD");
        assert_eq!(item.sale_price.unwrap(), "19.99 USD");
        assert_eq!(item.color.unwrap(), "Blue");
        assert_eq!(item.size.unwrap(), "Large");
        assert_eq!(item.offer_id.unwrap(), "test-product-456");
        assert_eq!(item.seller_name, "Stateset");
        assert_eq!(item.seller_url, "https://mystore.myshopify.com");
        assert_eq!(item.return_window.unwrap(), 30);
    }

    #[test]
    fn generate_handle_from_name() {
        let id = Uuid::new_v4();
        let handle = super::generate_product_handle("My Great Product", &id);
        assert_eq!(handle, "my-great-product");
    }

    #[test]
    fn generate_handle_fallbacks_to_uuid() {
        let id = Uuid::new_v4();
        let handle = super::generate_product_handle("!!!", &id);
        assert_eq!(handle, id.to_string());
    }
}
