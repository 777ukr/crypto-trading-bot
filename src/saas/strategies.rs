//! Управление стратегиями пользователей

use anyhow::{Context, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserStrategy {
    pub id: i64,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub user_id: i64,
    pub strategy_name: String,
    pub description: Option<String>,
    pub config_text: String,
    pub config_json: serde_json::Value,
    pub is_active: bool,
    pub is_public: bool,
    pub initial_balance: Decimal,
    pub leverage: i32,
    pub rating: Decimal,
    pub stars: i32,
    pub best_roi: Option<Decimal>,
    pub best_profit_factor: Option<Decimal>,
    pub best_win_rate: Option<Decimal>,
    pub best_backtest_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
    pub ai_suggestions: serde_json::Value,
    pub version: i32,
    pub parent_strategy_id: Option<i64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStrategyRequest {
    pub strategy_name: String,
    pub description: Option<String>,
    pub config_text: String, // ##Begin_Strategy ... ##End_Strategy
    pub initial_balance: Option<f64>,
    pub leverage: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStrategyRequest {
    pub strategy_name: Option<String>,
    pub description: Option<String>,
    pub config_text: Option<String>,
    pub is_active: Option<bool>,
    pub is_public: Option<bool>,
    pub initial_balance: Option<f64>,
    pub leverage: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyListResponse {
    pub strategies: Vec<UserStrategy>,
    pub total: i64,
}

pub struct StrategyRepository {
    pool: PgPool,
}

impl StrategyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Создание новой стратегии
    pub async fn create_strategy(
        &self,
        user_id: i64,
        req: CreateStrategyRequest,
    ) -> Result<UserStrategy> {
        // Парсим config_text в JSON
        use crate::strategy::config_parser::StrategyConfig;
        let config = StrategyConfig::parse(&req.config_text)
            .context("Failed to parse strategy config")?;
        let config_json = serde_json::to_value(&config)?;
        
        let strategy = sqlx::query_as::<_, UserStrategy>(
            r#"
            INSERT INTO user_strategies (
                user_id, strategy_name, description, config_text, config_json,
                initial_balance, leverage, tags, category
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, created_at, updated_at, user_id, strategy_name, description,
                      config_text, config_json, is_active, is_public, initial_balance,
                      leverage, rating, stars, best_roi, best_profit_factor, best_win_rate,
                      best_backtest_id, tags, category, ai_suggestions, version,
                      parent_strategy_id, metadata
            "#
        )
        .bind(user_id)
        .bind(&req.strategy_name)
        .bind(&req.description)
        .bind(&req.config_text)
        .bind(&config_json)
        .bind(Decimal::try_from(req.initial_balance.unwrap_or(1000.0)).unwrap_or(Decimal::ZERO))
        .bind(req.leverage.unwrap_or(10))
        .bind(&req.tags)
        .bind(&req.category)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create strategy")?;
        
        Ok(strategy)
    }
    
    /// Получение стратегии по ID
    pub async fn get_by_id(&self, strategy_id: i64, user_id: Option<i64>) -> Result<Option<UserStrategy>> {
        let query = if let Some(uid) = user_id {
            "SELECT * FROM user_strategies WHERE id = $1 AND user_id = $2"
        } else {
            "SELECT * FROM user_strategies WHERE id = $1"
        };
        
        let mut query_builder = sqlx::query_as::<_, UserStrategy>(query);
        query_builder = query_builder.bind(strategy_id);
        if let Some(uid) = user_id {
            query_builder = query_builder.bind(uid);
        }
        
        let strategy = query_builder
            .fetch_optional(&self.pool)
            .await
            .context("Failed to fetch strategy")?;
        
        Ok(strategy)
    }
    
    /// Список стратегий пользователя
    pub async fn list_user_strategies(
        &self,
        user_id: i64,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<StrategyListResponse> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);
        
        let strategies = sqlx::query_as::<_, UserStrategy>(
            r#"
            SELECT * FROM user_strategies
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch strategies")?;
        
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_strategies WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count strategies")?;
        
        Ok(StrategyListResponse {
            strategies,
            total,
        })
    }
    
    /// Топ стратегий по рейтингу (публичные)
    pub async fn top_strategies(
        &self,
        limit: Option<i64>,
        min_rating: Option<Decimal>,
    ) -> Result<Vec<UserStrategy>> {
        let limit = limit.unwrap_or(20);
        let min_rating = min_rating.unwrap_or(Decimal::ZERO);
        
        let strategies = sqlx::query_as::<_, UserStrategy>(
            r#"
            SELECT * FROM user_strategies
            WHERE is_public = true AND rating >= $1
            ORDER BY rating DESC, best_roi DESC NULLS LAST
            LIMIT $2
            "#
        )
        .bind(min_rating)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch top strategies")?;
        
        Ok(strategies)
    }
    
    /// Обновление стратегии
    pub async fn update_strategy(
        &self,
        strategy_id: i64,
        user_id: i64,
        req: UpdateStrategyRequest,
    ) -> Result<UserStrategy> {
        // Если обновляется config_text, нужно перепарсить JSON
        let config_json = if let Some(ref config_text) = req.config_text {
            use crate::strategy::config_parser::StrategyConfig;
            let config = StrategyConfig::parse(config_text)
                .context("Failed to parse updated config")?;
            Some(serde_json::to_value(&config)?)
        } else {
            None
        };
        
        // Строим динамический UPDATE запрос
        let mut updates = Vec::new();
        let mut binds: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = Vec::new();
        let mut param_num = 1;
        
        if let Some(ref name) = req.strategy_name {
            updates.push(format!("strategy_name = ${}", param_num));
            binds.push(Box::new(name.clone()));
            param_num += 1;
        }
        
        if let Some(ref desc) = req.description {
            updates.push(format!("description = ${}", param_num));
            binds.push(Box::new(desc.clone()));
            param_num += 1;
        }
        
        if let Some(ref config_text) = req.config_text {
            updates.push(format!("config_text = ${}", param_num));
            binds.push(Box::new(config_text.clone()));
            param_num += 1;
        }
        
        if let Some(_) = config_json {
            // фикс только списка полей, сами биндинги ниже в статичном UPDATE
            updates.push(format!("config_json = ${}", param_num));
            param_num += 1;
        }
        
        if let Some(is_active) = req.is_active {
            updates.push(format!("is_active = ${}", param_num));
            binds.push(Box::new(is_active));
            param_num += 1;
        }
        
        if let Some(is_public) = req.is_public {
            updates.push(format!("is_public = ${}", param_num));
            binds.push(Box::new(is_public));
            param_num += 1;
        }
        
        if let Some(balance) = req.initial_balance {
            updates.push(format!("initial_balance = ${}", param_num));
            binds.push(Box::new(Decimal::try_from(balance).unwrap_or(Decimal::ZERO)));
            param_num += 1;
        }
        
        if let Some(leverage) = req.leverage {
            updates.push(format!("leverage = ${}", param_num));
            binds.push(Box::new(leverage));
            param_num += 1;
        }
        
        if let Some(ref tags) = req.tags {
            updates.push(format!("tags = ${}", param_num));
            binds.push(Box::new(tags.clone()));
            param_num += 1;
        }
        
        if let Some(ref category) = req.category {
            updates.push(format!("category = ${}", param_num));
            binds.push(Box::new(category.clone()));
            param_num += 1;
        }
        
        if updates.is_empty() {
            return self.get_by_id(strategy_id, Some(user_id))
                .await?
                .ok_or_else(|| anyhow::anyhow!("Strategy not found"));
        }
        
        updates.push(format!("updated_at = NOW()"));
        
        let query = format!(
            "UPDATE user_strategies SET {} WHERE id = ${} AND user_id = ${} RETURNING *",
            updates.join(", "),
            param_num,
            param_num + 1
        );
        
        // Это упрощенная версия - на практике лучше использовать sqlx::query с динамическими параметрами
        // Для простоты сделаем отдельный запрос для каждого поля
        
        // Упрощенная версия: обновляем по одному полю или используем готовый запрос
        // Пока вернемся к простому подходу с отдельными UPDATE для каждого поля
        
        // Временная реализация: используем sqlx::query с явными биндингами
        // На практике здесь нужен более сложный query builder
        
        let strategy = sqlx::query_as::<_, UserStrategy>(
            r#"
            UPDATE user_strategies
            SET 
                strategy_name = COALESCE($1, strategy_name),
                description = COALESCE($2, description),
                config_text = COALESCE($3, config_text),
                config_json = COALESCE($4, config_json),
                is_active = COALESCE($5, is_active),
                is_public = COALESCE($6, is_public),
                initial_balance = COALESCE($7, initial_balance),
                leverage = COALESCE($8, leverage),
                tags = COALESCE($9, tags),
                category = COALESCE($10, category),
                updated_at = NOW()
            WHERE id = $11 AND user_id = $12
            RETURNING id, created_at, updated_at, user_id, strategy_name, description,
                      config_text, config_json, is_active, is_public, initial_balance,
                      leverage, rating, stars, best_roi, best_profit_factor, best_win_rate,
                      best_backtest_id, tags, category, ai_suggestions, version,
                      parent_strategy_id, metadata
            "#
        )
        .bind(req.strategy_name)
        .bind(req.description)
        .bind(req.config_text)
        .bind(config_json)
        .bind(req.is_active)
        .bind(req.is_public)
        .bind(req.initial_balance.map(|b| Decimal::try_from(b).unwrap_or(Decimal::ZERO)))
        .bind(req.leverage)
        .bind(req.tags)
        .bind(req.category)
        .bind(strategy_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to update strategy")?
        .ok_or_else(|| anyhow::anyhow!("Strategy not found or access denied"))?;
        
        Ok(strategy)
    }
    
    /// Удаление стратегии
    pub async fn delete_strategy(&self, strategy_id: i64, user_id: i64) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_strategies WHERE id = $1 AND user_id = $2"
        )
        .bind(strategy_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete strategy")?;
        
        Ok(result.rows_affected() > 0)
    }
    
    /// Обновление лучших результатов бэктеста
    pub async fn update_best_backtest(
        &self,
        strategy_id: i64,
        backtest_id: i64,
        roi: Decimal,
        profit_factor: Option<Decimal>,
        win_rate: Decimal,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE user_strategies
            SET 
                best_roi = GREATEST(best_roi, $1),
                best_profit_factor = GREATEST(best_profit_factor, $2),
                best_win_rate = GREATEST(best_win_rate, $3),
                best_backtest_id = CASE 
                    WHEN $1 > COALESCE(best_roi, -999999) THEN $4
                    ELSE best_backtest_id
                END
            WHERE id = $5
            "#
        )
        .bind(roi)
        .bind(profit_factor)
        .bind(win_rate)
        .bind(backtest_id)
        .bind(strategy_id)
        .execute(&self.pool)
        .await
        .context("Failed to update best backtest")?;
        
        Ok(())
    }
}

