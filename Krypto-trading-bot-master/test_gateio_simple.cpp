#include "ccapi_cpp/ccapi_session.h"
#include <iostream>
#include <thread>
#include <chrono>

namespace ccapi {
Logger* Logger::logger = nullptr;
}

using ::ccapi::Event;
using ::ccapi::EventHandler;
using ::ccapi::Request;
using ::ccapi::Session;
using ::ccapi::SessionConfigs;
using ::ccapi::SessionOptions;
using ::ccapi::toString;

class SimpleTestHandler : public EventHandler {
public:
    void processEvent(const Event& event, Session* sessionPtr) override {
        std::cout << "\n[СОБЫТИЕ] Тип: " << toString(event.getType()) << std::endl;
        
        if (event.getType() == Event::Type::RESPONSE) {
            std::cout << "✅ Получен ответ от Gate.io!" << std::endl;
            std::cout << "Correlation ID: " << event.getCorrelationIdList()[0] << std::endl;
            
            for (const auto& message : event.getMessageList()) {
                std::cout << "\n--- Данные ---" << std::endl;
                std::cout << message.toString() << std::endl;
                
                // Парсим баланс если это запрос баланса
                if (event.getCorrelationIdList()[0] == "GET_BALANCE") {
                    std::cout << "\n💰 БАЛАНС:" << std::endl;
                    for (const auto& element : message.getElementList()) {
                        const auto& map = element.getNameValueMap();
                        for (const auto& [key, value] : map) {
                            std::cout << "  " << key << ": " << value << std::endl;
                        }
                    }
                }
            }
        } else if (event.getType() == Event::Type::SUBSCRIPTION_DATA) {
            std::cout << "📊 Данные подписки получены!" << std::endl;
            for (const auto& message : event.getMessageList()) {
                std::cout << "Инструмент: " << message.getInstrument() << std::endl;
                std::cout << message.toString() << std::endl;
            }
        } else if (event.getType() == Event::Type::SUBSCRIPTION_STATUS) {
            std::cout << "📡 Статус подписки: " << toString(event) << std::endl;
        }
    }
};

int main() {
    std::cout << "═══════════════════════════════════════════════════════" << std::endl;
    std::cout << "  Тест подключения к Gate.io через CCAPI" << std::endl;
    std::cout << "═══════════════════════════════════════════════════════" << std::endl;
    std::cout << std::endl;

    // API ключи
    std::string apiKey = "ac78ffea0103fcb2d0c25ab89e5c3b34";
    std::string apiSecret = "9cfc0c897560614f4cbbc558c172af81a6c5d0ef6612623692646ccecdb97d6f";

    std::cout << "API Key: " << apiKey.substr(0, 12) << "..." << std::endl;
    std::cout << std::endl;

    SessionOptions sessionOptions;
    SessionConfigs sessionConfigs;
    
    // Устанавливаем API ключи
    sessionConfigs.setCredential({
        {CCAPI_GATEIO_API_KEY, apiKey},
        {CCAPI_GATEIO_API_SECRET, apiSecret}
    });

    SimpleTestHandler eventHandler;
    Session session(sessionOptions, sessionConfigs, &eventHandler);

    std::cout << "1️⃣ Тест: Получение тикера ETH_USDT (публичный запрос)" << std::endl;
    Request request1(Request::Operation::GET_BBOS, "gateio", "ETH_USDT");
    request1.setCorrelationId("GET_TICKER");
    session.sendRequest(request1);
    std::this_thread::sleep_for(std::chrono::seconds(2));

    std::cout << "\n2️⃣ Тест: Получение баланса через REST API" << std::endl;
    Request request2(Request::Operation::GET_ACCOUNT_BALANCES, "gateio");
    request2.setCorrelationId("GET_BALANCE");
    session.sendRequest(request2);
    std::this_thread::sleep_for(std::chrono::seconds(3));
    
    std::cout << "\n2b️⃣ Тест: Получение баланса через WebSocket (spot.balances)" << std::endl;
    Subscription balanceSubscription("gateio", "", "BALANCE_UPDATE");
    session.subscribe(balanceSubscription);
    std::this_thread::sleep_for(std::chrono::seconds(3));

    std::cout << "\n3️⃣ Тест: Подписка на тикер ETH_USDT (WebSocket)" << std::endl;
    Subscription subscription("gateio", "ETH_USDT", "MARKET_DATA");
    session.subscribe(subscription);
    std::cout << "Ожидание данных WebSocket (5 секунд)..." << std::endl;
    std::this_thread::sleep_for(std::chrono::seconds(5));

    session.stop();
    std::cout << "\n✅ Тестирование завершено" << std::endl;
    return EXIT_SUCCESS;
}

