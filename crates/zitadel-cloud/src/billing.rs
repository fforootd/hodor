// SPDX-License-Identifier: LicenseRef-ZITADEL-Cloud
//
// Stripe integration: subscriptions, usage metering, invoicing.
// Follows the pattern from ADR-030 section 6:
//   persist desired state -> enqueue job -> perform side effects -> record observed state.
