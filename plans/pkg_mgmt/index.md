# Ori Package Management Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

Quick-reference keyword index for finding package management implementation sections.

---

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file: `section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Manifest
**File:** `section-01-manifest.md` | **Status:** Not Started

```
manifest, oripk.toml, package.toml
metadata, name, version, description
dependencies, deps, requires
features, optional features
```

---

### Section 02: Resolution
**File:** `section-02-resolution.md` | **Status:** Not Started

```
version resolution, solver, SAT
exact version, single version policy
dependency graph, dep graph
conflict, resolution error
```

---

### Section 03: Cache
**File:** `section-03-cache.md` | **Status:** Not Started

```
cache, ~/.ori/cache, global cache
installation, install, fetch
checksum, integrity, SHA256
lock file, oripk.lock
```

---

### Section 04: Registry Protocol
**File:** `section-04-registry-protocol.md` | **Status:** Not Started

```
registry, package registry
protocol, API, REST
metadata endpoint, download
Cloudflare, Workers, R2
```

---

### Section 05: Registry Client
**File:** `section-05-registry-client.md` | **Status:** Not Started

```
client, registry client
fetch, download, request
authentication, token, API key
rate limiting, retry
```

---

### Section 06: Dependency Commands
**File:** `section-06-dep-commands.md` | **Status:** Not Started

```
ori install, install
ori add, add dependency
ori remove, remove dependency
ori upgrade, upgrade, update
ori sync, sync, lock
ori check, verify, audit
```

---

### Section 07: Publishing
**File:** `section-07-publishing.md` | **Status:** Not Started

```
ori publish, publish, upload
package, tarball, archive
yank, unpublish, deprecate
version bump, semver
```

---

### Section 08: Workspaces
**File:** `section-08-workspaces.md` | **Status:** Not Started

```
workspace, monorepo
member, package member
shared dependencies, hoisting
workspace root, workspace member
```

---

### Section 09: Scripts
**File:** `section-09-scripts.md` | **Status:** Not Started

```
scripts, ori run, task
pre, post, lifecycle hooks
custom commands, aliases
```

---

### Section 10: Tooling
**File:** `section-10-tooling.md` | **Status:** Not Started

```
ori docs, documentation
REPL, interactive, ori repl
ori search, search registry
ori info, package info
```

---

### Section 11: Infrastructure
**File:** `section-11-infrastructure.md` | **Status:** Not Started

```
deployment, production
Cloudflare, Workers, R2, KV
CDN, distribution, hosting
monitoring, analytics
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Manifest | `section-01-manifest.md` |
| 02 | Resolution | `section-02-resolution.md` |
| 03 | Cache | `section-03-cache.md` |
| 04 | Registry Protocol | `section-04-registry-protocol.md` |
| 05 | Registry Client | `section-05-registry-client.md` |
| 06 | Dependency Commands | `section-06-dep-commands.md` |
| 07 | Publishing | `section-07-publishing.md` |
| 08 | Workspaces | `section-08-workspaces.md` |
| 09 | Scripts | `section-09-scripts.md` |
| 10 | Tooling | `section-10-tooling.md` |
| 11 | Infrastructure | `section-11-infrastructure.md` |
