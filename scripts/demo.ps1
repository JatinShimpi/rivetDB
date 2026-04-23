# RivetDB Interactive Demo Script
# Usage: .\scripts\demo.ps1
# Requirements: RivetDB running on port 7878, Docker Desktop

$ErrorActionPreference = "SilentlyContinue"

$PORT = 7878
$HOST = "host.docker.internal"

function Send-Command {
    param([string[]]$args)
    $cmd = $args -join " "
    Write-Host "> $cmd" -ForegroundColor Cyan
    docker run --rm redis redis-cli -h $HOST -p $PORT $args
    Write-Host ""
}

function Section {
    param([string]$title)
    Write-Host ""
    Write-Host "============================================" -ForegroundColor Magenta
    Write-Host "  $title" -ForegroundColor Magenta
    Write-Host "============================================" -ForegroundColor Magenta
    Write-Host ""
}

function Pause-Demo {
    Write-Host "Press Enter to continue..." -ForegroundColor Yellow
    Read-Host
}

# Start
Clear-Host
Write-Host ""
Write-Host "  ██████╗ ██╗██╗   ██╗███████╗████████╗██████╗ ██████╗ " -ForegroundColor Cyan
Write-Host "  ██╔══██╗██║██║   ██║██╔════╝╚══██╔══╝██╔══██╗██╔══██╗" -ForegroundColor Cyan
Write-Host "  ██████╔╝██║██║   ██║█████╗     ██║   ██║  ██║██████╔╝" -ForegroundColor Cyan
Write-Host "  ██╔══██╗██║╚██╗ ██╔╝██╔══╝     ██║   ██║  ██║██╔══██╗" -ForegroundColor Cyan
Write-Host "  ██║  ██║██║ ╚████╔╝ ███████╗   ██║   ██████╔╝██████╔╝" -ForegroundColor Cyan
Write-Host "  ╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝   ╚═╝   ╚═════╝ ╚═════╝ " -ForegroundColor Cyan
Write-Host ""
Write-Host "  A High-Performance In-Memory Database in Rust" -ForegroundColor White
Write-Host ""
Pause-Demo

# Check connection
Write-Host "Checking connection to RivetDB..." -ForegroundColor Yellow
$testResult = docker run --rm redis redis-cli -h $HOST -p $PORT PING 2>&1
if ($testResult -ne "PONG") {
    Write-Host "ERROR: Cannot connect to RivetDB on port $PORT" -ForegroundColor Red
    Write-Host "Start RivetDB with: cargo run --release" -ForegroundColor Yellow
    exit 1
}
Write-Host "Connected!" -ForegroundColor Green
Write-Host ""

# ============================================
# STRINGS
# ============================================
Section "1. STRING OPERATIONS"
Write-Host "Strings are the most basic data type in RivetDB." -ForegroundColor Gray
Write-Host ""

Send-Command "SET", "user:1:name", "Alice"
Send-Command "GET", "user:1:name"
Send-Command "APPEND", "user:1:name", " Smith"
Send-Command "GET", "user:1:name"
Send-Command "STRLEN", "user:1:name"

Write-Host "Atomic counters:" -ForegroundColor Gray
Send-Command "SET", "visitors", "100"
Send-Command "INCR", "visitors"
Send-Command "INCRBY", "visitors", "10"
Send-Command "GET", "visitors"

Pause-Demo

# ============================================
# LISTS
# ============================================
Section "2. LIST OPERATIONS (Queues/Stacks)"
Write-Host "Lists are perfect for queues, stacks, and timelines." -ForegroundColor Gray
Write-Host ""

Send-Command "LPUSH", "tasks", "task1", "task2", "task3"
Send-Command "LRANGE", "tasks", "0", "-1"
Send-Command "RPUSH", "tasks", "task4"
Send-Command "LPOP", "tasks"
Send-Command "LLEN", "tasks"
Send-Command "LRANGE", "tasks", "0", "-1"

Pause-Demo

# ============================================
# SETS
# ============================================
Section "3. SET OPERATIONS (Unique Collections)"
Write-Host "Sets store unique elements - great for tags, friends, etc." -ForegroundColor Gray
Write-Host ""

Send-Command "SADD", "user:1:skills", "rust", "python", "javascript"
Send-Command "SADD", "user:2:skills", "python", "java", "go"
Send-Command "SMEMBERS", "user:1:skills"
Send-Command "SISMEMBER", "user:1:skills", "rust"
Send-Command "SINTER", "user:1:skills", "user:2:skills"
Send-Command "SUNION", "user:1:skills", "user:2:skills"

Pause-Demo

# ============================================
# HASHES
# ============================================
Section "4. HASH OPERATIONS (Objects)"
Write-Host "Hashes are like objects/dictionaries - perfect for user profiles." -ForegroundColor Gray
Write-Host ""

Send-Command "HSET", "user:100", "name", "Bob", "age", "30", "city", "NYC"
Send-Command "HGET", "user:100", "name"
Send-Command "HGETALL", "user:100"
Send-Command "HINCRBY", "user:100", "age", "1"
Send-Command "HGET", "user:100", "age"

Pause-Demo

# ============================================
# SORTED SETS
# ============================================
Section "5. SORTED SET OPERATIONS (Leaderboards)"
Write-Host "Sorted sets maintain elements ordered by score - ideal for rankings." -ForegroundColor Gray
Write-Host ""

Send-Command "ZADD", "leaderboard", "100", "player1", "250", "player2", "180", "player3"
Send-Command "ZRANGE", "leaderboard", "0", "-1", "WITHSCORES"
Send-Command "ZREVRANGE", "leaderboard", "0", "-1", "WITHSCORES"
Send-Command "ZINCRBY", "leaderboard", "50", "player1"
Send-Command "ZRANK", "leaderboard", "player1"
Send-Command "ZSCORE", "leaderboard", "player1"

Pause-Demo

# ============================================
# TTL/EXPIRY
# ============================================
Section "6. KEY EXPIRATION (TTL)"
Write-Host "Keys can automatically expire - essential for caching." -ForegroundColor Gray
Write-Host ""

Send-Command "SET", "session:abc", "user_data"
Send-Command "EXPIRE", "session:abc", "60"
Send-Command "TTL", "session:abc"
Write-Host "Key will automatically delete after 60 seconds!" -ForegroundColor Yellow
Write-Host ""

Pause-Demo

# ============================================
# ADMIN COMMANDS
# ============================================
Section "7. ADMIN & MONITORING"
Write-Host "RivetDB provides built-in monitoring capabilities." -ForegroundColor Gray
Write-Host ""

Send-Command "STATS"
Send-Command "KEYS", "*"

Pause-Demo

# ============================================
# CLEANUP
# ============================================
Section "CLEANUP"
Write-Host "Cleaning up demo data..." -ForegroundColor Gray
Send-Command "DEL", "user:1:name", "visitors", "tasks", "user:1:skills", "user:2:skills", "user:100", "leaderboard", "session:abc"

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "  Demo Complete!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host ""
Write-Host "RivetDB Features Demonstrated:" -ForegroundColor Cyan
Write-Host "  [✓] Strings with atomic operations"
Write-Host "  [✓] Lists (queues/stacks)"
Write-Host "  [✓] Sets (unique collections)"
Write-Host "  [✓] Hashes (objects)"
Write-Host "  [✓] Sorted Sets (leaderboards)"
Write-Host "  [✓] Key expiration (TTL)"
Write-Host "  [✓] Admin/monitoring commands"
Write-Host ""
Write-Host "Run benchmarks with: .\scripts\benchmark.ps1" -ForegroundColor Yellow
Write-Host ""
