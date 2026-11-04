# Karate business plan

Status: hypothesis, not validated revenue or product-market fit. Initial research: 2026-11-04.

## Positioning

Karate proposes repeatable, explainable security assessments for teams building multi-tenant web/API products. The buyer is an engineering/security lead; the operator is a developer or AppSec engineer. The trigger is a release, enterprise procurement review or recurring security review. Current alternatives include manual pentesting, vendor autonomous pentesting and in-house checks. Proposed advantage: self-hostable source, explicit evidence and gaps, portable model providers, isolated test sessions, and predictable budgets.

## Competitive evidence

| Alternative | Vendor-stated offer | Pricing evidence | Implication |
| --- | --- | --- | --- |
| Aikido | Agent-led discovery, validation, reports, retests and continuous tests. | Standard 4,000 credits; one credit = USD 1. Continuous uses 10 credits per launched agent. See PAGE-040 and PAGE-027. | Strong workflow breadth; Karate must earn trust through measured reliability rather than unsupported superiority. |
| XBOW | Exploit evidence, continuous/on-demand testing and retesting. | No numerical price verified on the inspected homepage; demo-led sales. https://xbow.com/ | Reproducible proof is a baseline customer expectation, not unique positioning. |
| Horizon3 NodeZero | Authenticated web testing with runner support and explicit coverage limitations. | Numerical price not verified. https://docs.horizon3.ai/portal/test_types/webapp/ | Private deployment, credential handling and honest coverage matter. |
| Pentera | Broader validation across internal/external/cloud surfaces and remediation workflows. | Numerical price not verified. https://pentera.io/pentera-platform/ | Broader enterprise validation competes for the same security budget. |
| Manual consultancy/in-house | Human judgment and bespoke scope; quality and cost vary. | No quote collected. | Karate should complement human review on ambiguous/high-impact paths. |

All competitor descriptions are vendor claims, accessed 2026-11-04, not independent performance comparisons. No currency conversions made. Aikido's platform subscription and pentest credits must not be conflated. Tax/region-specific terms require confirmation before comparison.

A public Reddit discussion included positive support/remediation comments and a false-positive complaint, but it concerns the wider platform, is self-selected, and includes vendor participation. It does not prove AI Pentest accuracy or complaint prevalence: https://www.reddit.com/r/devsecops/comments/1pb2a2a/is_aikido_legit_or_a_scam/.

## Proposed packaging

- Community: MIT source, self-operated infrastructure, bring your own providers. No implied support SLA.
- Managed Team hypothesis: USD 299/month, three projects, bounded concurrent runs and usage allowance, evidence exports, all core security/access capabilities.
- Managed Growth hypothesis: USD 899/month, ten projects, higher concurrency and support allocation. Usage beyond allowance requires an explicit spending cap.
- Enterprise: quote after residency, private connectivity and support requirements are known.

These prices are experiments, not published offers. Core isolation, RBAC, accessibility, localization and evidence access must not depend on buying a security add-on. Initial collection workflow can be manually reconciled invoices with audited entitlement grants; no live payment processor is assumed configured.

## Unit economics assumptions (USD/customer/month)

| Scenario | Revenue | Compute/storage/egress | Model/voice APIs | Support/payment | Contribution | Margin |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Light | 299 | 25 | 25 | 35 | 214 | 71.57% |
| Base | 299 | 45 | 70 | 60 | 124 | 41.47% |
| Heavy | 299 | 90 | 180 | 90 | -61 | -20.40% |

Inputs are planning estimates, not provider quotes. At an assumed USD 500 CAC, payback is 2.34/4.03 months for light/base contribution; heavy usage has no positive payback. At USD 5,000 monthly fixed cost, base break-even is 41 customers (ceiling of 5000/124), excluding growth investment. Double model cost cuts base contribution to USD 54. Meter model tokens, voice duration, request counts, elapsed compute and storage; reserve a maximum cost before dispatch. Do not promise profitability from these scenarios.

## Validation and operations

Recruit five design partners with staging apps and permission to test; interviews/outreach require explicit user instruction. Measure onboarding success, time to first verified finding or meaningful coverage report, reproductions accepted by engineers, false-positive triage time, and retest usefulness. Targets to test: 80% configure a run unaided; median setup under 20 minutes; every reported confirmed finding has reproducible evidence; zero cross-scope requests. Compare findings against an authorized seeded benchmark and expert review before making accuracy claims.

Growth hypotheses: developer documentation, open-source adoption, CI integrations and specialist partners. Retention depends on useful release-to-release results, not scan count. Support needs documented auth/connectivity troubleshooting, incident contact, export and deletion. Pricing tests should establish willingness to pay and heavy-tenant cost bounds before public launch.

Risks: evidence reliability, scope escape, credential custody, inference cost, benchmark overfitting and procurement expectations. Geography, hosting region, processor agreements, retention/legal requirements and paid deployment remain unresolved decisions. Do not claim SOC2, ISO, HIPAA or other certification from report generation.
