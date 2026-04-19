# ADR-NNNN: <imperative verb phrase that names the decision>

- **Status:** Proposed | Accepted | Superseded by ADR-NNNN | Deprecated
- **Date:** YYYY-MM-DD
- **Deciders:** <names or roles>
- **Supersedes:** ADR-NNNN (omit if none)

## Context

What problem are we solving? What forces are at play (technical, business,
team, external)? What's the relevant current state? What constraints
apply? Be concrete. Cite source paths, RFCs, vendor docs, or prior ADRs by
number when relevant.

Aim for the reader who joins the project six months from now and has no
memory of the conversation.

## Decision

State the chosen option in one or two sentences at the top. Then expand
into the design as needed.

> Example: We validate inbound bearer tokens on the API gateway via Zitadel's
> OIDC Token Introspection endpoint, using the `zitadel` crate's
> `IntrospectedUser` actix extractor with JWT Profile authentication.

## Consequences

What becomes easier as a result of this decision? What becomes harder?
What's now locked in? What follow-up work is implied?

- **Positive:** ...
- **Negative / accepted trade-offs:** ...
- **Locked in:** ...
- **Follow-up:** ... (if any)

## Alternatives considered

List at least one other option that was rejected, with the reason. If
genuinely no alternatives existed, say so and explain why.

### Option A — <name>

Brief description, why considered, why rejected.

### Option B — <name>

Same.

## Implementation notes (optional)

Pointers to where this decision lives in the codebase. Sketches of API
shape or contract are welcome here, marked `<!-- sketch -->`. Do NOT paste
full implementation; that belongs in source.

```rust
<!-- sketch -->
// Illustrative: the actix extractor pattern this decision relies on.
async fn protected(user: IntrospectedUser) -> impl Responder { ... }
```

## References

- Source code: `apps/<service>/src/<file>.rs:<line>`
- External docs: <links>
- Originated from: `plans/NNN-<slug>.md` at commit `<sha>` (if extracted
  from a legacy plan)
