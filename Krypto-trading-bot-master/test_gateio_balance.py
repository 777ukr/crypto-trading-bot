#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Быстрый тест подключения к Gate.io API и проверка баланса
Использует официальный формат Gate.io API v4
"""

import hashlib
import hmac
import json
import time
from datetime import datetime

import requests

# API ключи из конфигурации
API_KEY = "ac78ffea0103fcb2d0c25ab89e5c3b34"
API_SECRET = "9cfc0c897560614f4cbbc558c172af81a6c5d0ef6612623692646ccecdb97d6f"

BASE_URL = "https://api.gateio.ws"

def get_timestamp():
    """Получить текущий timestamp в секундах"""
    return int(time.time())

def sign_request(method, path, query_string, payload, timestamp):
    """
    Создать подпись для Gate.io API v4
    
    Формат: METHOD\nPATH\nQUERY_STRING\nHEX(SHA512(PAYLOAD))\nTIMESTAMP
    
    Важно: query_string может быть пустым, но должен быть в формате строки
    """
    # Вычисляем хеш payload (даже если он пустой)
    if payload:
        payload_hash = hashlib.sha512(payload.encode('utf-8')).hexdigest()
    else:
        # Для пустого payload хеш тоже должен быть вычислен
        payload_hash = hashlib.sha512("".encode('utf-8')).hexdigest()
    
    # Формируем строку для подписи
    # ВАЖНО: query_string должен быть пустой строкой, не None
    sign_string = f"{method}\n{path}\n{query_string}\n{payload_hash}\n{timestamp}"
    
    # Создаем подпись
    signature = hmac.new(
        API_SECRET.encode('utf-8'),
        sign_string.encode('utf-8'),
        hashlib.sha512
    ).hexdigest()
    
    return signature, sign_string

def test_public_api():
    """Тест публичного API - получение тикера ETH_USDT"""
    print("=" * 60)
    print("ТЕСТ 1: Публичный API - Тикер ETH_USDT")
    print("=" * 60)
    
    url = f"{BASE_URL}/api/v4/spot/tickers"
    params = {"currency_pair": "ETH_USDT"}
    
    try:
        response = requests.get(url, params=params, timeout=10)
        response.raise_for_status()
        data = response.json()
        
        if data and len(data) > 0:
            ticker = data[0]
            print("✅ Подключение успешно!")
            print(f"   Пара: {ticker.get('currency_pair', 'N/A')}")
            print(f"   Цена: {ticker.get('last', 'N/A')} USDT")
            print(f"   Изменение: {ticker.get('change_percentage', 'N/A')}%")
            print(f"   Объем 24ч: {ticker.get('base_volume', 'N/A')} ETH")
            return True
        else:
            print("❌ Пустой ответ")
            return False
    except Exception as e:
        print(f"❌ Ошибка: {e}")
        return False

def test_private_api_balance():
    """Тест приватного API - получение баланса"""
    print("\n" + "=" * 60)
    print("ТЕСТ 2: Приватный API - Баланс аккаунта")
    print("=" * 60)
    
    # Согласно документации Gate.io, правильный путь для баланса
    method = "GET"
    path = "/api/v4/spot/accounts"
    query_string = ""  # Пустая строка для GET запроса без параметров
    payload = ""
    timestamp = get_timestamp()
    
    signature, sign_string = sign_request(method, path, query_string, payload, timestamp)
    
    headers = {
        "KEY": API_KEY,
        "Timestamp": str(timestamp),
        "SIGN": signature,
        "Content-Type": "application/json",
        "Accept": "application/json"
    }
    
    url = BASE_URL + path
    
    print(f"URL: {url}")
    print(f"Method: {method}")
    print(f"Timestamp: {timestamp}")
    print(f"Sign string (repr): {repr(sign_string)}")
    print(f"Signature (first 30): {signature[:30]}...")
    
    try:
        response = requests.get(url, headers=headers, timeout=10)
        
        print(f"HTTP Status: {response.status_code}")
        
        if response.status_code == 200:
            data = response.json()
            print("✅ Баланс получен!")
            print("\n💰 Балансы на спот-аккаунте:")
            print("-" * 60)
            
            total_balance = 0.0
            non_zero_balances = []
            
            for account in data:
                currency = account.get('currency', '')
                available = float(account.get('available', '0'))
                locked = float(account.get('locked', '0'))
                total = available + locked
                
                if total > 0:
                    non_zero_balances.append({
                        'currency': currency,
                        'available': available,
                        'locked': locked,
                        'total': total
                    })
            
            if non_zero_balances:
                for bal in sorted(non_zero_balances, key=lambda x: x['total'], reverse=True):
                    print(f"   {bal['currency']:8s} | Доступно: {bal['available']:15.8f} | Заблокировано: {bal['locked']:15.8f} | Всего: {bal['total']:15.8f}")
            else:
                print("   Нет балансов (paper trading или пустой аккаунт)")
            
            print("-" * 60)
            return True
        else:
            print(f"❌ Ошибка HTTP {response.status_code}")
            print(f"Ответ: {response.text}")
            return False
    except Exception as e:
        print(f"❌ Ошибка: {e}")
        return False

def test_websocket_format():
    """Проверка формата подписки WebSocket"""
    print("\n" + "=" * 60)
    print("ТЕСТ 3: Формат подписки WebSocket")
    print("=" * 60)
    
    timestamp = get_timestamp()
    channel = "spot.balances"
    event = "subscribe"
    
    # Формат подписи для WebSocket: channel=xxx&event=xxx&time=xxx
    message = f"channel={channel}&event={event}&time={timestamp}"
    signature = hmac.new(
        API_SECRET.encode('utf-8'),
        message.encode('utf-8'),
        hashlib.sha512
    ).hexdigest()
    
    ws_request = {
        "time": timestamp,
        "channel": channel,
        "event": event,
        "auth": {
            "method": "api_key",
            "KEY": API_KEY,
            "SIGN": signature
        }
    }
    
    print("✅ Формат подписки WebSocket:")
    print(json.dumps(ws_request, indent=2))
    print(f"\nПодпись создана для: {message}")
    print(f"Signature: {signature[:20]}...")
    
    return True

def main():
    print("\n" + "=" * 60)
    print("  ТЕСТ ПОДКЛЮЧЕНИЯ К GATE.IO API v4")
    print("=" * 60)
    print(f"\nВремя: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"API Key: {API_KEY[:12]}...")
    print()
    
    results = []
    
    # Тест 1: Публичный API
    results.append(("Публичный API", test_public_api()))
    
    # Тест 2: Приватный API (баланс)
    results.append(("Приватный API (баланс)", test_private_api_balance()))
    
    # Тест 3: Формат WebSocket
    results.append(("Формат WebSocket", test_websocket_format()))
    
    # Итоги
    print("\n" + "=" * 60)
    print("ИТОГИ ТЕСТИРОВАНИЯ")
    print("=" * 60)
    for name, result in results:
        status = "✅ ПРОШЕЛ" if result else "❌ НЕ ПРОШЕЛ"
        print(f"  {name:30s} {status}")
    
    print("\n" + "=" * 60)
    
    if all(result for _, result in results):
        print("✅ Все тесты пройдены успешно!")
        return 0
    else:
        print("⚠️ Некоторые тесты не прошли")
        return 1

if __name__ == "__main__":
    exit(main())

