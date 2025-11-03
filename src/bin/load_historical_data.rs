//! Скрипт для загрузки исторических данных через Gate.io API
//! Загружает данные о сделках (trades) для BTC, ETH, SOL за последние 180 дней
//! Сохраняет в PostgreSQL базу данных

#![cfg(all(feature = "database", feature = "gate_exec"))]

use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use rust_test::database::DatabaseRepository;
use rust_test::utils::logging;
use std::env;
use std::time::Duration as StdDuration;

const GATE_API_BASE: &str = "https://api.gateio.ws/api/v4";
const SYMBOLS: &[&str] = &["BTC_USDT", "ETH_USDT", "SOL_USDT"];
const DAYS_BACK: i64 = 180;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Инициализация логирования
    logging::init_logging();
    
    log::info!("🚀 Загрузка исторических данных через Gate.io API");
    
    // Проверка переменных окружения
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL должен быть установлен");
    
    // Поддерживаем оба варианта названий переменных
    let api_key = env::var("GATE_API_KEY")
        .or_else(|_| env::var("GATEIO_API_KEY"))
        .ok();
    let api_secret = env::var("GATE_API_SECRET")
        .or_else(|_| env::var("GATEIO_SECRET_KEY"))
        .ok();
    
    if api_key.is_none() || api_secret.is_none() {
        log::warn!("⚠️  GATE_API_KEY и GATE_API_SECRET не установлены");
        log::warn!("   Будут использованы публичные endpoints (без аутентификации)");
        log::warn!("   Для полного доступа к данным установите API ключи");
    }
    
    // Подключение к БД
    log::info!("📊 Подключение к PostgreSQL...");
    let pool = DatabaseRepository::create_pool(&database_url).await?;
    let repo = DatabaseRepository::new(pool);
    log::info!("✅ Подключено к базе данных");
    
    // Загружаем данные для каждого символа
    for symbol in SYMBOLS {
        log::info!("");
        log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        log::info!("📈 Загрузка данных для {}", symbol);
        log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        match load_symbol_data(symbol, &repo).await {
            Ok(count) => {
                log::info!("✅ Загружено {} тиков для {}", count, symbol);
            }
            Err(e) => {
                log::error!("❌ Ошибка загрузки {}: {}", symbol, e);
            }
        }
        
        // Небольшая задержка между запросами
        tokio::time::sleep(StdDuration::from_secs(1)).await;
    }
    
    log::info!("");
    log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    log::info!("✅ Загрузка данных завершена!");
    log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    Ok(())
}

async fn load_symbol_data(
    symbol: &str,
    repo: &DatabaseRepository,
) -> anyhow::Result<usize> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()?;
    
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(DAYS_BACK);
    
    log::info!("   Период: {} - {}", start_time.format("%Y-%m-%d"), end_time.format("%Y-%m-%d"));
    
    // Проверяем сколько данных уже есть в БД
    let existing = repo.query_ticks(&rust_test::database::TickQuery {
        symbol: symbol.to_string(),
        start_time: Some(start_time),
        end_time: Some(end_time),
        limit: Some(1),
        exchange: None,
    }).await?;
    
    if !existing.is_empty() {
        log::info!("   ℹ️  Данные уже есть в БД, проверяем полноту...");
        // Можно добавить логику проверки полноты данных
    }
    
    // Gate.io формат: BTC_USDT -> BTC_USDT для API
    let gate_symbol = symbol;
    
    // Загружаем данные по частям (по дням)
    let mut total_count = 0;
    let mut current_start = start_time;
    
    while current_start < end_time {
        let current_end = (current_start + Duration::days(1)).min(end_time);
        
        match fetch_trades_batch(&client, gate_symbol, current_start, current_end).await {
            Ok(trades) => {
                if trades.is_empty() {
                    log::debug!("   Пропуск: нет данных за {}", current_start.format("%Y-%m-%d"));
                } else {
                    let count = save_trades_to_db(&repo, symbol, &trades).await?;
                    total_count += count;
                    log::info!("   ✅ {}: загружено {} тиков", current_start.format("%Y-%m-%d"), count);
                }
            }
            Err(e) => {
                log::warn!("   ⚠️  Ошибка за {}: {}", current_start.format("%Y-%m-%d"), e);
            }
        }
        
        current_start = current_end;
        
        // Задержка между запросами (rate limit)
        tokio::time::sleep(StdDuration::from_millis(200)).await;
    }
    
    Ok(total_count)
}

async fn fetch_trades_batch(
    client: &reqwest::Client,
    symbol: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> anyhow::Result<Vec<GateTrade>> {
    let url = format!(
        "{}/spot/trades?currency_pair={}&from={}&to={}&limit=1000",
        GATE_API_BASE,
        symbol,
        start.timestamp(),
        end.timestamp()
    );
    
    log::debug!("   Запрос: {}", url);
    
    let response = client
        .get(&url)
        .send()
        .await?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP {}: {}", response.status(), response.text().await?);
    }
    
    let trades: Vec<GateTrade> = response.json().await?;
    Ok(trades)
}

#[derive(serde::Deserialize, Debug)]
struct GateTrade {
    id: String,
    create_time: String,
    create_time_ms: String,
    side: String,
    amount: String,
    price: String,
}

async fn save_trades_to_db(
    repo: &DatabaseRepository,
    symbol: &str,
    trades: &[GateTrade],
) -> anyhow::Result<usize> {
    let mut saved = 0;
    
    for trade in trades {
        // Парсим timestamp
        let timestamp_ms = trade.create_time_ms.parse::<i64>()?;
        let timestamp = DateTime::from_timestamp(timestamp_ms / 1000, 0)
            .unwrap_or_else(|| Utc::now());
        
        // Парсим цену и количество
        let price = Decimal::try_from(trade.price.parse::<f64>()?)?;
        let quantity = Decimal::try_from(trade.amount.parse::<f64>()?)?;
        
        let tick_data = rust_test::database::types::TickData {
            timestamp,
            symbol: symbol.to_string(),
            price,
            quantity,
            side: trade.side.clone(),
            trade_id: trade.id.clone(),
            is_buyer_maker: trade.side == "sell", // sell = maker sells (buyer is maker)
            exchange: "gate.io".to_string(),
        };
        
        // Сохраняем в БД (игнорируем дубликаты)
        if let Err(e) = repo.insert_tick(&tick_data).await {
            // Игнорируем ошибки дубликатов
            if !e.to_string().contains("duplicate") {
                log::warn!("   Ошибка сохранения тика {}: {}", trade.id, e);
            }
        } else {
            saved += 1;
        }
    }
    
    Ok(saved)
}

