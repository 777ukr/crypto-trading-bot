//! Парсер конфигурационных файлов стратегий в формате ##Begin_Strategy ... ##End_Strategy
//! Поддерживает формат из существующего бота пользователя

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub active: bool,
    pub f_version: u32,
    pub strategy_name: String,
    pub last_edit_date: String,
    pub signal_type: String,
    pub silent_no_charts: bool,
    pub report_trades_to_telegram: bool,
    pub sound_kind: String,
    pub independent_signals: bool,
    
    // Фильтры по монетам
    pub coins_white_list: Vec<String>,
    pub coins_black_list: Vec<String>,
    
    // Параметры EMA и сигналов
    pub next_detect_penalty: u32,
    pub custom_ema: String, // MAX(30s,2s) < -0.8 and MIN(1h,2s) < 1 and MIN(120s,2s) > -0.2
    pub penalty_time: u32,
    pub trade_penalty_time: u32,
    pub binance_price_bug: f64,
    
    // Фильтры по объему
    pub min_volume: f64,
    pub max_volume: f64,
    pub min_hourly_volume: f64,
    pub max_hourly_volume: f64,
    
    // Дельты (изменения цен)
    pub delta_3h_max: f64,
    pub delta_24h_min: f64,
    pub delta_24h_max: f64,
    pub delta2_max: f64,
    pub delta_btc_min: f64,
    pub delta_btc_max: f64,
    pub delta_btc_24_min: f64,
    pub delta_btc_24_max: f64,
    pub delta_btc_5m_max: f64,
    pub delta_market_min: f64,
    pub delta_market_max: f64,
    pub delta_market_24_min: f64,
    pub delta_market_24_max: f64,
    
    // Триггеры
    pub trigger_key: u32,
    pub trigger_seconds: u32,
    pub max_active_orders: u32,
    pub max_markets: u32,
    
    // Параметры покупки
    pub auto_cancel_buy: f64,
    pub order_size: f64,
    pub buy_price: f64,
    pub use_30_sec_old_ask: bool,
    pub buy_modifier: f64,
    pub add_price_bug: f64,
    pub buy_price_step: f64,
    pub order_size_step: f64,
    
    // Параметры продажи
    pub sell_price: f64,
    pub price_down_delay: u32,
    pub price_down_percent: f64,
    pub price_down_allowed_drop: f64,
    pub use_scalping_mode: bool,
    pub sell_ema_check_enter: bool,
    
    // Уровни продажи
    pub sell_level_time: u32,
    pub sell_level_count: u32,
    pub sell_level_adjust: f64,
    pub sell_level_allowed_drop: f64,
    
    // Stop Loss
    pub use_stop_loss: bool,
    pub stop_loss_delay: u32,
    pub stop_loss: f64,
    pub stop_loss_spread: f64,
    pub allowed_drop: f64,
    pub second_stop_loss: f64,
    pub stop_loss3: f64,
    pub allowed_drop3: f64,
    
    // Trailing Stop
    pub use_trailing: bool,
    pub trailing_percent: f64,
    
    // Take Profit
    pub use_take_profit: bool,
    pub take_profit: f64,
    
    pub strategy_penalty: u32,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        StrategyConfig {
            active: false,
            f_version: 12,
            strategy_name: String::new(),
            last_edit_date: String::new(),
            signal_type: "EMA".to_string(),
            silent_no_charts: false,
            report_trades_to_telegram: false,
            sound_kind: "gold".to_string(),
            independent_signals: true,
            coins_white_list: Vec::new(),
            coins_black_list: Vec::new(),
            next_detect_penalty: 300,
            custom_ema: String::new(),
            penalty_time: 30,
            trade_penalty_time: 99,
            binance_price_bug: 0.5,
            min_volume: 0.0,
            max_volume: 9999999.0,
            min_hourly_volume: 0.0,
            max_hourly_volume: 10000000000.0,
            delta_3h_max: 4.0,
            delta_24h_min: 1.0,
            delta_24h_max: 8.0,
            delta2_max: 2000.0,
            delta_btc_min: -50.0,
            delta_btc_max: 50.0,
            delta_btc_24_min: -100.0,
            delta_btc_24_max: 5.0,
            delta_btc_5m_max: 100.0,
            delta_market_min: -50.0,
            delta_market_max: 1000.0,
            delta_market_24_min: -1000.0,
            delta_market_24_max: 1000.0,
            trigger_key: 16,
            trigger_seconds: 360,
            max_active_orders: 30,
            max_markets: 80,
            auto_cancel_buy: 2000.0,
            order_size: 6.0,
            buy_price: 0.0,
            use_30_sec_old_ask: false,
            buy_modifier: -1.0,
            add_price_bug: 0.2,
            buy_price_step: -0.33,
            order_size_step: 1.0,
            sell_price: 1.35,
            price_down_delay: 0,
            price_down_percent: 0.0,
            price_down_allowed_drop: 0.2,
            use_scalping_mode: false,
            sell_ema_check_enter: false,
            sell_level_time: 0,
            sell_level_count: 0,
            sell_level_adjust: 0.0,
            sell_level_allowed_drop: 0.0,
            use_stop_loss: false,
            stop_loss_delay: 20,
            stop_loss: -1.0,
            stop_loss_spread: 1.0,
            allowed_drop: -60.0,
            second_stop_loss: -1.0,
            stop_loss3: 0.0,
            allowed_drop3: -0.1,
            use_trailing: false,
            trailing_percent: -0.1,
            use_take_profit: false,
            take_profit: 0.3,
            strategy_penalty: 0,
        }
    }
}

