//! Метрики бэктеста

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct BacktestMetrics {
    pub total_pnl: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub max_drawdown: f64,
    pub max_profit: f64,
    pub equity_curve: Vec<(DateTime<Utc>, f64)>,
    pub trades: Vec<TradeRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub symbol: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size: f64,
    pub is_buy: bool,
    pub pnl: f64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRating {
    pub profitability_score: f64,  // 0-10
    pub stability_score: f64,      // 0-10
    pub risk_score: f64,            // 0-10 (обратный, чем больше риск - меньше score)
    pub fill_rate_score: f64,       // 0-10
    pub overall_rating: f64,        // Средневзвешенное 0-10
    pub stars: u8,                  // 0-5 звезд
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub total_pnl: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub average_profit: f64,
    pub average_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub fill_rate: f64,             // Процент исполненных ордеров
    pub avg_trade_duration_ms: f64, // Средняя длительность сделки
    pub rating: StrategyRating,     // Рейтинг стратегии
}

impl BacktestMetrics {
    fn calculate_rating(
        &self,
        profit_factor: f64,
        win_rate: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
        fill_rate: f64,
    ) -> StrategyRating {
        // Profitability Score (0-10)
        // Комбинация P&L, profit factor и win rate
        let pnl_score = ((self.total_pnl / 1000.0).min(10.0f64)).max(0.0f64); // 10 баллов за 10k прибыли
        let pf_score = ((profit_factor.min(5.0f64) / 5.0f64 * 10.0f64)).max(0.0f64); // 10 баллов за PF >= 5
        let wr_score = ((win_rate / 100.0f64 * 10.0f64)).max(0.0f64); // 10 баллов за 100% win rate
        let profitability_score = ((pnl_score * 0.4f64 + pf_score * 0.3f64 + wr_score * 0.3f64).min(10.0f64));
        
        // Stability Score (0-10)
        // Чем выше Sharpe ratio, тем стабильнее
        let sharpe_score = ((sharpe_ratio.min(3.0f64) / 3.0f64 * 10.0f64)).max(0.0f64); // 10 баллов за Sharpe >= 3
        let stability_score = sharpe_score.min(10.0f64);
        
        // Risk Score (0-10) - обратный, меньше drawdown = больше score
        let dd_score = if max_drawdown > 0.0 {
            (1.0f64 - (max_drawdown / 100.0f64).min(1.0f64)) * 10.0f64
        } else {
            10.0
        };
        let risk_score = dd_score.max(0.0f64).min(10.0f64);
        
        // Fill Rate Score (0-10)
        let fill_rate_score = ((fill_rate / 100.0f64 * 10.0f64)).max(0.0f64).min(10.0f64);
        
        // Overall Rating (средневзвешенное)
        let overall_rating = profitability_score * 0.35 +
            stability_score * 0.25 +
            risk_score * 0.25 +
            fill_rate_score * 0.15;
        
        // Stars (0-5)
        let stars = match overall_rating {
            r if r >= 9.0 => 5,
            r if r >= 8.0 => 4,
            r if r >= 7.0 => 3,
            r if r >= 6.0 => 2,
            r if r >= 5.0 => 1,
            _ => 0,
        };
        
        StrategyRating {
            profitability_score,
            stability_score,
            risk_score,
            fill_rate_score,
            overall_rating,
            stars,
        }
    }
}

impl BacktestMetrics {
    pub fn new() -> Self {
        Self {
            total_pnl: 0.0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            max_drawdown: 0.0,
            max_profit: 0.0,
            equity_curve: Vec::new(),
            trades: Vec::new(),
        }
    }
    
    pub fn record_trade(
        &mut self,
        symbol: String,
        entry_price: f64,
        exit_price: f64,
        size: f64,
        is_buy: bool,
        pnl: f64,
        timestamp: DateTime<Utc>,
    ) {
        let trade = TradeRecord {
            symbol,
            entry_price,
            exit_price,
            size,
            is_buy,
            pnl,
            entry_time: timestamp,
            exit_time: timestamp,
        };
        
        self.trades.push(trade);
        self.total_trades += 1;
        self.total_pnl += pnl;
        
        if pnl > 0.0 {
            self.winning_trades += 1;
        } else {
            self.losing_trades += 1;
        }
        
        // Обновляем equity curve
        self.equity_curve.push((timestamp, self.total_pnl));
        
        // Обновляем max drawdown
        if self.total_pnl > self.max_profit {
            self.max_profit = self.total_pnl;
        }
        
        let drawdown = self.max_profit - self.total_pnl;
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
        }
    }
    
    pub fn to_result(&self) -> BacktestResult {
        let win_rate = if self.total_trades > 0 {
            self.winning_trades as f64 / self.total_trades as f64 * 100.0
        } else {
            0.0
        };
        
        let (total_profit, total_loss) = self.trades.iter()
            .fold((0.0, 0.0), |(profit, loss), trade| {
                if trade.pnl > 0.0 {
                    (profit + trade.pnl, loss)
                } else {
                    (profit, loss + trade.pnl.abs())
                }
            });
        
        let profit_factor = if total_loss > 0.0 {
            total_profit / total_loss
        } else if total_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        
        let average_profit = if self.winning_trades > 0 {
            total_profit / self.winning_trades as f64
        } else {
            0.0
        };
        
        let average_loss = if self.losing_trades > 0 {
            total_loss / self.losing_trades as f64
        } else {
            0.0
        };
        
        let largest_win = self.trades.iter()
            .map(|t| t.pnl)
            .filter(|&p| p > 0.0)
            .fold(0.0, f64::max);
        
        let largest_loss = self.trades.iter()
            .map(|t| t.pnl)
            .filter(|&p| p < 0.0)
            .fold(0.0f64, |acc, p| acc.min(p));
        
        // Упрощенный Sharpe ratio (без risk-free rate)
        let sharpe_ratio = if self.trades.len() > 1 {
            let returns: Vec<f64> = self.trades.iter().map(|t| t.pnl).collect();
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / returns.len() as f64;
            let std_dev = variance.sqrt();
            
            if std_dev > 0.0 {
                mean / std_dev
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Вычисляем fill rate
        let total_order_attempts = self.total_trades + self.losing_trades; // Упрощенно
        let fill_rate = if total_order_attempts > 0 {
            self.total_trades as f64 / total_order_attempts as f64 * 100.0
        } else {
            0.0
        };
        
        // Средняя длительность сделок
        let avg_duration = if !self.trades.is_empty() {
            let durations: Vec<i64> = self.trades.iter()
                .map(|t| (t.exit_time - t.entry_time).num_milliseconds())
                .collect();
            durations.iter().sum::<i64>() as f64 / durations.len() as f64
        } else {
            0.0
        };
        
        // Вычисляем рейтинг
        let rating = self.calculate_rating(
            profit_factor,
            win_rate,
            sharpe_ratio,
            self.max_drawdown,
            fill_rate,
        );
        
        BacktestResult {
            total_pnl: self.total_pnl,
            total_trades: self.total_trades,
            winning_trades: self.winning_trades,
            losing_trades: self.losing_trades,
            win_rate,
            profit_factor,
            max_drawdown: self.max_drawdown,
            sharpe_ratio,
            average_profit,
            average_loss,
            largest_win,
            largest_loss,
            fill_rate,
            avg_trade_duration_ms: avg_duration,
            rating,
        }
    }
}

