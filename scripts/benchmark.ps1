# RivetDB Benchmark Script
# Usage: .\scripts\benchmark.ps1
# Requirements: Docker Desktop running

param(
    [int]$Operations = 100000,
    [int]$Clients = 50,
    [switch]$SkipRedis,
    [switch]$Detailed
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "       RivetDB vs Redis Benchmark" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

$RIVET_PORT = 7878
$REDIS_PORT = 6379
$DOCKER_HOST = "host.docker.internal"

# Function to format numbers with commas
function Format-Number($num) {
    return "{0:N0}" -f $num
}

# Check Docker
Write-Host "[Prerequisites] Checking Docker..." -ForegroundColor Yellow
try {
    docker version | Out-Null
    Write-Host "  Docker is running" -ForegroundColor Green
} catch {
    Write-Host "  ERROR: Docker is not running!" -ForegroundColor Red
    Write-Host "  Please start Docker Desktop and try again." -ForegroundColor Yellow
    exit 1
}

# Check RivetDB
Write-Host "[Prerequisites] Checking RivetDB on port $RIVET_PORT..." -ForegroundColor Yellow
$rivetTest = Test-NetConnection -ComputerName localhost -Port $RIVET_PORT -WarningAction SilentlyContinue -ErrorAction SilentlyContinue
if (-not $rivetTest.TcpTestSucceeded) {
    Write-Host "  ERROR: RivetDB not running on port $RIVET_PORT" -ForegroundColor Red
    Write-Host ""
    Write-Host "  Start RivetDB first:" -ForegroundColor Yellow
    Write-Host "    cd c:\rivetdb" -ForegroundColor White
    Write-Host "    cargo run --release" -ForegroundColor White
    Write-Host ""
    exit 1
}
Write-Host "  RivetDB is running" -ForegroundColor Green

Write-Host ""
Write-Host "Benchmark Parameters:" -ForegroundColor Cyan
Write-Host "  Operations: $(Format-Number $Operations)"
Write-Host "  Clients:    $Clients concurrent"
Write-Host ""

# ============================================
# RIVETDB BENCHMARKS
# ============================================
Write-Host "============================================" -ForegroundColor Magenta
Write-Host "         RIVETDB BENCHMARK" -ForegroundColor Magenta
Write-Host "============================================" -ForegroundColor Magenta
Write-Host ""

# Basic Operations
Write-Host "[RivetDB] Basic Operations (SET, GET)..." -ForegroundColor Yellow
docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $RIVET_PORT `
    -t set,get -n $Operations -c $Clients -q

Write-Host ""
Write-Host "[RivetDB] Data Structures (INCR, LPUSH, SADD, HSET, ZADD)..." -ForegroundColor Yellow
docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $RIVET_PORT `
    -t incr,lpush,sadd,hset,zadd -n $Operations -c $Clients -q

if ($Detailed) {
    Write-Host ""
    Write-Host "[RivetDB] High Concurrency (100 clients)..." -ForegroundColor Yellow
    docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $RIVET_PORT `
        -t set,get -n $Operations -c 100 -q
    
    Write-Host ""
    Write-Host "[RivetDB] Pipeline Test (16 commands/batch)..." -ForegroundColor Yellow
    docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $RIVET_PORT `
        -t set,get -n $Operations -P 16 -q
}

# ============================================
# REDIS BENCHMARKS (if not skipped)
# ============================================
if (-not $SkipRedis) {
    Write-Host ""
    Write-Host "============================================" -ForegroundColor Red
    Write-Host "           REDIS BENCHMARK" -ForegroundColor Red
    Write-Host "============================================" -ForegroundColor Red
    Write-Host ""
    
    # Start Redis
    Write-Host "[Redis] Starting Redis container..." -ForegroundColor Yellow
    # Cleanup any existing container (ignore errors if it doesn't exist)
    $ErrorActionPreference = "SilentlyContinue"
    docker stop redis-bench 2>&1 | Out-Null
    docker rm redis-bench 2>&1 | Out-Null
    $ErrorActionPreference = "Stop"
    docker run -d --name redis-bench -p ${REDIS_PORT}:6379 redis | Out-Null
    Start-Sleep -Seconds 2
    Write-Host "  Redis started on port $REDIS_PORT" -ForegroundColor Green
    Write-Host ""
    
    # Basic Operations
    Write-Host "[Redis] Basic Operations (SET, GET)..." -ForegroundColor Yellow
    docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $REDIS_PORT `
        -t set,get -n $Operations -c $Clients -q
    
    Write-Host ""
    Write-Host "[Redis] Data Structures (INCR, LPUSH, SADD, HSET, ZADD)..." -ForegroundColor Yellow
    docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $REDIS_PORT `
        -t incr,lpush,sadd,hset,zadd -n $Operations -c $Clients -q
    
    if ($Detailed) {
        Write-Host ""
        Write-Host "[Redis] High Concurrency (100 clients)..." -ForegroundColor Yellow
        docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $REDIS_PORT `
            -t set,get -n $Operations -c 100 -q
        
        Write-Host ""
        Write-Host "[Redis] Pipeline Test (16 commands/batch)..." -ForegroundColor Yellow
        docker run --rm redis redis-benchmark -h $DOCKER_HOST -p $REDIS_PORT `
            -t set,get -n $Operations -P 16 -q
    }
    
    # Cleanup
    Write-Host ""
    Write-Host "[Cleanup] Stopping Redis container..." -ForegroundColor Yellow
    docker stop redis-bench | Out-Null
    docker rm redis-bench | Out-Null
    Write-Host "  Redis container removed" -ForegroundColor Green
}

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "       BENCHMARK COMPLETE!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Tips:" -ForegroundColor Yellow
Write-Host "  - Run with -Detailed for more tests"
Write-Host "  - Run with -SkipRedis to only test RivetDB"
Write-Host "  - Increase -Clients to see multi-threading advantage"
Write-Host ""
