---
title: "Annex A — Formal grammar"
description: "Ori Language Specification — Annex A (normative)"
order: 100
section: "Annexes"
---

# Annex A (normative) — Formal grammar

This annex defines the complete formal grammar of the Ori language in Extended Backus-Naur Form (EBNF). The notation conventions are defined in Clause 5.

The grammar is maintained in a companion file: [grammar.ebnf](grammar.ebnf)

## A.1 Notation

Productions use the conventions defined in §5.1. Terminal symbols are enclosed in double quotes. Non-terminal symbols use `snake_case`.

## A.2 Lexical grammar

The lexical grammar defines the formation of tokens from source characters. See [grammar.ebnf](grammar.ebnf) § LEXICAL for the complete lexical productions.

## A.3 Syntactic grammar

The syntactic grammar defines the structure of Ori programs in terms of tokens. See [grammar.ebnf](grammar.ebnf) § SYNTACTIC for the complete syntactic productions.