impl StrategyConfig {
    /// Парсит конфигурацию стратегии из текста формата ##Begin_Strategy ... ##End_Strategy
    pub fn parse(config_text: &str) -> anyhow::Result<Self> {
        let mut config = StrategyConfig::default();
        let lines: Vec<&str> = config_text.lines().collect();
        
        let mut in_strategy = false;
        let mut params = HashMap::new();
        
        for line in lines {
            let line = line.trim();
            
            if line.starts_with("##Begin_Strategy") {
                in_strategy = true;
                continue;
            }
            
            if line.starts_with("##End_Strategy") {
                break;
            }
            
            if !in_strategy {
                continue;
            }
            
            // Парсим параметры вида Key=Value
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                params.insert(key.to_string(), value.to_string());
            }
        }
        
        // Заполняем конфигурацию из параметров
        if let Some(v) = params.get("Active") {
            config.active = parse_bool(v)?;
        }
        
        if let Some(v) = params.get("FVersion") {
            config.f_version = v.parse()?;
        }
        
        if let Some(v) = params.get("StrategyName") {
            config.strategy_name = v.to_string();
        }
        
        if let Some(v) = params.get("LastEditDate") {
            config.last_edit_date = v.to_string();
        }
        
        if let Some(v) = params.get("SignalType") {
            config.signal_type = v.to_string();
        }
        
        if let Some(v) = params.get("SilentNoCharts") {
            config.silent_no_charts = parse_bool(v)?;
        }
        
        if let Some(v) = params.get("ReportTradesToTelegram") {
            config.report_trades_to_telegram = parse_bool(v)?;
        }
        
        if let Some(v) = params.get("SoundKind") {
            config.sound_kind = v.to_string();
        }
        
        if let Some(v) = params.get("IndependentSignals") {
            config.independent_signals = parse_bool(v)?;
        }
        
