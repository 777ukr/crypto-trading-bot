#include "ccapi_cpp/ccapi_session.h"
#include <iostream>
#include <map>
#include <string>
#include <vector>
#include <cmath>
#include <chrono>
#include <iomanip>
#include <mutex>
#include <sstream>

namespace ccapi {
Logger* Logger::logger = nullptr;
}

using ::ccapi::Event;
using ::ccapi::EventHandler;
using ::ccapi::Request;
using ::ccapi::Session;
using ::ccapi::SessionConfigs;
using ::ccapi::SessionOptions;
using ::ccapi::Subscription;
using ::ccapi::toString;

struct PairData {
    double currentPrice = 0.0;
    double maxPrice = 0.0;
    std::chrono::system_clock::time_point maxPriceTime;
    std::chrono::system_clock::time_point lastUpdate;
    bool hasData = false;
    int updateCount = 0;
};

class AllPairsDipMonitor : public EventHandler {
public:
    AllPairsDipMonitor(double dipThreshold = 20.0) 
        : dipThreshold_(dipThreshold), 
          startTime_(std::chrono::system_clock::now()) {}

    void processEvent(const Event& event, Session* sessionPtr) override {
        if (event.getType() == Event::Type::SUBSCRIPTION_STATUS) {
            std::lock_guard<std::mutex> lock(mutex_);
            std::cout << "[" << getCurrentTime() << "] Subscription: " 
                      << event.getCorrelationIdList()[0] << std::endl;
        } else if (event.getType() == Event::Type::SUBSCRIPTION_DATA) {
            for (const auto& message : event.getMessageList()) {
                processTickerMessage(message);
            }
        } else if (event.getType() == Event::Type::RESPONSE) {
            // Обработка ответа на запрос списка пар
            if (event.getCorrelationIdList()[0] == "GET_ALL_PAIRS") {
                processPairsList(event);
            }
        }
    }

    void setAllPairs(const std::vector<std::string>& pairs) {
        std::lock_guard<std::mutex> lock(mutex_);
        for (const auto& pair : pairs) {
            pairs_[pair] = PairData();
        }
        std::cout << "Загружено " << pairs.size() << " торговых пар" << std::endl;
    }

    void printStats() {
        std::lock_guard<std::mutex> lock(mutex_);
        int activePairs = 0;
        int pairsWithData = 0;
        for (const auto& [symbol, data] : pairs_) {
            if (data.hasData) {
                pairsWithData++;
                if (data.currentPrice > 0) {
                    activePairs++;
                }
            }
        }
        std::cout << "\n=== Статистика ===" << std::endl;
        std::cout << "Всего пар: " << pairs_.size() << std::endl;
        std::cout << "Пар с данными: " << pairsWithData << std::endl;
        std::cout << "Активных пар: " << activePairs << std::endl;
        std::cout << "Время работы: " << getUptime() << std::endl;
    }

private:
    std::map<std::string, PairData> pairs_;
    double dipThreshold_;
    std::mutex mutex_;
    std::chrono::system_clock::time_point startTime_;

    std::string getCurrentTime() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%H:%M:%S");
        return ss.str();
    }

    std::string getUptime() {
        auto now = std::chrono::system_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::seconds>(
            now - startTime_).count();
        int hours = duration / 3600;
        int minutes = (duration % 3600) / 60;
        int seconds = duration % 60;
        std::stringstream ss;
        ss << hours << "h " << minutes << "m " << seconds << "s";
        return ss.str();
    }

    void processTickerMessage(const Message& message) {
        std::string symbol = message.getInstrument();
        if (symbol.empty()) {
            return;
        }

        // Преобразуем формат если нужно (BTC-USDT -> BTC_USDT)
        std::replace(symbol.begin(), symbol.end(), '-', '_');

        double price = 0.0;
        
        // Извлекаем цену из сообщения
        for (const auto& element : message.getElementList()) {
            const auto& nameValueMap = element.getNameValueMap();
            
            // Пробуем разные поля
            std::vector<std::string> priceFields = {
                "LAST_PRICE", "MARKET_PRICE", "BID_PRICE", "ASK_PRICE",
                "p", "last", "close", "price"
            };
            
            for (const auto& field : priceFields) {
                auto it = nameValueMap.find(field);
                if (it != nameValueMap.end()) {
                    try {
                        price = std::stod(it->second);
                        if (price > 0) break;
                    } catch (...) {
                        continue;
                    }
                }
            }
        }

        if (price > 0.0) {
            updatePairData(symbol, price);
        }
    }

    void updatePairData(const std::string& symbol, double price) {
        std::lock_guard<std::mutex> lock(mutex_);
        
        auto& data = pairs_[symbol];
        data.currentPrice = price;
        data.lastUpdate = std::chrono::system_clock::now();
        data.updateCount++;

        if (!data.hasData) {
            data.maxPrice = price;
            data.maxPriceTime = std::chrono::system_clock::now();
            data.hasData = true;
            return;
        }

        // Обновляем максимум
        if (price > data.maxPrice) {
            data.maxPrice = price;
            data.maxPriceTime = std::chrono::system_clock::now();
        }

        // Проверяем просадку
        if (data.maxPrice > 0 && price < data.maxPrice) {
            double dipPercent = ((data.maxPrice - price) / data.maxPrice) * 100.0;
            
            if (dipPercent >= dipThreshold_) {
                // Вычисляем время с максимума
                auto timeSinceMax = std::chrono::duration_cast<std::chrono::seconds>(
                    std::chrono::system_clock::now() - data.maxPriceTime).count();
                
                std::cout << "\n🚨🚨🚨 АЛЕРТ: ПРОСАДКА " << dipPercent << "% 🚨🚨🚨" << std::endl;
                std::cout << "   Пара: " << symbol << std::endl;
                std::cout << "   Текущая: " << std::fixed << std::setprecision(8) << price << std::endl;
                std::cout << "   Максимум: " << std::fixed << std::setprecision(8) << data.maxPrice << std::endl;
                std::cout << "   Просадка: " << std::fixed << std::setprecision(2) << dipPercent << "%" << std::endl;
                std::cout << "   Время с максимума: " << timeSinceMax << " сек" << std::endl;
                std::cout << "   Обновлений: " << data.updateCount << std::endl;
                std::cout << "   Время: " << getCurrentTime() << std::endl;
                std::cout << std::endl;
            }
        }
    }

    void processPairsList(const Event& event) {
        // Обработка списка пар из REST API ответа
        // Это будет вызвано после запроса списка всех пар
    }
};

