# Phase 11: Registry Infrastructure

**Goal**: Deploy registry on Cloudflare

**Status**: ⬜ Not Started

---

## 11.1 Architecture

```
┌─────────────────────────────────────┐
│         Cloudflare Edge             │
│                                     │
│  Workers (Ori/WASM)                 │
│    ├── API endpoints                │
│    ├── Auth / rate limiting         │
│    ├── Search                       │
│    │                                │
│  Containers (Ori native)            │
│    ├── Package processing           │
│    ├── Advisory scanning            │
│    │                                │
│   R2              KV                │
│  (packages)    (metadata)           │
└─────────────────────────────────────┘
```

---

## 11.2 Workers (Ori/WASM)

- [ ] **Implement**: API endpoint handlers
  - [ ] All v1 endpoints
  - [ ] Written in Ori, compiled to WASM
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Authentication middleware
  - [ ] Token validation
  - [ ] Scope checking
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Rate limiting
  - [ ] Per-IP and per-token limits
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Search indexing
  - [ ] Query KV for matches
  - [ ] **Tests**: Integration tests

---

## 11.3 Containers (Ori native)

- [ ] **Implement**: Package processor
  - [ ] Validate uploads
  - [ ] Compute checksums
  - [ ] Written in Ori, native binary
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Advisory scanner
  - [ ] Check new packages against advisories
  - [ ] **Tests**: Integration tests

---

## 11.4 R2 Storage

- [ ] **Implement**: Package storage
  - [ ] `.ori.tar.gz` archives
  - [ ] Content-addressed paths
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Checksum storage
  - [ ] Transparency log data
  - [ ] **Tests**: Integration tests

---

## 11.5 KV Storage

- [ ] **Implement**: Package metadata
  - [ ] Version lists
  - [ ] Package info
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Search index
  - [ ] Name/description indexing
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: User/scope data
  - [ ] Ownership records
  - [ ] Token records
  - [ ] **Tests**: Integration tests

---

## 11.6 Advisory Database

- [ ] **Implement**: Advisory storage
  - [ ] CVE records
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Advisory API
  - [ ] Query by package
  - [ ] **Tests**: Integration tests

---

## 11.7 Monitoring

- [ ] **Implement**: Health checks
  - [ ] Endpoint monitoring
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Error tracking
  - [ ] Log errors
  - [ ] **Tests**: Integration tests

---

## 11.8 Phase Completion Checklist

- [ ] Workers handling all API endpoints
- [ ] Containers processing packages
- [ ] R2 storing packages
- [ ] KV storing metadata
- [ ] Advisory database working
- [ ] Monitoring in place
- [ ] Production deployment

**Exit Criteria**: Registry running on Cloudflare
