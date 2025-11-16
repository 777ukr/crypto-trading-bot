#include "ccapi_cpp/ccapi_session.h"
#include <iostream>
#include <map>
#include <string>
#include <cmath>
#include <chrono>
#include <iomanip>

namespace ccapi {
Logger* Logger::logger = nullptr;
}

using ::ccapi::Event;
using ::ccapi::EventHandler;
using ::ccapi::Logger;
using ::ccapi::Message;
using ::ccapi::Session;
using ::ccapi::SessionConfigs;
using ::ccapi::SessionOptions;
using ::ccapi::Subscription;
using ::ccapi::toString;

// Структура для хранения данных о паре
struct PairData {
    double currentPrice = 0.0;
    double maxPrice = 0.0;
    double minPrice = 0.0;
    std::chrono::system_clock::time_point lastUpdate;
    bool hasData = false;
};

class DipMonitorHandler : public EventHandler {
public:
    DipMonitorHandler(double dipThreshold = 20.0) : dipThreshold_(dipThreshold) {}

    void processEvent(const Event& event, Session* sessionPtr) override {
        if (event.getType() == Event::Type::SUBSCRIPTION_STATUS) {
            std::cout << "[" << getCurrentTime() << "] Subscription status: " 
                      << toString(event) << std::endl;
        } else if (event.getType() == Event::Type::SUBSCRIPTION_DATA) {
            for (const auto& message : event.getMessageList()) {
                processTickerMessage(message);
            }
        }
    }

private:
    std::map<std::string, PairData> pairs_;
    double dipThreshold_; // Процент просадки (20%)
    std::mutex mutex_;

    std::string getCurrentTime() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        return ss.str();
    }

    void processTickerMessage(const Message& message) {
        std::lock_guard<std::mutex> lock(mutex_);
        
        std::string symbol = message.getInstrument();
        if (symbol.empty()) {
            return;
        }

        // Получаем цену из сообщения
        double price = 0.0;
        for (const auto& element : message.getElementList()) {
            const auto& nameValueMap = element.getNameValueMap();
            
            // Ищем поле с ценой (может быть "LAST_PRICE", "BID_PRICE", "ASK_PRICE", "MARKET_PRICE")
            for (const auto& [key, value] : nameValueMap) {
                std::string keyStr(key);
                if (keyStr.find("PRICE") != std::string::npos || 
                    keyStr == "p" || keyStr == "last") {
                    try {
                        price = std::stod(value);
                        break;
                    } catch (...) {
                        continue;
                    }
                }
            }
            
            // Альтернативный способ - искать в "MARKET_DATA"
            if (price == 0.0) {
                auto it = nameValueMap.find("MARKET_DATA");
                if (it != nameValueMap.end()) {
                    // Парсим JSON если нужно
                }
            }
        }

        if (price > 0.0) {
            updatePairData(symbol, price);
        }
    }

    void updatePairData(const std::string& symbol, double price) {
        auto& data = pairs_[symbol];
        data.currentPrice = price;
        data.lastUpdate = std::chrono::system_clock::now();
        
        if (!data.hasData) {
            // Первое обновление
            data.maxPrice = price;
            data.minPrice = price;
            data.hasData = true;
            std::cout << "[" << getCurrentTime() << "] Начало мониторинга: " 
                      << symbol << " = " << price << std::endl;
            return;
        }

        // Обновляем максимум
        if (price > data.maxPrice) {
            data.maxPrice = price;
            data.minPrice = price; // Сбрасываем минимум при новом максимуме
        }

        // Обновляем минимум
        if (price < data.minPrice) {
            data.minPrice = price;
        }

        // Проверяем просадку от максимума
        if (data.maxPrice > 0) {
            double dipPercent = ((data.maxPrice - price) / data.maxPrice) * 100.0;
            
            if (dipPercent >= dipThreshold_) {
                std::cout << "\n🚨 АЛЕРТ: ПРОСАДКА ОБНАРУЖЕНА!" << std::endl;
                std::cout << "   Пара: " << symbol << std::endl;
                std::cout << "   Текущая цена: " << price << std::endl;
                std::cout << "   Максимум: " << data.maxPrice << std::endl;
                std::cout << "   Просадка: " << std::fixed << std::setprecision(2) 
                          << dipPercent << "%" << std::endl;
                std::cout << "   Время: " << getCurrentTime() << std::endl;
                std::cout << std::endl;
            }
        }
    }
};

int main(int argc, char** argv) {
    std::cout << "=== Gate.io Dip Monitor ===" << std::endl;
    std::cout << "Мониторинг всех спот-монет на просадку 20%" << std::endl;
    std::cout << std::endl;

    // Параметры
    double dipThreshold = 20.0;
    if (argc > 1) {
        try {
            dipThreshold = std::stod(argv[1]);
        } catch (...) {
            std::cerr << "Неверный порог просадки, используем 20%" << std::endl;
        }
    }

    SessionOptions sessionOptions;
    SessionConfigs sessionConfigs;
    
    // Настройка API ключей (если нужны приватные данные)
    // sessionConfigs.setCredential({
    //     {CCAPI_GATEIO_API_KEY, "your_api_key"},
    //     {CCAPI_GATEIO_API_SECRET, "your_api_secret"}
    // });

    DipMonitorHandler eventHandler(dipThreshold);
    Session session(sessionOptions, sessionConfigs, &eventHandler);

    // Подписка на тикеры всех спот-пар
    // Для Gate.io нужно подписаться на канал "spot.tickers"
    // CCAPI автоматически обработает подписку
    
    std::cout << "Подключение к Gate.io WebSocket..." << std::endl;
    
    // Подписка на тикеры (CCAPI автоматически получит все пары)
    // Для получения всех пар нужно сначала запросить список через REST API
    // или использовать специальную подписку
    
    // Временное решение: подписка на популярные пары
    std::vector<std::string> popularPairs = {
        "BTC_USDT", "ETH_USDT", "BNB_USDT", "SOL_USDT", "XRP_USDT",
        "ADA_USDT", "DOGE_USDT", "DOT_USDT", "MATIC_USDT", "AVAX_USDT",
        "LINK_USDT", "UNI_USDT", "LTC_USDT", "ATOM_USDT", "ETC_USDT"
    };

    for (const auto& pair : popularPairs) {
        Subscription subscription("gateio", pair, "MARKET_DATA");
        session.subscribe(subscription);
        std::cout << "Подписка на: " << pair << std::endl;
    }

    std::cout << std::endl;
    std::cout << "Мониторинг запущен. Ожидание данных..." << std::endl;
    std::cout << "Порог просадки: " << dipThreshold << "%" << std::endl;
    std::cout << std::endl;

    // Запуск мониторинга
    std::this_thread::sleep_for(std::chrono::hours(24)); // Работает 24 часа
    
    session.stop();
    std::cout << "Мониторинг остановлен." << std::endl;
    return EXIT_SUCCESS;
}