        if let Some(v) = params.get("CoinsWhiteList") {
            config.coins_white_list = v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Some(v) = params.get("CoinsBlackList") {
            config.coins_black_list = v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Some(v) = params.get("NextDetectPenalty") {
            config.next_detect_penalty = v.parse()?;
        }
        
        if let Some(v) = params.get("CustomEMA") {
            config.custom_ema = v.to_string();
        }
        
        // Парсим числовые параметры
        parse_f64_param(&mut config.penalty_time, &params, "PenaltyTime");
        parse_f64_param(&mut config.trade_penalty_time, &params, "TradePenaltyTime");
        parse_f64_param(&mut config.binance_price_bug, &params, "BinancePriceBug");
        parse_f64_param(&mut config.min_volume, &params, "MinVolume");
        parse_f64_param(&mut config.max_volume, &params, "MaxVolume");
        parse_f64_param(&mut config.min_hourly_volume, &params, "MinHourlyVolume");
        parse_f64_param(&mut config.max_hourly_volume, &params, "MaxHourlyVolume");
        parse_f64_param(&mut config.delta_3h_max, &params, "Delta_3h_Max");
        parse_f64_param(&mut config.delta_24h_min, &params, "Delta_24h_Min");
        parse_f64_param(&mut config.delta_24h_max, &params, "Delta_24h_Max");
        parse_f64_param(&mut config.delta2_max, &params, "Delta2_Max");
        parse_f64_param(&mut config.delta_btc_min, &params, "Delta_BTC_Min");
        parse_f64_param(&mut config.delta_btc_max, &params, "Delta_BTC_Max");
        parse_f64_param(&mut config.delta_btc_24_min, &params, "Delta_BTC_24_Min");
        parse_f64_param(&mut config.delta_btc_24_max, &params, "Delta_BTC_24_Max");
        parse_f64_param(&mut config.delta_btc_5m_max, &params, "Delta_BTC_5m_Max");
        parse_f64_param(&mut config.delta_market_min, &params, "Delta_Market_Min");
        parse_f64_param(&mut config.delta_market_max, &params, "Delta_Market_Max");
        parse_f64_param(&mut config.delta_market_24_min, &params, "Delta_Market_24_Min");
        parse_f64_param(&mut config.delta_market_24_max, &params, "Delta_Market_24_Max");
        parse_f64_param(&mut config.trigger_key, &params, "TriggerKey");
        parse_f64_param(&mut config.trigger_seconds, &params, "TriggerSeconds");
        parse_f64_param(&mut config.max_active_orders, &params, "MaxActiveOrders");
        parse_f64_param(&mut config.max_markets, &params, "MaxMarkets");
        parse_f64_param(&mut config.auto_cancel_buy, &params, "AutoCancelBuy");
        parse_f64_param(&mut config.order_size, &params, "OrderSize");
        parse_f64_param(&mut config.buy_price, &params, "buyPrice");
        parse_f64_param(&mut config.use_30_sec_old_ask, &params, "Use30SecOldASK");
        parse_f64_param(&mut config.buy_modifier, &params, "BuyModifier");
        parse_f64_param(&mut config.add_price_bug, &params, "AddPriceBug");
        parse_f64_param(&mut config.buy_price_step, &params, "BuyPriceStep");
        parse_f64_param(&mut config.order_size_step, &params, "OrderSizeStep");
        parse_f64_param(&mut config.sell_price, &params, "SellPrice");
        parse_f64_param(&mut config.price_down_delay, &params, "PriceDownDelay");
        parse_f64_param(&mut config.price_down_percent, &params, "PriceDownPercent");
        parse_f64_param(&mut config.price_down_allowed_drop, &params, "PriceDownAllowedDrop");
        parse_f64_param(&mut config.use_scalping_mode, &params, "UseScalpingMode");
        parse_f64_param(&mut config.sell_ema_check_enter, &params, "SellEMACheckEnter");
        parse_f64_param(&mut config.sell_level_time, &params, "SellLevelTime");
        parse_f64_param(&mut config.sell_level_count, &params, "SellLevelCount");
        parse_f64_param(&mut config.sell_level_adjust, &params, "SellLevelAdjust");
        parse_f64_param(&mut config.sell_level_allowed_drop, &params, "SellLevelAllowedDrop");
        parse_f64_param(&mut config.use_stop_loss, &params, "UseStopLoss");
        parse_f64_param(&mut config.stop_loss_delay, &params, "StopLossDelay");
        parse_f64_param(&mut config.stop_loss, &params, "StopLoss");
        parse_f64_param(&mut config.stop_loss_spread, &params, "StopLossSpread");
        parse_f64_param(&mut config.allowed_drop, &params, "AllowedDrop");
        parse_f64_param(&mut config.second_stop_loss, &params, "SecondStopLoss");
        parse_f64_param(&mut config.stop_loss3, &params, "StopLoss3");
        parse_f64_param(&mut config.allowed_drop3, &params, "AllowedDrop3");
        parse_f64_param(&mut config.use_trailing, &params, "UseTrailing");
        parse_f64_param(&mut config.trailing_percent, &params, "TrailingPercent");
        parse_f64_param(&mut config.use_take_profit, &params, "UseTakeProfit");
        parse_f64_param(&mut config.take_profit, &params, "TakeProfit");
        parse_f64_param(&mut config.strategy_penalty, &params, "StrategyPenalty");
        
        // Парсим MoonBot стратегии параметры (MShot, MStrike, Hook)
        // Эти параметры парсятся независимо от типа стратегии
        self::parse_moon_strategy_params(&mut config, &params);
        
        Ok(config)
    }
    
    /// Создает конфигурацию стратегии из текста для MoonBot стратегий
    pub fn from_moon_strategy(strategy_name: &str, config_text: &str) -> anyhow::Result<StrategyConfig> {
        let mut config = StrategyConfig::parse(config_text)?;
        config.strategy_name = strategy_name.to_string();
        Ok(config)
    }
}

/// Парсинг параметров MoonBot стратегий (MShot, MStrike, Hook, Spread)
fn parse_moon_strategy_params(config: &mut StrategyConfig, params: &HashMap<String, String>) {
    // MShot параметры (если есть в конфиге, будут сохранены для будущего использования)
    // Пока просто парсим, но не сохраняем в структуру (нужно расширить StrategyConfig)
    
    // MStrike параметры
    if let Some(_) = params.get("MStrikeDepth") {
        // TODO: Сохранить в расширенную структуру или отдельную конфигурацию
    }
    
    // Hook параметры
    if let Some(_time_frame) = params.get("HookTimeFrame") {
        // TODO: Парсить HookTimeFrame
    }
}

impl StrategyConfig {
    /// Форматирует конфигурацию обратно в текстовый формат
    pub fn to_string(&self) -> String {
        let mut result = String::from("##Begin_Strategy\n");
        
        result.push_str(&format!("   Active={}\n", if self.active { 1 } else { 0 }));
        result.push_str(&format!("   FVersion={}\n", self.f_version));
        result.push_str(&format!("  StrategyName={}\n", self.strategy_name));
        result.push_str(&format!("  LastEditDate={}\n", self.last_edit_date));
        result.push_str(&format!("  SignalType={}\n", self.signal_type));
        result.push_str(&format!("  SilentNoCharts={}\n", bool_to_yes_no(self.silent_no_charts)));
        result.push_str(&format!("  ReportTradesToTelegram={}\n", bool_to_yes_no(self.report_trades_to_telegram)));
        result.push_str(&format!("  SoundKind={}\n", self.sound_kind));
        result.push_str(&format!("  IndependentSignals={}\n", bool_to_yes_no(self.independent_signals)));
        result.push_str(&format!("  CoinsWhiteList={}\n", self.coins_white_list.join(",")));
        result.push_str(&format!("  CoinsBlackList={}\n", self.coins_black_list.join(",")));
        result.push_str(&format!("  NextDetectPenalty={}\n", self.next_detect_penalty));
        result.push_str(&format!("  CustomEMA={}\n", self.custom_ema));
        result.push_str(&format!("  PenaltyTime={}\n", self.penalty_time));
        result.push_str(&format!("  TradePenaltyTime={}\n", self.trade_penalty_time));
        result.push_str(&format!("  BinancePriceBug={:.4}\n", self.binance_price_bug));
        result.push_str(&format!("  MinVolume={}\n", self.min_volume));
        result.push_str(&format!("  MaxVolume={:.0}M\n", self.max_volume / 1_000_000.0));
        result.push_str(&format!("  MinHourlyVolume={}\n", self.min_hourly_volume));
        result.push_str(&format!("  MaxHourlyVolume={:.0}M\n", self.max_hourly_volume / 1_000_000.0));
        result.push_str(&format!("  Delta_3h_Max={:.3}\n", self.delta_3h_max));
        result.push_str(&format!("  Delta_24h_Min={:.4}\n", self.delta_24h_min));
        result.push_str(&format!("  Delta_24h_Max={:.3}\n", self.delta_24h_max));
        result.push_str(&format!("  Delta2_Max={:.1}\n", self.delta2_max));
        result.push_str(&format!("  Delta_BTC_Min={:.2}\n", self.delta_btc_min));
        result.push_str(&format!("  Delta_BTC_Max={:.2}\n", self.delta_btc_max));
        result.push_str(&format!("  Delta_BTC_24_Min={:.2}\n", self.delta_btc_24_min));
        result.push_str(&format!("  Delta_BTC_24_Max={:.3}\n", self.delta_btc_24_max));
        result.push_str(&format!("  Delta_BTC_5m_Max={:.2}\n", self.delta_btc_5m_max));
        result.push_str(&format!("  Delta_Market_Min={:.2}\n", self.delta_market_min));
        result.push_str(&format!("  Delta_Market_Max={:.1}\n", self.delta_market_max));
        result.push_str(&format!("  Delta_Market_24_Min={:.1}\n", self.delta_market_24_min));
        result.push_str(&format!("  Delta_Market_24_Max={:.1}\n", self.delta_market_24_max));
        result.push_str(&format!("  TriggerKey={}\n", self.trigger_key));
        result.push_str(&format!("  TriggerSeconds={}\n", self.trigger_seconds));
        result.push_str(&format!("  MaxActiveOrders={}\n", self.max_active_orders));
        result.push_str(&format!("  MaxMarkets={}\n", self.max_markets));
        result.push_str(&format!("  AutoCancelBuy={:.1}\n", self.auto_cancel_buy));
        result.push_str(&format!("  OrderSize={:.3}\n", self.order_size));
        result.push_str(&format!("  buyPrice={}\n", self.buy_price));
        result.push_str(&format!("  Use30SecOldASK={}\n", bool_to_yes_no(self.use_30_sec_old_ask)));
        result.push_str(&format!("  BuyModifier={:.4}\n", self.buy_modifier));
        result.push_str(&format!("  AddPriceBug={:.4}\n", self.add_price_bug));
        result.push_str(&format!("  BuyPriceStep={:.4}\n", self.buy_price_step));
        result.push_str(&format!("  OrderSizeStep={:.4}\n", self.order_size_step));
        result.push_str(&format!("  SellPrice={:.3}\n", self.sell_price));
        result.push_str(&format!("  PriceDownDelay={}\n", self.price_down_delay));
        result.push_str(&format!("  PriceDownPercent={}\n", self.price_down_percent));
        result.push_str(&format!("  PriceDownAllowedDrop={:.4}\n", self.price_down_allowed_drop));
        result.push_str(&format!("  UseScalpingMode={}\n", bool_to_yes_no(self.use_scalping_mode)));
        result.push_str(&format!("  SellEMACheckEnter={}\n", bool_to_yes_no(self.sell_ema_check_enter)));
        result.push_str(&format!("  SellLevelTime={}\n", self.sell_level_time));
        result.push_str(&format!("  SellLevelCount={}\n", self.sell_level_count));
        result.push_str(&format!("  SellLevelAdjust={}\n", self.sell_level_adjust));
        result.push_str(&format!("  SellLevelAllowedDrop={}\n", self.sell_level_allowed_drop));
        result.push_str(&format!("  UseStopLoss={}\n", bool_to_yes_no(self.use_stop_loss)));
        result.push_str(&format!("  StopLossDelay={}\n", self.stop_loss_delay));
        result.push_str(&format!("  StopLoss={:.4}\n", self.stop_loss));
        result.push_str(&format!("  StopLossSpread={:.4}\n", self.stop_loss_spread));
        result.push_str(&format!("  AllowedDrop={:.2}\n", self.allowed_drop));
        result.push_str(&format!("  SecondStopLoss={:.4}\n", self.second_stop_loss));
        result.push_str(&format!("  StopLoss3={}\n", self.stop_loss3));
        result.push_str(&format!("  AllowedDrop3={:.4}\n", self.allowed_drop3));
        result.push_str(&format!("  UseTrailing={}\n", bool_to_yes_no(self.use_trailing)));
        result.push_str(&format!("  TrailingPercent={:.4}\n", self.trailing_percent));
        result.push_str(&format!("  UseTakeProfit={}\n", bool_to_yes_no(self.use_take_profit)));
        result.push_str(&format!("  TakeProfit={:.4}\n", self.take_profit));
        result.push_str(&format!("  StrategyPenalty={}\n", self.strategy_penalty));
        
        result.push_str("##End_Strategy");
        result
    }
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.to_uppercase().as_str() {
        "YES" | "1" | "TRUE" | "ON" => Ok(true),
        "NO" | "0" | "FALSE" | "OFF" => Ok(false),
        _ => Err(anyhow::anyhow!("Invalid boolean value: {}", value)),
    }
}

fn bool_to_yes_no(value: bool) -> &'static str {
    if value { "YES" } else { "NO" }
}

