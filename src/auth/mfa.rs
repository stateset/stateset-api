/*!
 * # Multi-Factor Authentication (MFA) Module
 *
 * This module provides multi-factor authentication support including:
 * - TOTP (Time-based One-Time Password) using authenticator apps
 * - SMS-based OTP
 * - Email-based OTP
 * - Hardware security keys (future)
 * - Backup codes for recovery
 */

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use rand::Rng;
use tracing::{debug, error, info};

#[derive(Error, Debug)]
pub enum MfaError {
    #[error("Invalid TOTP code")]
    InvalidTotpCode,
    
    #[error("TOTP code expired")]
    TotpCodeExpired,
    
    #[error("MFA not enabled for this user")]
    MfaNotEnabled,
    
    #[error("Invalid backup code")]
    InvalidBackupCode,
    
    #[error("No backup codes remaining")]
    NoBackupCodes,
    
    #[error("MFA setup required")]
    SetupRequired,
    
    #[error("Invalid MFA method")]
    InvalidMethod,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("MFA code already used")]
    CodeAlreadyUsed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MfaMethod {
    Totp,
    Sms,
    Email,
    HardwareKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSecret {
    pub method: MfaMethod,
    pub secret: String,
    pub backup_codes: Vec<String>,
    pub created_at: u64,
    pub last_used: Option<u64>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct TotpConfig {
    pub issuer: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u64,
    pub window: i64, // Allowable time window for code validation
}

impl Default for TotpConfig {
    fn default() -> Self {
        Self {
            issuer: "Stateset API".to_string(),
            algorithm: "SHA1".to_string(),
            digits: 6,
            period: 30,
            window: 1, // Allow 1 period before/after current time
        }
    }
}

pub struct MfaService {
    config: TotpConfig,
    secrets: Arc<RwLock<HashMap<String, MfaSecret>>>,
    rate_limiter: Arc<RwLock<HashMap<String, Vec<u64>>>>, // user_id -> timestamps
}

impl MfaService {
    pub fn new() -> Self {
        Self {
            config: TotpConfig::default(),
            secrets: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Generate a new TOTP secret for a user
    pub async fn generate_totp_secret(&self, user_id: &str, account_name: &str) -> Result<(String, String), MfaError> {
        let secret = self.generate_random_secret(32);
        let uri = self.generate_totp_uri(&secret, account_name);
        
        let mfa_secret = MfaSecret {
            method: MfaMethod::Totp,
            secret: secret.clone(),
            backup_codes: self.generate_backup_codes(10),
            created_at: Self::current_timestamp(),
            last_used: None,
            enabled: false,
        };
        
        let mut secrets = self.secrets.write().await;
        secrets.insert(user_id.to_string(), mfa_secret);
        
        Ok((secret, uri))
    }
    
    /// Verify a TOTP code
    pub async fn verify_totp_code(&self, user_id: &str, code: &str) -> Result<bool, MfaError> {
        // Check rate limiting
        if !self.check_rate_limit(user_id).await {
            return Err(MfaError::RateLimitExceeded);
        }
        
        let secrets = self.secrets.read().await;
        let secret = secrets.get(user_id).ok_or(MfaError::MfaNotEnabled)?;
        
        if !secret.enabled {
            return Err(MfaError::MfaNotEnabled);
        }
        
        let code_int = code.parse::<u32>().map_err(|_| MfaError::InvalidTotpCode)?;
        
        // Check current time window and adjacent windows
        let current_time = Self::current_timestamp();
        let time_step = current_time / self.config.period;
        
        for offset in -self.config.window..=self.config.window {
            let check_time = ((time_step as i64) + offset) as u64 * self.config.period;
            let expected_code = self.generate_totp_code(&secret.secret, check_time)?;
            
            if code_int == expected_code {
                // Mark as used
                drop(secrets);
                let mut secrets_mut = self.secrets.write().await;
                if let Some(secret_mut) = secrets_mut.get_mut(user_id) {
                    secret_mut.last_used = Some(current_time);
                }
                
                return Ok(true);
            }
        }
        
        Err(MfaError::InvalidTotpCode)
    }
    
    /// Enable MFA for a user after successful setup verification
    pub async fn enable_mfa(&self, user_id: &str) -> Result<(), MfaError> {
        let mut secrets = self.secrets.write().await;
        if let Some(secret) = secrets.get_mut(user_id) {
            secret.enabled = true;
            Ok(())
        } else {
            Err(MfaError::MfaNotEnabled)
        }
    }
    
    /// Disable MFA for a user
    pub async fn disable_mfa(&self, user_id: &str) -> Result<(), MfaError> {
        let mut secrets = self.secrets.write().await;
        secrets.remove(user_id);
        Ok(())
    }
    
    /// Verify a backup code
    pub async fn verify_backup_code(&self, user_id: &str, code: &str) -> Result<bool, MfaError> {
        let mut secrets = self.secrets.write().await;
        if let Some(secret) = secrets.get_mut(user_id) {
            if let Some(pos) = secret.backup_codes.iter().position(|c| c == code) {
                secret.backup_codes.remove(pos);
                secret.last_used = Some(Self::current_timestamp());
                return Ok(true);
            }
        }
        Err(MfaError::InvalidBackupCode)
    }
    
    /// Get remaining backup codes count
    pub async fn get_backup_codes_count(&self, user_id: &str) -> usize {
        let secrets = self.secrets.read().await;
        secrets.get(user_id)
            .map(|s| s.backup_codes.len())
            .unwrap_or(0)
    }
    
    /// Regenerate backup codes
    pub async fn regenerate_backup_codes(&self, user_id: &str) -> Result<Vec<String>, MfaError> {
        let mut secrets = self.secrets.write().await;
        if let Some(secret) = secrets.get_mut(user_id) {
            let new_codes = self.generate_backup_codes(10);
            secret.backup_codes = new_codes.clone();
            Ok(new_codes)
        } else {
            Err(MfaError::MfaNotEnabled)
        }
    }
    
    /// Check if MFA is enabled for a user
    pub async fn is_mfa_enabled(&self, user_id: &str) -> bool {
        let secrets = self.secrets.read().await;
        secrets.get(user_id)
            .map(|s| s.enabled)
            .unwrap_or(false)
    }
    
    /// Get MFA status for a user
    pub async fn get_mfa_status(&self, user_id: &str) -> Option<MfaSecret> {
        let secrets = self.secrets.read().await;
        secrets.get(user_id).cloned()
    }
    
    // Helper methods
    
    fn generate_random_secret(&self, length: usize) -> String {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..length).map(|_| rng.gen()).collect();
        general_purpose::STANDARD.encode(&bytes)
    }
    
    fn generate_totp_uri(&self, secret: &str, account_name: &str) -> String {
        format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm={}&digits={}&period={}",
            self.config.issuer,
            account_name,
            secret,
            self.config.issuer,
            self.config.algorithm,
            self.config.digits,
            self.config.period
        )
    }
    
    fn generate_totp_code(&self, secret: &str, time: u64) -> Result<u32, MfaError> {
        let secret_bytes = general_purpose::STANDARD.decode(secret)
            .map_err(|_| MfaError::InvalidTotpCode)?;
        
        let time_bytes = time.to_be_bytes();
        
        // HMAC-SHA1
        let mut mac = Hmac::<Sha1>::new_from_slice(&secret_bytes)
            .map_err(|_| MfaError::InvalidTotpCode)?;
        mac.update(&time_bytes);
        let result = mac.finalize().into_bytes();
        
        // Dynamic truncation
        let offset = (result[19] & 0xf) as usize;
        let code = ((result[offset] & 0x7f) as u32) << 24
            | (result[offset + 1] as u32) << 16
            | (result[offset + 2] as u32) << 8
            | (result[offset + 3] as u32);
        
        // Get the requested number of digits
        let modulus = 10u32.pow(self.config.digits);
        Ok(code % modulus)
    }
    
    fn generate_backup_codes(&self, count: usize) -> Vec<String> {
        let mut codes = Vec::new();
        let mut rng = rand::thread_rng();
        
        for _ in 0..count {
            let code: String = (0..8)
                .map(|_| rng.gen_range(0..10).to_string())
                .collect();
            codes.push(code);
        }
        
        codes
    }
    
    async fn check_rate_limit(&self, user_id: &str) -> bool {
        let mut rate_limiter = self.rate_limiter.write().await;
        let timestamps = rate_limiter.entry(user_id.to_string()).or_insert_with(Vec::new);
        
        let now = Self::current_timestamp();
        let window_start = now - 300; // 5 minute window
        
        // Remove old timestamps
        timestamps.retain(|&t| t > window_start);
        
        // Check if under limit (max 10 attempts per 5 minutes)
        if timestamps.len() >= 10 {
            return false;
        }
        
        timestamps.push(now);
        true
    }
    
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// MFA middleware for protecting routes
pub mod middleware {
    use axum::{
        extract::{Request, State},
        http::StatusCode,
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use std::sync::Arc;
    
    use super::{MfaService, MfaError};
    
    pub async fn require_mfa(
        State(mfa_service): State<Arc<MfaService>>,
        mut request: Request,
        next: Next,
    ) -> Response {
        // Extract user ID from request (this would come from auth middleware)
        let user_id = request.headers()
            .get("x-user-id")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("anonymous");
        
        // Check if MFA is required for this route
        let require_mfa = request.headers()
            .get("x-require-mfa")
            .and_then(|h| h.to_str().ok())
            .map(|s| s == "true")
            .unwrap_or(false);
        
        if require_mfa {
            if !mfa_service.is_mfa_enabled(user_id).await {
                return (StatusCode::FORBIDDEN, "MFA required but not enabled").into_response();
            }
            
            // Check if MFA code was provided and verified
            let mfa_verified = request.headers()
                .get("x-mfa-verified")
                .and_then(|h| h.to_str().ok())
                .map(|s| s == "true")
                .unwrap_or(false);
            
            if !mfa_verified {
                return (StatusCode::FORBIDDEN, "MFA verification required").into_response();
            }
        }
        
        next.run(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_totp_generation_and_verification() {
        let service = MfaService::new();
        let user_id = "test_user";
        
        // Generate secret
        let (secret, uri) = service.generate_totp_secret(user_id, "test@example.com").await.unwrap();
        assert!(!secret.is_empty());
        assert!(uri.contains("otpauth://"));
        
        // Enable MFA
        service.enable_mfa(user_id).await.unwrap();
        
        // Generate a valid code
        let current_time = MfaService::current_timestamp();
        let valid_code = service.generate_totp_code(&secret, current_time).unwrap();
        let code_str = format!("{:06}", valid_code);
        
        // Verify the code
        let result = service.verify_totp_code(user_id, &code_str).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
    
    #[tokio::test]
    async fn test_invalid_code_rejection() {
        let service = MfaService::new();
        let user_id = "test_user";
        
        service.generate_totp_secret(user_id, "test@example.com").await.unwrap();
        service.enable_mfa(user_id).await.unwrap();
        
        // Try invalid code
        let result = service.verify_totp_code(user_id, "000000").await;
        assert!(matches!(result, Err(MfaError::InvalidTotpCode)));
    }
    
    #[tokio::test]
    async fn test_backup_codes() {
        let service = MfaService::new();
        let user_id = "test_user";
        
        service.generate_totp_secret(user_id, "test@example.com").await.unwrap();
        service.enable_mfa(user_id).await.unwrap();
        
        let initial_count = service.get_backup_codes_count(user_id).await;
        assert_eq!(initial_count, 10);
        
        // Use a backup code
        let codes = service.regenerate_backup_codes(user_id).await.unwrap();
        let test_code = codes[0].clone();
        
        let result = service.verify_backup_code(user_id, &test_code).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Count should decrease
        let new_count = service.get_backup_codes_count(user_id).await;
        assert_eq!(new_count, 9);
        
        // Try to use the same code again
        let result = service.verify_backup_code(user_id, &test_code).await;
        assert!(matches!(result, Err(MfaError::InvalidBackupCode)));
    }
    
    #[tokio::test]
    async fn test_mfa_disabled_by_default() {
        let service = MfaService::new();
        let user_id = "test_user";
        
        service.generate_totp_secret(user_id, "test@example.com").await.unwrap();
        
        // Should not be enabled by default
        assert!(!service.is_mfa_enabled(user_id).await);
        
        // Verification should fail
        let result = service.verify_totp_code(user_id, "123456").await;
        assert!(matches!(result, Err(MfaError::MfaNotEnabled)));
    }
}
