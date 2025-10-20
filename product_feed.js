/**
 * Public Version - Agentic Commerce Product Feed Generator
 * 
 * This is a simplified version that uses environment variables for credentials
 * instead of database lookups. Suitable for demos and single-store deployments.
 * 
 * Required Environment Variables:
 * - SHOPIFY_STORE: Your Shopify store domain (e.g., "mystore.myshopify.com")
 * - SHOPIFY_ACCESS_TOKEN: Your Shopify Admin API access token
 * - OPENAI_FEED_API_KEY (optional): For automatic OpenAI push
 */

import fetch from 'node-fetch';
import axios from "axios";

/**
 * Transform Shopify product data to OpenAI Agentic Commerce Protocol format
 */
const transformToAgenticCommerceFormat = (product, variant, shop, options = {}) => {
  const variantId = variant.id;
  const productId = product.id;
  const offerId = `${product.handle}-${variant.id}`;
  
  // Build product URL
  const productUrl = `https://${shop}/products/${product.handle}`;
  const variantUrl = variant.id ? `${productUrl}?variant=${variant.id}` : productUrl;
  
  // Determine availability
  let availability = 'out_of_stock';
  if (variant.inventory_quantity > 0) {
    availability = 'in_stock';
  }
  
  // Extract variant attributes
  const color = variant.option1 && product.options?.[0]?.name?.toLowerCase() === 'color' ? variant.option1 : null;
  const size = variant.option2 && product.options?.[1]?.name?.toLowerCase() === 'size' ? variant.option2 : null;
  
  // Custom variant handling for options beyond color/size
  const customVariant1 = product.options?.[2] ? {
    category: product.options[2].name,
    option: variant.option3
  } : null;
  
  // Build the feed item according to OpenAI Agentic Commerce Protocol
  const feedItem = {
    // OpenAI Flags
    enable_search: options.enable_search !== undefined ? options.enable_search : true,
    enable_checkout: options.enable_checkout !== undefined ? options.enable_checkout : true,
    
    // Basic Product Data
    id: String(variantId),
    gtin: variant.barcode || null,
    mpn: variant.sku || null,
    title: variant.title !== 'Default Title' ? `${product.title} - ${variant.title}` : product.title,
    description: product.body_html ? product.body_html.replace(/<[^>]*>/g, '').substring(0, 5000) : product.title,
    link: variantUrl,
    
    // Item Information
    condition: 'new',
    product_category: product.product_type || 'General',
    brand: product.vendor || null,
    material: null, // Not available in standard Shopify API
    weight: variant.weight ? `${variant.weight} ${variant.weight_unit}` : null,
    
    // Media
    image_link: variant.image_id 
      ? product.images?.find(img => img.id === variant.image_id)?.src 
      : product.image?.src || product.images?.[0]?.src || null,
    additional_image_link: product.images?.slice(1, 5).map(img => img.src).join(',') || null,
    
    // Price & Promotions
    price: `${variant.price} ${options.currency || 'USD'}`,
    sale_price: variant.compare_at_price ? `${variant.price} ${options.currency || 'USD'}` : null,
    
    // Availability & Inventory
    availability: availability,
    inventory_quantity: variant.inventory_quantity || 0,
    
    // Variants (if product has multiple variants)
    item_group_id: product.variants.length > 1 ? String(productId) : null,
    item_group_title: product.variants.length > 1 ? product.title : null,
    color: color,
    size: size,
    offer_id: offerId,
    
    // Custom variants
    custom_variant1_category: customVariant1?.category || null,
    custom_variant1_option: customVariant1?.option || null,
    
    // Fulfillment
    shipping: variant.requires_shipping ? `US::Standard:${options.default_shipping_cost || '0.00'} ${options.currency || 'USD'}` : null,
    
    // Merchant Info
    seller_name: options.seller_name || shop.split('.myshopify.com')[0],
    seller_url: `https://${shop}`,
    seller_privacy_policy: options.seller_privacy_policy || null,
    seller_tos: options.seller_tos || null,
    
    // Returns (required if enable_checkout is true)
    return_policy: options.return_policy || null,
    return_window: options.return_window || null,
    
    // Performance Signals
    popularity_score: null,
    return_rate: null,
    
    // Reviews and Q&A
    product_review_count: null,
    product_review_rating: null,
    
    // Additional Shopify-specific metadata
    shopify_product_id: String(productId),
    shopify_variant_id: String(variantId),
    product_handle: product.handle,
    tags: product.tags || null,
    created_at: product.created_at,
    updated_at: product.updated_at,
  };
  
  // Remove null values to keep feed clean
  return Object.fromEntries(
    Object.entries(feedItem).filter(([_, value]) => value !== null)
  );
};

