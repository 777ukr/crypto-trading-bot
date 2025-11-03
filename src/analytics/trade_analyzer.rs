//! Анализатор торговли за период (3 суток BTCUSDT)
//! Оценка успешности и выявление паттернов

use std::collections::HashMap;
use crate::analytics::performance::PerformanceMetrics;

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub timestamp: u64,
    pub entry_time: u64,
    pub entry_price: f64,
    pub exit_time: u64,
    pub exit_price: f64,
    pub side: String,
    pub size: f64,
    pub pnl: Option<f64>,
}

pub struct TradeAnalyzer;

impl TradeAnalyzer {
    /// Анализ торговли за период
    pub fn analyze_period(trades: &[TradeRecord], period_days: u32) -> PeriodAnalysis {
        let metrics = PerformanceMetrics::calculate(trades);
        
        // Анализ по времени суток
        let hourly_distribution = Self::analyze_hourly_distribution(trades);
        
        // Анализ по дням
        let daily_performance = Self::analyze_daily_performance(trades, period_days);
        
        // Оценка стратегии
        let strategy_score = Self::evaluate_strategy_score(&metrics);
        
        // Генерируем рекомендации до move
        let recommendations = Self::generate_recommendations(&metrics);
        
        PeriodAnalysis {
            period_days,
            metrics,
            hourly_distribution,
            daily_performance,
            strategy_score,
            recommendations,
        }
    }

    pub fn calculate_pnl(trade: &TradeRecord) -> f64 {
        if trade.side == "long" {
            (trade.exit_price - trade.entry_price) * trade.size
        } else {
            (trade.entry_price - trade.exit_price) * trade.size
        }
    }

    fn analyze_hourly_distribution(trades: &[TradeRecord]) -> HashMap<u32, HourStats> {
        let mut hourly: HashMap<u32, Vec<&TradeRecord>> = HashMap::new();
        
        for trade in trades {
            let hour = (trade.entry_time % 86400) / 3600;
            hourly.entry(hour as u32).or_insert_with(Vec::new).push(trade);
        }
        
        hourly.iter()
            .map(|(hour, trades)| {
                let wins = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count();
                let total_pnl: f64 = trades.iter().map(|t| t.pnl.unwrap_or(0.0)).sum();
                
                (*hour, HourStats {
                    trades: trades.len(),
                    wins,
                    pnl: total_pnl,
                })
            })
            .collect()
    }

    fn analyze_daily_performance(trades: &[TradeRecord], days: u32) -> Vec<DayPerformance> {
        let mut daily: HashMap<u32, Vec<&TradeRecord>> = HashMap::new();
        
        for trade in trades {
            let day = trade.entry_time / 86400;
            daily.entry(day as u32).or_insert_with(Vec::new).push(trade);
        }
        
        daily.iter()
            .map(|(day, trades)| {
                let wins = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count();
                let total_pnl: f64 = trades.iter().map(|t| t.pnl.unwrap_or(0.0)).sum();
                
                DayPerformance {
                    day: *day,
                    trades: trades.len(),
                    wins,
                    pnl: total_pnl,
                    win_rate: if !trades.is_empty() {
                        wins as f64 / trades.len() as f64 * 100.0
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }

    fn evaluate_strategy_score(metrics: &PerformanceMetrics) -> f64 {
        let mut score = 0.0;
        
        // Win rate component (40%)
        score += (metrics.win_rate / 100.0) * 0.4;
        
        // Profit factor component (30%)
        let pf_score = metrics.profit_factor.min(5.0) / 5.0;
        score += pf_score * 0.3;
        
        // Sharpe ratio component (20%)
        let sharpe_score = metrics.sharpe_ratio.min(3.0) / 3.0;
        score += sharpe_score * 0.2;
        
        // Drawdown penalty (10%)
        let dd_penalty = 1.0 - (metrics.max_drawdown.min(50.0) / 50.0);
        score += dd_penalty * 0.1;
        
        score * 100.0
    }

    fn generate_recommendations(metrics: &PerformanceMetrics) -> Vec<String> {
        let mut recs = Vec::new();
        
        if metrics.win_rate < 50.0 {
            recs.push("⚠️ Низкий win rate - рассмотрите улучшение точек входа".to_string());
        }
        
        if metrics.profit_factor < 1.5 {
            recs.push("⚠️ Profit factor ниже 1.5 - риск/награда неоптимальна".to_string());
        }
        
        if metrics.max_drawdown > 20.0 {
            recs.push("⚠️ Большой drawdown - усильте управление рисками".to_string());
        }
        
        if metrics.sharpe_ratio < 1.0 {
            recs.push("⚠️ Низкий Sharpe ratio - волатильность результатов высока".to_string());
        }
        
        if recs.is_empty() {
            recs.push("✅ Стратегия показывает хорошие результаты".to_string());
        }
        
        recs
    }
}

#[derive(Debug)]
pub struct PeriodAnalysis {
    pub period_days: u32,
    pub metrics: PerformanceMetrics,
    pub hourly_distribution: HashMap<u32, HourStats>,
    pub daily_performance: Vec<DayPerformance>,
    pub strategy_score: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug)]
pub struct HourStats {
    pub trades: usize,
    pub wins: usize,
    pub pnl: f64,
}

#[derive(Debug)]
pub struct DayPerformance {
    pub day: u32,
    pub trades: usize,
    pub wins: usize,
    pub pnl: f64,
    pub win_rate: f64,
}

impl PeriodAnalysis {
    pub fn print(&self) {
        println!("\n📈 Period Analysis ({} days):", self.period_days);
        self.metrics.print();
        
        println!("\n🎯 Strategy Score: {:.1}/100", self.strategy_score);
        
        println!("\n💡 Recommendations:");
        for rec in &self.recommendations {
            println!("  {}", rec);
        }
    }
}

