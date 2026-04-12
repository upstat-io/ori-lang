---
paths:
  - "**"
---

# Intelligence Graph

## Availability

The intelligence graph at `../lang_intelligence/` is OPTIONAL. Check before querying:
```
scripts/intel-query.sh status
```
If unavailable, proceed normally — intelligence is additive, never blocking.

## When to Query

Query the intelligence graph proactively in these workflows:

- **Design decisions**: Before choosing an approach, query for how reference compilers handled it
- **Bug investigation** (/fix-bug Phase 1): Query for similar bugs across languages
- **TPR reviews** (/tpr-review Step 2): Pre-query relevant prior art for the evidence packet
- **Code reviews** (/review-work): Query for prior art relevant to the reviewed changes
- **Plan reviews** (/review-plan): Query for cross-language precedent on plan assumptions
- **Proposals** (/create-draft-proposal): Query cross-language precedent for the Prior Art section
- **Pattern review** (/design-pattern-review): Query for equivalent implementations
- **Roadmap** (/continue-roadmap): After focus resolution, query for section-relevant intelligence

## How to Query

Always use the canonical helper — never open-code Neo4j access:
```
scripts/intel-query.sh search "pattern matching exhaustiveness"
scripts/intel-query.sh compare "type inference"
scripts/intel-query.sh fixed "memory leak" --repo rust,swift
scripts/intel-query.sh hot --repo rust
scripts/intel-query.sh ori-arc                          # also: ori-inference, ori-codegen, ori-patterns, ori-diagnostics
scripts/intel-query.sh cypher "MATCH (i:Issue)-[:FIXES]->(b) RETURN count(i)"
```

## How to Use Results

Results are for DISCOVERY, not replacement:
- A hit means "investigate this" — read the actual source code and issue
- Weight by: state_reason (completed > not_planned), reactions, author_association (MEMBER > NONE)
- Cross-language convergence is highest signal (3+ repos hit same issue class)
- Never cite a Neo4j result without verifying against the actual code/issue
