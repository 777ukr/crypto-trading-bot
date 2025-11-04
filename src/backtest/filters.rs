//! Фильтры и селекторы рынков для стратегий

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketFilters {
    pub delta_filters: Vec<DeltaFilter>,
    pub volume_filters: Vec<VolumeFilter>,
    pub funding_rate_filter: Option<FundingRateFilter>,
    pub price_step_filter: Option<PriceStepFilter>,
    pub mark_price_filter: Option<MarkPriceFilter>,
    pub white_list: Vec<String>,
    pub black_list: Vec<String>,
    pub exclude_blacklist_from_delta: bool, // Исключить из расчета дельт
    pub max_active_markets: usize,
    pub quote_asset: Option<String>, // USDT, BTC, etc.
    
    // Enhanced filters
    pub dont_buy_if_price_changed_more: Option<f64>, // % изменения цены
    pub min_pump_quality: Option<f64>, // Минимальный "pump Q"
    pub min_daily_volume_btc: Option<f64>, // Минимальный дневной объем в BTC
    pub max_3h_volume_btc: Option<f64>, // Максимальный объем за 3ч в BTC
    pub dont_buy_pumped: bool, // Не покупать уже "накачанные" монеты
    pub dont_buy_new_coins_minutes: Option<u32>, // Не покупать ново-добавленные монеты N минут
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaFilter {
    pub time_window: TimeWindow,
    pub min_delta: Option<f64>, // Минимальная дельта (%)
    pub max_delta: Option<f64>, // Максимальная дельта (%)
    pub is_absolute: bool,       // Абсолютная или относительная дельта
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeWindow {
    Min1,
    Min3,
    Min5,
    Min15,
    Min30,
    Hour1,
    Hour24,
    Custom(Duration),
}

impl TimeWindow {
    pub fn to_duration(&self) -> Duration {
        match self {
            TimeWindow::Min1 => Duration::minutes(1),
            TimeWindow::Min3 => Duration::minutes(3),
            TimeWindow::Min5 => Duration::minutes(5),
            TimeWindow::Min15 => Duration::minutes(15),
            TimeWindow::Min30 => Duration::minutes(30),
            TimeWindow::Hour1 => Duration::hours(1),
            TimeWindow::Hour24 => Duration::hours(24),
            TimeWindow::Custom(d) => *d,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeFilter {
    pub min_volume_24h: Option<f64>, // Минимальный объем за 24ч (USDT)
    pub max_volume_24h: Option<f64>,
    pub min_liquidity: Option<f64>,  // Минимальная ликвидность
    pub min_volatility: Option<f64>, // Минимальная волатильность
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateFilter {
    pub min_rate: f64,              // Минимальная ставка (%)
    pub max_rate: f64,              // Максимальная ставка (%)
    pub before_payout: Option<Duration>, // До выплаты
    pub after_payout: Option<Duration>,  // После выплаты
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceStepFilter {
    pub min_step: f64,              // Минимальный шаг цены
    pub max_step: f64,              // Максимальный шаг цены
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkPriceFilter {
    pub max_deviation: f64,         // Максимальное отклонение от марк прайса (%)
}

pub struct MarketSelector {
    filters: MarketFilters,
    last_update: DateTime<Utc>,
    update_interval: Duration,
}

impl MarketSelector {
    pub fn new(filters: MarketFilters, update_interval: Duration) -> Self {
        Self {
            filters,
            last_update: Utc::now(),
            update_interval,
        }
    }
    
    /// Проверка, проходит ли символ все фильтры
    pub fn check_symbol(
        &self,
        symbol: &str,
        market_data: &MarketDataSnapshot,
        _current_time: DateTime<Utc>,
    ) -> bool {
        // Черный список
        if self.filters.black_list.contains(&symbol.to_string()) {
            return false;
        }
        
        // Белый список
        if !self.filters.white_list.is_empty() {
            if !self.filters.white_list.contains(&symbol.to_string()) {
                return false;
            }
        }
        
        // Quote asset фильтр
        if let Some(ref quote) = self.filters.quote_asset {
            if !symbol.ends_with(quote) {
                return false;
            }
        }
        
        // Дельта фильтры
        for filter in &self.filters.delta_filters {
            if !self.check_delta_filter(filter, market_data) {
                return false;
            }
        }
        
        // Объемные фильтры
        for filter in &self.filters.volume_filters {
            if !self.check_volume_filter(filter, market_data) {
                return false;
            }
        }
        
        // Фильтр ставки финансирования
        if let Some(ref filter) = self.filters.funding_rate_filter {
            if let Some(funding_rate) = market_data.funding_rate {
                if funding_rate < filter.min_rate || funding_rate > filter.max_rate {
                    return false;
                }
            }
        }
        
        // Фильтр шага цены
        if let Some(ref filter) = self.filters.price_step_filter {
            if let Some(price_step) = market_data.price_step {
                if price_step < filter.min_step || price_step > filter.max_step {
                    return false;
                }
            }
        }
        
        // Фильтр марк прайса
        if let Some(ref filter) = self.filters.mark_price_filter {
            if let (Some(mark_price), Some(current_price)) = (market_data.mark_price, market_data.current_price) {
                let deviation = (mark_price - current_price).abs() / current_price * 100.0;
                if deviation > filter.max_deviation {
                    return false;
                }
            }
        }
        
        true
    }
    
    fn check_delta_filter(&self, filter: &DeltaFilter, data: &MarketDataSnapshot) -> bool {
        let delta = data.get_delta_for_window(filter.time_window);
        
        if let Some(min) = filter.min_delta {
            if delta < min {
                return false;
            }
        }
        
        if let Some(max) = filter.max_delta {
            if delta > max {
                return false;
            }
        }
        
        true
    }
    
    fn check_volume_filter(&self, filter: &VolumeFilter, data: &MarketDataSnapshot) -> bool {
        if let Some(min) = filter.min_volume_24h {
            if data.volume_24h < min {
                return false;
            }
        }
        
        if let Some(max) = filter.max_volume_24h {
            if data.volume_24h > max {
                return false;
            }
        }
        
        if let Some(min_liq) = filter.min_liquidity {
            if data.liquidity < min_liq {
                return false;
            }
        }
        
        if let Some(min_vol) = filter.min_volatility {
            if data.volatility < min_vol {
                return false;
            }
        }
        
        true
    }
    
    /// Топ-N рынков по критерию
    pub fn select_top_markets(
        &self,
        markets: &[(String, MarketDataSnapshot)],
        sort_by: SortCriterion,
        limit: usize,
    ) -> Vec<String> {
        let mut sorted: Vec<_> = markets
            .iter()
            .filter(|(symbol, data)| self.check_symbol(symbol, data, Utc::now()))
            .collect();
        
        match sort_by {
            SortCriterion::Volume24h => {
                sorted.sort_by(|a, b| b.1.volume_24h.partial_cmp(&a.1.volume_24h).unwrap());
            }
            SortCriterion::Delta1h => {
                sorted.sort_by(|a, b| {
                    let delta_a = a.1.get_delta_for_window(TimeWindow::Hour1);
                    let delta_b = b.1.get_delta_for_window(TimeWindow::Hour1);
                    delta_b.partial_cmp(&delta_a).unwrap()
                });
            }
            SortCriterion::Volatility => {
                sorted.sort_by(|a, b| b.1.volatility.partial_cmp(&a.1.volatility).unwrap());
            }
            SortCriterion::Liquidity => {
                sorted.sort_by(|a, b| b.1.liquidity.partial_cmp(&a.1.liquidity).unwrap());
            }
        }
        
        sorted
            .into_iter()
            .take(limit)
            .map(|(symbol, _)| symbol.clone())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct MarketDataSnapshot {
    pub symbol: String,
    pub current_price: Option<f64>,
    pub mark_price: Option<f64>,
    pub volume_24h: f64,
    pub liquidity: f64,
    pub volatility: f64,
    pub funding_rate: Option<f64>,
    pub price_step: Option<f64>,
    pub deltas: std::collections::HashMap<TimeWindow, f64>,
}

impl MarketDataSnapshot {
    pub fn get_delta_for_window(&self, window: TimeWindow) -> f64 {
        self.deltas.get(&window).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SortCriterion {
    Volume24h,
    Delta1h,
    Volatility,
    Liquidity,
}