/**
 * Fetch products from Shopify with pagination
 */
async function fetchShopifyProducts(shop, shopify_access_token, limit = 250) {
  const axiosConfig = {
    headers: {
      'Content-Type': 'application/json',
      'X-Shopify-Access-Token': shopify_access_token
    },
  };
  
  let allProducts = [];
  let nextPageUrl = `https://${shop}/admin/api/2025-04/products.json?limit=${limit}`;
  
  try {
    while (nextPageUrl) {
      console.log(`Fetching products from: ${nextPageUrl}`);
      
      const response = await axios.get(nextPageUrl, axiosConfig);
      const products = response.data.products;
      
      allProducts = allProducts.concat(products);
      
      // Check for pagination link in headers
      const linkHeader = response.headers.link;
      if (linkHeader) {
        const nextLink = linkHeader.split(',').find(link => link.includes('rel="next"'));
        if (nextLink) {
          const match = nextLink.match(/<(.+?)>/);
          nextPageUrl = match ? match[1] : null;
        } else {
          nextPageUrl = null;
        }
      } else {
        nextPageUrl = null;
      }
      
      console.log(`Fetched ${products.length} products. Total: ${allProducts.length}`);
    }
    
    return allProducts;
  } catch (error) {
    console.error('Error fetching Shopify products:', error.message);
    throw error;
  }
}

/**
 * Push feed to OpenAI's endpoint
 */
