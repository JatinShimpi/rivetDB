# RivetDB Diagram Guide

Use the following diagrams as a reference to recreate them in your flowchart application (like Draw.io, Lucidchart, or Visio). 

## 1. SDLC Phases Diagram (Agile Methodology)
**Location:** Chapter 1
```mermaid
flowchart LR
    A[Requirement Analysis] --> B[System Design]
    B --> C[Implementation in Rust]
    C --> D[Testing & Benchmarking]
    D --> E[Deployment & Review]
    E -. Iterative Feedback .-> A
```

## 2. Basic Block Diagram (System Architecture)
**Location:** Chapter 3
```mermaid
flowchart TD
    Client[Redis Clients / TCP Streams] --> Network[Tokio Async Network Engine]
    Network --> Parser[RESP & SQL Parser]
    Parser --> Engine[Command Execution Engine]
    Engine --> Storage[DashMap Core Storage]
    Engine --> AOF[AOF Persistence Channel]
    AOF --> Disk[(File System)]
```

## 3. System Workflow Flowchart
**Location:** Chapter 3
```mermaid
flowchart TD
    Start([Client Connects]) --> Read[Read TCP Stream]
    Read --> Parse{Parse Valid RESP?}
    Parse -- No --> ErrResp[Return Error]
    Parse -- Yes --> Cmd{Command Type?}
    Cmd -- Read --> ExecRead[Read from DashMap]
    Cmd -- Write --> ExecWrite[Mutate DashMap]
    ExecWrite --> AOF[Push to AOF Channel]
    ExecRead --> Send[Send Response]
    AOF --> Send
    ErrResp --> Send
    Send --> Loop[Wait for Next Command]
```

## 4. Hash Map Sharding Diagram (DashMap)
**Location:** Chapter 3
```mermaid
flowchart TD
    Req1[Thread 1: SET KeyA] --> H[Hash Function]
    Req2[Thread 2: SET KeyB] --> H
    Req3[Thread 3: GET KeyC] --> H
    H --> S1[(Shard 1)]
    H --> S2[(Shard 2)]
    H --> S3[(Shard 3)]
    H --> S4[(Shard 4)]
    
    Info["Each Shard has its own Lock.<br>Threads 1 and 2 can write simultaneously<br>if they hit different shards!"]
```

## 5. Time-Series Layout
**Location:** Chapter 3
```mermaid
flowchart LR
    Key[Key: sensor_temp] --> Tree[(BTreeMap)]
    Tree --> Node1[Timestamp: 16900000 -> Value: 24.5]
    Tree --> Node2[Timestamp: 16900005 -> Value: 24.8]
    Tree --> Node3[Timestamp: 16900010 -> Value: 25.1]
    
    Info["BTreeMap keeps timestamps automatically sorted<br>for O(log N) range queries!"]
```

## 6. Multi-Tenancy Architecture
**Location:** Chapter 3
```mermaid
flowchart TD
    Client1[Tenant A] --> PrefixA["Key: A:user:1"]
    Client2[Tenant B] --> PrefixB["Key: B:user:1"]
    
    PrefixA --> Engine[RivetDB Engine]
    PrefixB --> Engine
    
    Engine --> Mem1[Tenant A Memory Quota]
    Engine --> Mem2[Tenant B Memory Quota]
```

## 7. Network Event Loop Sequence Diagram
**Location:** Chapter 4
```mermaid
sequenceDiagram
    participant Client
    participant Tokio Event Loop
    participant Worker Task
    
    Client->>Tokio Event Loop: Connect (TCP)
    Tokio Event Loop->>Worker Task: tokio::spawn(handle_client)
    Client->>Worker Task: Send SET Command
    Worker Task->>Worker Task: Parse RESP
    Worker Task->>Worker Task: Execute against DashMap
    Worker Task->>Client: Send +OK Response
```

## 8. Asynchronous AOF Persistence Workflow
**Location:** Chapter 4
```mermaid
flowchart LR
    Worker1[Worker Task 1] --> Chan((MPSC Channel))
    Worker2[Worker Task 2] --> Chan
    Worker3[Worker Task 3] --> Chan
    
    Chan --> BgThread[Background OS Thread]
    BgThread --> BufWriter[64KB BufWriter]
    BufWriter -- Every 1 Second --> fsync[(Disk fsync)]
```

## 9. Memory Eviction Algorithm Flowchart
**Location:** Chapter 5
```mermaid
flowchart TD
    MemCheck{Current Memory >= Max Memory?}
    MemCheck -- No --> Proceed[Execute Command]
    MemCheck -- Yes --> Sample[Randomly Sample 5 Keys]
    Sample --> Find[Find Key with oldest LRU time]
    Find --> Evict[Evict Key from Memory]
    Evict --> MemCheck
```

## 10. SQL Query Execution Flow
**Location:** Chapter 4
```mermaid
flowchart TD
    SQL["SELECT id, name WHERE age gt 25"] --> Lexer[Tokenization]
    Lexer --> Parser[Generate AST]
    Parser --> Engine[Iterate DashMap]
    Engine --> Eval{Evaluate Predicate}
    Eval -- True --> Results[Add to Result Array]
    Eval -- False --> Skip[Skip Entry]
    Results --> Format[Format as RESP Array]
```

## 11. Literature Review Timeline/Comparison
**Location:** Chapter 2
```mermaid
gantt
    title Evolution of In-Memory Data Stores
    dateFormat YYYY
    axisFormat %Y
    
    section Disk-Based
    PostgreSQL       :1996, 2010
    MySQL            :1995, 2010
    
    section In-Memory Caching
    Memcached        :2003, 2020
    Redis (Single Thread) :2009, 2026
    
    section Modern IMDBs
    RivetDB (Multi-Thread) :2025, 2026
```

## 12. Benchmark Architecture
**Location:** Chapter 6
```mermaid
flowchart LR
    Tool[redis-benchmark Tool] --> TCP1[Connection 1]
    Tool --> TCPN["Connection N (Up to 1000)"]
    
    TCP1 --> RivetDB[RivetDB Server]
    TCPN --> RivetDB
    
    RivetDB --> CPU[Multi-Core CPU Utilization]
```
