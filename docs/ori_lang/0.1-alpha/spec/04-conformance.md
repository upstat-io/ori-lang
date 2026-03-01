---
title: "Conformance"
description: "Ori Language Specification — Clause 4: Conformance"
order: 4
section: "Preliminary"
---

# 4 Conformance

## 4.1 Conforming implementation

A _conforming implementation_ of Ori is one that accepts and correctly executes any conforming program, subject only to resource limitations.

A conforming implementation shall:

- accept all conforming programs;
- reject all non-conforming programs with appropriate diagnostics;
- produce the behavior specified by this document for all conforming programs.

## 4.2 Conforming program

A _conforming program_ is one that is accepted by a conforming implementation and whose behavior is fully defined by this document.

## 4.3 Extensions

A conforming implementation may have extensions, provided they do not alter the behavior of any conforming program.

## 4.4 Implementation-defined behavior

Certain aspects of this specification are described as _implementation-defined_. A conforming implementation shall document its choices for all implementation-defined behavior.

## 4.5 Undefined behavior

Ori is designed to minimize undefined behavior. Where this document does not specify behavior, a conforming implementation shall either:

- reject the program with a diagnostic; or
- produce a panic at run time.

Silent undefined behavior is not permitted in a conforming implementation.
