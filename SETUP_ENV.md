# 🔧 Настройка .env файла

## ✅ Что уже есть в вашем .env:

- `GATEIO_API_KEY` ✅
- `GATEIO_SECRET_KEY` ✅

## ❌ Что нужно добавить:

### DATABASE_URL (обязательно)

Добавьте в ваш `.env` файл:

```bash
DATABASE_URL=postgresql://postgres:ваш_пароль@localhost:5432/cryptotrader
```

**Пример полного .env файла:**

```bash
# Gate.io API ключи (уже есть)
GATEIO_API_KEY=your_key_here
GATEIO_SECRET_KEY=your_secret_here

# PostgreSQL (нужно добавить)
DATABASE_URL=postgresql://postgres:password@localhost:5432/cryptotrader

# Логирование (опционально)
RUST_LOG=info
```

## 📝 Быстрая команда для добавления:

```bash
# Добавить DATABASE_URL в .env (замените password на ваш пароль)
echo 'DATABASE_URL=postgresql://postgres:password@localhost:5432/cryptotrader' >> .env
```

## ✅ Проверка:

```bash
# Проверить что все переменные есть
cat .env | grep -E "DATABASE_URL|GATEIO"
```

## 🔄 Совместимость

Код поддерживает оба варианта названий Gate.io переменных:
- `GATE_API_KEY` или `GATEIO_API_KEY` ✅
- `GATE_API_SECRET` или `GATEIO_SECRET_KEY` ✅

Ваши текущие названия (`GATEIO_API_KEY` и `GATEIO_SECRET_KEY`) работают!

