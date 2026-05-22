# MkDocs Material with Diátaxis Documentation Framework

* Status: accepted
* Date: 2026-05-18
* Deciders: @hefarica
* Consulted: D-LEXICON
* Informed: All team members

## Context and Problem Statement

Project documentation was fragmented across READMEs, inline comments, and informal notes. New team members struggled to find information, and there was no single source of truth for operational procedures, API documentation, or architectural decisions.

## Decision Drivers

* Single documentation source
* Four distinct documentation types need different structures
* Must support Mermaid diagrams
* Must support versioning

## Considered Options

* Option A: README-only — simple but scales poorly
* Option B: Docusaurus — React-based, complex setup
* Option C: MkDocs Material + Diátaxis — proven, simple, structured

## Decision Outcome

Chosen option: "Option C — MkDocs Material + Diátaxis", because it provides the four-quadrant structure with minimal setup.

### Consequences

* Good, because four clear documentation types
* Good, because MkDocs is simple to maintain
* Good, because Material theme is professional
* Bad, because requires Python for builds
* Bad, because not as interactive as Docusaurus

## Validation

Documentation structure:
- Tutorials: Learning-oriented, step-by-step
- How-To Guides: Task-oriented, goal-driven
- Reference: Information-oriented, lookup
- Explanation: Understanding-oriented, deep dives

## More Information

* Diátaxis: https://diataxis.fr/
* MkDocs Material: https://squidfunk.github.io/mkdocs-material/