fn parse_f64_param<T: std::str::FromStr>(
    field: &mut T,
    params: &HashMap<String, String>,
    key: &str,
) where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    if let Some(v) = params.get(key) {
        // Удаляем суффикс 'M' для миллионов
        let cleaned = v.replace('M', "").trim().to_string();
        if let Ok(parsed) = cleaned.parse::<f64>() {
            // Если был суффикс 'M', умножаем на миллион
            let multiplier = if v.contains('M') { 1_000_000.0 } else { 1.0 };
            if let Ok(val) = (parsed * multiplier).to_string().parse() {
                *field = val;
            }
        }
    }
}

// Для bool параметров
impl StrategyConfig {
    fn parse_bool_param(
        field: &mut bool,
        params: &HashMap<String, String>,
        key: &str,
    ) {
        if let Some(v) = params.get(key) {
            if let Ok(val) = parse_bool(v) {
                *field = val;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_strategy_config() {
        let config_text = r##"
##Begin_Strategy
   Active=1
   FVersion=12
  StrategyName=ema_80
  SignalType=EMA
  UseTrailing=YES
  TrailingPercent=-0.1000
  CoinsWhiteList=btc,eth,sol
##End_Strategy
"##;
        
        let config = StrategyConfig::parse(config_text).unwrap();
        assert_eq!(config.active, true);
        assert_eq!(config.strategy_name, "ema_80");
        assert_eq!(config.use_trailing, true);
        assert_eq!(config.coins_white_list.len(), 3);
    }
}


