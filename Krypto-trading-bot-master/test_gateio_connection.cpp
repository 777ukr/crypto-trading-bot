#include "ccapi_cpp/ccapi_session.h"
#include <iostream>
#include <iomanip>
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

class TestEventHandler : public EventHandler {
public:
    bool receivedResponse = false;
    bool hasError = false;
    std::string responseData;

    void processEvent(const Event& event, Session* sessionPtr) override {
        std::cout << "\n=== Получено событие ===" << std::endl;
        std::cout << "Тип: " << toString(event.getType()) << std::endl;
        
        if (event.getType() == Event::Type::RESPONSE) {
            receivedResponse = true;
            
            std::cout << "\n--- Ответ от Gate.io ---" << std::endl;
            for (const auto& message : event.getMessageList()) {
                std::cout << "Сообщение: " << toString(message) << std::endl;
                std::cout << "\nДетали:" << std::endl;
                
                for (const auto& element : message.getElementList()) {
                    const auto& nameValueMap = element.getNameValueMap();
                    std::cout << "  Элемент:" << std::endl;
                    for (const auto& [key, value] : nameValueMap) {
                        std::cout << "    " << key << " = " << value << std::endl;
                    }
                }
                
                responseData = message.toString();
            }
        } else if (event.getType() == Event::Type::SUBSCRIPTION_DATA) {
            std::cout << "\n--- Данные подписки ---" << std::endl;
            for (const auto& message : event.getMessageList()) {
                std::cout << toString(message) << std::endl;
            }
        } else if (event.getType() == Event::Type::SUBSCRIPTION_STATUS) {
            std::cout << "\n--- Статус подписки ---" << std::endl;
            std::cout << toString(event) << std::endl;
        }
        
        std::cout << "\nПолное событие:\n" << event.toPrettyString(2, 2) << std::endl;
    }
};

int main(int argc, char** argv) {
    std::cout << "╔════════════════════════════════════════════════════════╗" << std::endl;
    std::cout << "║   Тест подключения к Gate.io через CCAPI              ║" << std::endl;
    std::cout << "╚════════════════════════════════════════════════════════╝" << std::endl;
    std::cout << std::endl;

    // API ключи из конфигурации
    std::string apiKey = "ac78ffea0103fcb2d0c25ab89e5c3b34";
    std::string apiSecret = "9cfc0c897560614f4cbbc558c172af81a6c5d0ef6612623692646ccecdb97d6f";

    if (argc >= 3) {
        apiKey = argv[1];
        apiSecret = argv[2];
    }

    std::cout << "API Key: " << apiKey.substr(0, 10) << "..." << std::endl;
    std::cout << "API Secret: " << apiSecret.substr(0, 10) << "..." << std::endl;
    std::cout << std::endl;

    SessionOptions sessionOptions;
    SessionConfigs sessionConfigs;
    
    // Настройка API ключей
    sessionConfigs.setCredential({
        {CCAPI_GATEIO_API_KEY, apiKey},
        {CCAPI_GATEIO_API_SECRET, apiSecret}
    });

    TestEventHandler eventHandler;
    Session session(sessionOptions, sessionConfigs, &eventHandler);

    std::cout << "=== Тест 1: Получение списка торговых пар ===" << std::endl;
    Request request1(Request::Operation::GET_INSTRUMENTS, "gateio", "");
    request1.setCorrelationId("GET_PAIRS");
    session.sendRequest(request1);
    
    std::this_thread::sleep_for(std::chrono::seconds(3));
    
    if (!eventHandler.receivedResponse) {
        std::cout << "⚠️ Не получен ответ на запрос списка пар" << std::endl;
    }

    std::cout << "\n=== Тест 2: Получение тикера ETH_USDT ===" << std::endl;
    Request request2(Request::Operation::GET_BBOS, "gateio", "ETH_USDT");
    request2.setCorrelationId("GET_TICKER");
    session.sendRequest(request2);
    
    std::this_thread::sleep_for(std::chrono::seconds(3));

    std::cout << "\n=== Тест 3: Получение баланса аккаунта ===" << std::endl;
    // Для баланса используем GENERIC_PRIVATE_REQUEST
    Request request3(Request::Operation::GENERIC_PRIVATE_REQUEST, "gateio", "", "GET_BALANCE");
    request3.appendParam({
        {"url", "/api/v4/spot/accounts"},
        {"method", "GET"}
    });
    request3.setCorrelationId("GET_BALANCE");
    session.sendRequest(request3);
    
    std::this_thread::sleep_for(std::chrono::seconds(3));

    std::cout << "\n=== Тест 4: Подписка на тикер ETH_USDT ===" << std::endl;
    Subscription subscription("gateio", "ETH_USDT", "MARKET_DATA");
    session.subscribe(subscription);
    
    std::cout << "Ожидание данных (10 секунд)..." << std::endl;
    std::this_thread::sleep_for(std::chrono::seconds(10));

    session.stop();
    
    std::cout << "\n=== Итоги тестирования ===" << std::endl;
    if (eventHandler.receivedResponse) {
        std::cout << "✅ Получены ответы от Gate.io API" << std::endl;
    } else {
        std::cout << "❌ Не получены ответы от Gate.io API" << std::endl;
    }
    
    std::cout << "\nТестирование завершено." << std::endl;
    return EXIT_SUCCESS;
}

