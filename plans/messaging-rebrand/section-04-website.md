---
section: "04"
title: "Website Landing Page"
status: complete
goal: "Update website landing page to match new positioning"
depends_on: ["01", "03"]
sections:
  - id: "04.1"
    title: "Hero Component"
    status: not-started
  - id: "04.2"
    title: "Features Component"
    status: not-started
  - id: "04.3"
    title: "VirtuousCycle Component"
    status: not-started
  - id: "04.4"
    title: "SEO & Metadata"
    status: not-started
  - id: "04.5"
    title: "BaseLayout Default Metadata"
    status: not-started
  - id: "04.6"
    title: "OG Image"
    status: not-started
  - id: "04.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Website Landing Page

**Status:** Not Started
**Goal:** Update the website landing page components to match the new positioning decided in Section 01 and implemented in Section 03 (README).

**Context:** The website at `ori-lang.com` uses Astro with Svelte components. The landing page consists of 4 components: Hero, Features, VirtuousCycle, and GetStarted. All need content updates to align with the new messaging.

**Depends on:** Section 01 (tagline, feature cards), Section 03 (README structure provides the content source).

---

## 04.1 Hero Component

**File:** `website/src/components/landing/Hero.astro`

### Current

```html
<h1>Code That Proves Itself</h1>
<p>Every function tested. Every change traced. Every effect explicit.</p>
```

### Target

Update `<h1>` to selected tagline from Section 01.
Update `<p>` to selected subtitle from Section 01.

- [ ] Update hero title (`h1.hero-title`)
- [ ] Update hero subtitle (`p.hero-subtitle`)
- [ ] Verify gradient styling works with new text length

---

## 04.2 Features Component

**File:** `website/src/components/landing/Features.astro`

### Current Feature Cards

1. "Invisible Memory Management"
2. "Mandatory Verification"
3. "Dependency-Aware Testing"
4. "Explicit Effects"

### Target Feature Cards (from Section 01.3)

1. **"The Memory Model Nobody Else Has"** — No garbage collector. No borrow checker. No manual allocation. Eight stacked optimizations turn functional code into in-place mutations — zero compromise between safety and speed.

2. **"Effects You Can See"** — Side effects are explicit. Every function declares its capabilities. Mocking is built-in — just provide a different implementation with `with...in`. No frameworks. No DI containers.

3. **"The Design That Compounds"** — Value semantics enables safe memory, which enables complete effects, which enables trivial mocking, which makes testing painless. One decision, compounding returns.

4. **"Smart Testing"** — Tests live in the dependency graph. Change a function, and every affected test runs automatically. Your tests even make your binary faster — they're automatic profiling data for the optimizer.

- [ ] Update `features` array in Features.astro with new titles and descriptions from Section 01.3
- [ ] Review SVG icons for card 2 ("Mandatory Verification" icon) and card 4 ("Dependency-Aware Testing" icon) — update if they depict "mandatory" concepts
- [ ] Decide whether to keep heading "What Makes Ori Different" or change (based on Section 01 tone)

---

## 04.3 VirtuousCycle Component

**File:** `website/src/components/landing/VirtuousCycle.astro`

### Current

Root: "Value Semantics" — correct, keep.

Branches:
1. "Safe Memory" — keep
2. "Complete Effects" — keep
3. "Trivial Mocking" — keep
4. "Safe Refactoring" — change: currently says "Mandatory tests + dependency tracking = change with confidence. If it compiles, it works."

### Target

Change branch 4 detail text from referencing mandatory tests:
```
Current: "Mandatory tests + dependency tracking = change with confidence. If it compiles, it works."
New:     "Smart testing + dependency tracking = change with confidence. Tests that know your code."
```

- [ ] Update branch 4 label and detail text
- [ ] Keep root and branches 1-3 unchanged (they're accurate)

---

## 04.4 SEO & Metadata

**File:** `website/src/pages/index.astro`

### Title Tag

```
Current: "Ori — Code That Proves Itself"
Target:  "Ori — [Selected Tagline]"
```

### Meta Description

```
Current: "Ori is a statically-typed language with functional semantics and imperative
         performance. No garbage collector, no borrow checker — eight stacked ARC
         optimizations turn value-semantic code into in-place mutations. Every function
         tested. Every effect explicit. Free and open source."

Target:  "Ori is a statically-typed language where functional code compiles to imperative
         speed. No garbage collector, no borrow checker — eight stacked optimizations turn
         value-semantic code into in-place mutations. Effects you can see. Testing that
         knows your code."
```

### FAQ Schema

Update the FAQ structured data:
- Question 2 ("How does mandatory testing work in Ori?") → Reframe around configurable testing
- Keep Questions 1, 3, 4, 5 with minor copy adjustments

### Keywords

```
Current includes: "mandatory testing"
Target: Replace with "smart testing", "dependency-aware testing", "configurable test enforcement"
```

- [ ] Update page title
- [ ] Update meta description
- [ ] Update FAQ schema (especially Q2 about mandatory testing)
- [ ] Update keywords meta tag

---

## 04.5 BaseLayout Default Metadata

**File:** `website/src/layouts/BaseLayout.astro`

The layout file provides default description, keywords, and structured data used by ALL pages (not just index.astro). These defaults contain "mandatory testing" references.

### Changes

```
Line 22: description default → remove "mandatory testing", "Every function tested"
Line 23: keywords default → replace "mandatory testing" with "smart testing" / "configurable test enforcement"
Line 113: structured data → replace "Mandatory testing for all functions"
```

- [ ] Update default `description` in BaseLayout.astro
- [ ] Update default `keywords` in BaseLayout.astro
- [ ] Update structured data features list in BaseLayout.astro

---

## 04.6 OG Image

**File:** `website/public/og-image.svg`

The Open Graph image (used for social media previews) contains hardcoded text:
- Line 24: `"Code That Proves Itself"` — old tagline
- Line 27: `"Mandatory testing · Dependency-aware integrity · Explicit effects"` — old subtitle

- [ ] Update tagline text in og-image.svg to match new tagline
- [ ] Update subtitle text to remove "Mandatory testing"
- [ ] Verify SVG renders correctly after text changes (text may need repositioning)

---

## 04.7 Completion Checklist

- [ ] Hero.astro updated with new tagline and subtitle
- [ ] Features.astro updated with new card order and copy
- [ ] VirtuousCycle.astro updated to remove mandatory testing reference
- [ ] index.astro metadata updated (title, description, FAQ, keywords)
- [ ] BaseLayout.astro default description, keywords, and structured data updated
- [ ] og-image.svg tagline and subtitle updated
- [ ] Website builds without errors (`npm run build` in website/)
- [ ] Visual review: text fits within layout at all breakpoints
- [ ] No broken links
- [ ] `grep -rn "mandatory test" website/` returns 0 results

**Exit Criteria:** Website landing page at `ori-lang.com` displays new positioning. Zero references to "mandatory testing" as a requirement. All metadata (title, description, FAQ schema, OG image, layout defaults) aligned with new messaging. Site builds and renders correctly.
