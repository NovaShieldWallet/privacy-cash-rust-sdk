//! Configuration fetching from the relayer API

use crate::constants::RELAYER_API_URL;
use crate::error::{PrivacyCashError, Result};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Global cached configuration
static CONFIG_CACHE: OnceCell<RwLock<Option<Config>>> = OnceCell::new();

/// Configuration from the relayer API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Fee rate for withdrawals (as a decimal, e.g., 0.01 = 1%)
    pub withdraw_fee_rate: f64,

    /// Rent fee for withdrawals in SOL
    pub withdraw_rent_fee: f64,

    /// Fee rate for deposits
    pub deposit_fee_rate: f64,

    /// USDC-specific withdraw rent fee
    #[serde(default)]
    pub usdc_withdraw_rent_fee: f64,

    /// Rent fees per token
    #[serde(default)]
    pub rent_fees: HashMap<String, f64>,

    /// Minimum withdrawal amounts per token
    #[serde(default)]
    pub minimum_withdrawal: HashMap<String, f64>,

    /// Token prices in USD
    #[serde(default)]
    pub prices: HashMap<String, f64>,
}

/// Supported token information (dynamic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedToken {
    pub name: String,
    pub min_withdrawal: f64,
    pub rent_fee: f64,
    pub price_usd: f64,
}

impl Config {
    /// Fetch configuration from the relayer API
    pub async fn fetch() -> Result<Self> {
        let url = format!("{}/config", *RELAYER_API_URL);

        let response = reqwest::get(&url)
            .await
            .map_err(|e| PrivacyCashError::ApiError(format!("Failed to fetch config: {}", e)))?;

        if !response.status().is_success() {
            return Err(PrivacyCashError::ApiError(format!(
                "Config API returned status: {}",
                response.status()
            )));
        }

        let config: Config = response
            .json()
            .await
            .map_err(|e| PrivacyCashError::ApiError(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Get cached configuration or fetch if not cached
    pub async fn get_or_fetch() -> Result<Self> {
        let cache = CONFIG_CACHE.get_or_init(|| RwLock::new(None));

        // Try to read from cache first
        {
            let read_guard = cache.read();
            if let Some(config) = read_guard.as_ref() {
                return Ok(config.clone());
            }
        }

        // Fetch and cache
        let config = Self::fetch().await?;
        {
            let mut write_guard = cache.write();
            *write_guard = Some(config.clone());
        }

        Ok(config)
    }

    /// Clear the cached configuration
    pub fn clear_cache() {
        if let Some(cache) = CONFIG_CACHE.get() {
            let mut write_guard = cache.write();
            *write_guard = None;
        }
    }

    /// Get withdraw fee rate
    pub async fn get_withdraw_fee_rate() -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        Ok(config.withdraw_fee_rate)
    }

    /// Get withdraw rent fee
    pub async fn get_withdraw_rent_fee() -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        Ok(config.withdraw_rent_fee)
    }

    /// Get deposit fee rate
    pub async fn get_deposit_fee_rate() -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        Ok(config.deposit_fee_rate)
    }

    /// Get rent fee for a specific token
    pub async fn get_token_rent_fee(token_name: &str) -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        config
            .rent_fees
            .get(token_name)
            .copied()
            .ok_or_else(|| PrivacyCashError::ConfigError(format!("No rent fee for {}", token_name)))
    }

    /// Get list of supported token names (dynamically from API)
    pub async fn get_supported_token_names() -> Result<Vec<String>> {
        let config = Self::get_or_fetch().await?;
        Ok(config.minimum_withdrawal.keys().cloned().collect())
    }

    /// Check if a token is supported
    pub async fn is_token_supported(token_name: &str) -> Result<bool> {
        let config = Self::get_or_fetch().await?;
        Ok(config.minimum_withdrawal.contains_key(&token_name.to_lowercase()))
    }

    /// Get minimum withdrawal for a token
    pub async fn get_minimum_withdrawal(token_name: &str) -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        config
            .minimum_withdrawal
            .get(&token_name.to_lowercase())
            .copied()
            .ok_or_else(|| PrivacyCashError::ConfigError(format!("Token {} not supported", token_name)))
    }

    /// Get all supported tokens with their details
    pub async fn get_supported_tokens() -> Result<Vec<SupportedToken>> {
        let config = Self::get_or_fetch().await?;
        
        let mut tokens = Vec::new();
        for (name, min_withdrawal) in &config.minimum_withdrawal {
            let rent_fee = config.rent_fees.get(name).copied().unwrap_or(0.0);
            let price_usd = config.prices.get(name).copied().unwrap_or(0.0);
            
            tokens.push(SupportedToken {
                name: name.clone(),
                min_withdrawal: *min_withdrawal,
                rent_fee,
                price_usd,
            });
        }
        
        Ok(tokens)
    }

    /// Get token price in USD
    pub async fn get_token_price(token_name: &str) -> Result<f64> {
        let config = Self::get_or_fetch().await?;
        config
            .prices
            .get(&token_name.to_lowercase())
            .copied()
            .ok_or_else(|| PrivacyCashError::ConfigError(format!("No price for {}", token_name)))
    }

    /// Alias for get_or_fetch
    pub async fn get() -> Result<Self> {
        Self::get_or_fetch().await
    }
}