int main(int argc, char** argv) {
    std::cout << "╔════════════════════════════════════════════════════════╗" << std::endl;
    std::cout << "║   Gate.io Dip Monitor - Мониторинг всех спот-монет     ║" << std::endl;
    std::cout << "╚════════════════════════════════════════════════════════╝" << std::endl;
    std::cout << std::endl;

    double dipThreshold = 20.0;
    if (argc > 1) {
        try {
            dipThreshold = std::stod(argv[1]);
        } catch (...) {
            std::cerr << "Неверный порог, используем 20%" << std::endl;
        }
    }

    std::cout << "Порог просадки: " << dipThreshold << "%" << std::endl;
    std::cout << std::endl;

    SessionOptions sessionOptions;
    SessionConfigs sessionConfigs;
    
    AllPairsDipMonitor eventHandler(dipThreshold);
    Session session(sessionOptions, sessionConfigs, &eventHandler);

    // Шаг 1: Получаем список всех спот-пар через REST API
    std::cout << "Получение списка всех спот-пар..." << std::endl;
    Request request(Request::Operation::GENERIC_PUBLIC_REQUEST, "gateio", "", "GET_ALL_PAIRS");
    request.appendParam({
        {"url", "/api/v4/spot/currency_pairs"},
        {"method", "GET"}
    });
    request.setCorrelationId("GET_ALL_PAIRS");
    session.sendRequest(request);

    // Ждем немного для получения списка
    std::this_thread::sleep_for(std::chrono::seconds(2));

    // Шаг 2: Подписываемся на тикеры всех пар
    // Для начала используем популярные пары
    std::vector<std::string> allPairs = {
        "BTC_USDT", "ETH_USDT", "BNB_USDT", "SOL_USDT", "XRP_USDT", "ADA_USDT",
        "DOGE_USDT", "DOT_USDT", "MATIC_USDT", "AVAX_USDT", "LINK_USDT",
        "UNI_USDT", "LTC_USDT", "ATOM_USDT", "ETC_USDT", "XLM_USDT", "FIL_USDT",
        "TRX_USDT", "EOS_USDT", "AAVE_USDT", "ALGO_USDT", "VET_USDT", "ICP_USDT",
        "THETA_USDT", "FTM_USDT", "HBAR_USDT", "EGLD_USDT", "NEAR_USDT",
        "AXS_USDT", "SAND_USDT", "MANA_USDT", "GALA_USDT", "CHZ_USDT"
    };

    eventHandler.setAllPairs(allPairs);

    std::cout << "Подписка на тикеры " << allPairs.size() << " пар..." << std::endl;
    
    for (const auto& pair : allPairs) {
        Subscription subscription("gateio", pair, "MARKET_DATA");
        session.subscribe(subscription);
    }

    std::cout << "Мониторинг запущен!" << std::endl;
    std::cout << "Ожидание данных и поиск просадок..." << std::endl;
    std::cout << std::endl;

    // Периодический вывод статистики
    auto statsThread = std::thread([&eventHandler]() {
        while (true) {
            std::this_thread::sleep_for(std::chrono::minutes(5));
            eventHandler.printStats();
        }
    });

    // Основной цикл
    std::this_thread::sleep_for(std::chrono::hours(24));
    
    statsThread.detach();
    session.stop();
    std::cout << "\nМониторинг остановлен." << std::endl;
    return EXIT_SUCCESS;
}