async function pushFeedToOpenAI(feed, openai_endpoint, openai_api_key) {
  try {
    console.log('Pushing feed to OpenAI endpoint...');
    
    const response = await fetch(openai_endpoint, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${openai_api_key}`
      },
      body: JSON.stringify(feed)
    });
    
    if (!response.ok) {
      const error = await response.text();
      throw new Error(`OpenAI API error: ${response.status} - ${error}`);
    }
    
    const result = await response.json();
    console.log('Successfully pushed feed to OpenAI');
    return result;
  } catch (error) {
    console.error('Error pushing feed to OpenAI:', error.message);
    throw error;
  }
}

/**
 * Main function to create Agentic Commerce product feed
 * 
 * This public version reads credentials from environment variables instead of a database.
 * Perfect for single-store deployments, demos, or open-source projects.
 */
const createAgenticCommerceFeed = async (req, res) => {
  console.log('Creating Agentic Commerce Product Feed (Public Version)');
  
  const format = req.body.format || 'json'; // Support json, csv, tsv
  const maxProducts = req.body.max_products || null; // Optional limit for testing
  
  // Feed customization options
  const feedOptions = {
    enable_search: req.body.enable_search !== undefined ? req.body.enable_search : true,
    enable_checkout: req.body.enable_checkout !== undefined ? req.body.enable_checkout : true,
    currency: req.body.currency || 'USD',
    default_shipping_cost: req.body.default_shipping_cost || '0.00',
    seller_name: req.body.seller_name || null,
    seller_privacy_policy: req.body.seller_privacy_policy || null,
    seller_tos: req.body.seller_tos || null,
    return_policy: req.body.return_policy || null,
    return_window: req.body.return_window || 30, // days
  };
  
  // OpenAI push options
  const pushToOpenAI = req.body.push_to_openai || false;
  const openai_endpoint = req.body.openai_endpoint || null;
  const openai_api_key = req.body.openai_api_key || process.env.OPENAI_FEED_API_KEY || null;
  
  // Validate required fields if checkout is enabled
  if (feedOptions.enable_checkout) {
    if (!feedOptions.seller_privacy_policy || !feedOptions.seller_tos || !feedOptions.return_policy) {
      return res.status(400).json({
        success: false,
        error: 'When enable_checkout is true, you must provide seller_privacy_policy, seller_tos, and return_policy URLs'
      });
    }
  }
  
  try {
    // Read credentials from environment variables
    const shop = process.env.SHOPIFY_STORE;
    const shopify_access_token = process.env.SHOPIFY_ACCESS_TOKEN;
    
    if (!shop || !shopify_access_token) {
      return res.status(500).json({
        success: false,
        error: 'Shopify credentials not configured. Please set SHOPIFY_STORE and SHOPIFY_ACCESS_TOKEN environment variables.'
      });
    }
    
    // Fetch all products from Shopify
    console.log('Fetching products from Shopify...');
    const products = await fetchShopifyProducts(shop, shopify_access_token);
    
    // Transform products to Agentic Commerce format
    console.log('Transforming products to Agentic Commerce format...');
    const feedItems = [];
    
    for (const product of products) {
      // Create feed item for each variant
      for (const variant of product.variants) {
        const feedItem = transformToAgenticCommerceFormat(product, variant, shop, feedOptions);
        feedItems.push(feedItem);
        
        // Check if we've reached max products limit
        if (maxProducts && feedItems.length >= maxProducts) {
          break;
        }
      }
      
      if (maxProducts && feedItems.length >= maxProducts) {
        break;
      }
    }
    
    console.log(`Transformed ${feedItems.length} product variants`);
    
    // Generate feed in requested format
    let feedOutput;
    let contentType;
    
    switch (format.toLowerCase()) {
      case 'json':
        feedOutput = {
          feed_metadata: {
            generated_at: new Date().toISOString(),
            product_count: feedItems.length,
            merchant: shop,
            format: 'agentic_commerce_v1',
            enable_checkout: feedOptions.enable_checkout
          },
          products: feedItems
        };
        contentType = 'application/json';
        break;
        
      case 'csv':
      case 'tsv':
        const delimiter = format === 'csv' ? ',' : '\t';
        const headers = Object.keys(feedItems[0] || {});
        const rows = feedItems.map(item => 
          headers.map(header => {
            const value = item[header] || '';
            // Escape values containing delimiter or quotes
            return typeof value === 'string' && (value.includes(delimiter) || value.includes('"'))
              ? `"${value.replace(/"/g, '""')}"`
              : value;
          }).join(delimiter)
        );
        feedOutput = [headers.join(delimiter), ...rows].join('\n');
        contentType = format === 'csv' ? 'text/csv' : 'text/tab-separated-values';
        break;
        
      default:
        return res.status(400).json({
          success: false,
          error: 'Unsupported format. Use json, csv, or tsv'
        });
    }
    
    // Push to OpenAI if requested
    let openaiResponse = null;
    if (pushToOpenAI && openai_endpoint && openai_api_key) {
      try {
        openaiResponse = await pushFeedToOpenAI(feedOutput, openai_endpoint, openai_api_key);
      } catch (error) {
        console.error('Failed to push to OpenAI:', error);
        // Continue anyway and return the feed
      }
    }
    
    // Return the feed
    return res.status(200).json({
      success: true,
      message: `Generated feed with ${feedItems.length} products`,
      feed: format === 'json' ? feedOutput : feedOutput,
      metadata: {
        format: format,
        product_count: feedItems.length,
        generated_at: new Date().toISOString(),
        merchant: shop,
        enable_checkout: feedOptions.enable_checkout,
        pushed_to_openai: pushToOpenAI && openaiResponse !== null
      },
      openai_response: openaiResponse
    });
    
  } catch (error) {
    console.error('Error creating Agentic Commerce feed:', error);
    return res.status(500).json({
      success: false,
      error: error.message,
      stack: process.env.NODE_ENV === 'development' ? error.stack : undefined
    });
  }
};

export default createAgenticCommerceFeed;