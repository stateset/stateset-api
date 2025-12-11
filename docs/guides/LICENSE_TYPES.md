# StateSet API Licensing Options

This document explains the licensing model for the StateSet Rust API and outlines common license “editions” used in commerce platforms, similar to how Magento distinguishes between an open‑source core and commercial offerings. It is provided for informational purposes and is not legal advice.

## Why licensing matters for a commerce OS

Commerce backends sit at the center of revenue‑critical workflows. A good license strategy should:

- Encourage broad adoption and contributions.
- Protect the project from being “strip‑mined” into a competing hosted service without reinvestment.
- Keep downstream usage clear for merchants, integrators, and SaaS providers.
- Provide a sustainable path for long‑term maintenance and enterprise support.

Magento’s historical approach is a useful analogue:

- **Magento Open Source**: freely usable, modifiable source code.
- **Magento/Adobe Commerce**: paid license with additional features, SLAs, and cloud services.

StateSet uses a similar “open core with commercial path” pattern, but implemented through a time‑delayed source‑available license.

## 1) Business Source License (BSL) 1.1 — current license

The StateSet API is currently licensed under the **Business Source License 1.1** (see `LICENSE`). BSL is **source‑available**: you can read, modify, and redistribute the code, with a specific restriction intended to prevent unlicensed competitive hosting.

### What BSL allows

- **Internal production use** by merchants and companies running StateSet for their own commerce, supply‑chain, or manufacturing operations.
- **Modification and extension** (forking, adding modules, custom integrations).
- **Redistribution** of the original or modified work, provided the BSL terms are preserved.

### What BSL restricts

- **Offering StateSet as a hosted or embedded service to third parties** (a “StateSet‑as‑a‑Service” or embedded platform) without a commercial agreement.

### Example scenarios

- ✅ A retailer runs StateSet internally to power their storefront and fulfillment.
- ✅ A systems integrator deploys StateSet inside a client’s VPC as part of a project.
- ❌ A vendor runs multi‑tenant StateSet and sells access to many merchants.

If your use case falls into the restricted category, you should pursue a commercial license.

## 2) Change License: Apache License 2.0 — automatic future conversion

BSL includes a **Change Date** after which the work converts to an OSI‑approved open‑source license. For StateSet, the Change License is **Apache License 2.0** on the date specified in `LICENSE`.

Apache 2.0 is a permissive license that:

- Allows commercial use, modification, distribution, and hosting.
- Includes an explicit patent grant.
- Requires preservation of copyright and license notices.

After the Change Date, StateSet will be usable under Apache 2.0 without the BSL hosting restriction.

## 3) Commercial / Enterprise License — for hosted offerings

To support sustainability and protect the community edition, StateSet can offer a separate commercial license for:

- **SaaS providers** who want to host StateSet for third parties.
- **Embedded platform vendors** distributing StateSet as part of a commercial product.
- **Enterprises** requiring specific terms, indemnities, or SLAs.

This mirrors Magento’s “Commerce” tier: the core is available in source form, while commercial terms unlock hosted/enterprise use and additional support.

## 4) Optional edition and module licensing patterns

Many commerce systems evolve toward multiple “license types” or editions. StateSet may adopt any of the following patterns over time:

### Open core + proprietary add‑ons

- **Core API** under BSL → Apache 2.0.
- **Enterprise modules** (advanced multi‑tenant tooling, premium connectors, managed cloud) under a commercial EULA.

### Dual licensing

- Community users stay on BSL/Apache.
- Companies needing different terms buy a commercial license.

### Mixed‑license workspace

Some sub‑components can be permissively licensed (MIT/Apache) if they are intended for reuse beyond the core product, while the main API remains BSL. Any such exceptions should be documented per‑module and in the repository’s licensing notices.

## 5) How this compares to Magento (quick matrix)

| Dimension | Magento Open Source | Adobe Commerce | StateSet (today) | StateSet (after Change Date) |
|---|---|---|---|---|
| Source available | Yes | Partially | Yes | Yes |
| OSI open source | Yes (OSL/AFL) | No | Not yet (BSL) | Yes (Apache 2.0) |
| Internal production use | Allowed | Allowed | Allowed | Allowed |
| SaaS / hosted resale | Allowed | Allowed with subscription | Restricted without commercial license | Allowed |
| Enterprise features & SLAs | No | Yes | Via commercial license | Via commercial license |

## 6) Trademarks and branding

Software licenses govern code rights; **trademarks govern branding**. Even when code becomes Apache 2.0, use of “StateSet” branding in a competing hosted product may still require permission. Trademark policies are typically published separately.

## 7) Guidance for contributors

- Contributions to the core codebase are accepted under the repository license.
- If you contribute new modules, clearly state their license in the module docs if it differs from the repo’s default.
- Avoid importing dependencies with incompatible licenses; see `deny.toml` for policy.

## Questions?

If you’re unsure whether your use case is covered by BSL or needs a commercial license, open an issue or contact the StateSet team. Always consult your legal counsel for definitive interpretation.

