# Staging Seed Enrichment — Design

**Date:** 2026-02-20
**Goal:** Enrich `data/seed/staging.json` with richer demo data that covers every entity type, every status variant, and deliberate edge cases for dev/test purposes.

## Approach

Extend the existing Agile/Finance/Safety domain with a second organizational cluster (IT Governance Board + Change Advisory Board), then populate both old and new ToRs with data that hits every status code and entity type.

---

## Roles & Permissions

Two new roles added to `staging.json`:

| Role | Key permissions |
|------|----------------|
| `secretary` | `minutes.generate`, `minutes.edit`, `minutes.approve`, `tor.list`, `agenda.view`, `meetings.view`, `dashboard.view` |
| `governance_officer` | `tor.list`, `tor.create`, `tor.edit`, `tor.manage_members`, `suggestion.review`, `proposal.view`, `proposal.review`, `proposal.approve`, `agenda.view`, `agenda.manage`, `agenda.decide`, `minutes.approve`, `meetings.view`, `dashboard.view` |

---

## Users

Six new users, total grows from 4 to 10:

| name | label | roles | notes |
|------|-------|-------|-------|
| `eva` | Eva Secretary | secretary | |
| `frank` | Frank Ops | viewer | |
| `grace` | Grace Lead | editor + manager | multi-role: tests permission union |
| `henry` | Henry Officer | governance_officer | |
| `irene` | Irene Manager | manager | |
| `jack` | Jack Member | viewer | |

---

## ToRs & Positions

### IT Governance Board (`it_governance_board`)
- Cadence: monthly, 3rd Wednesday, 14:00, 120 min
- Functions:
  - `igb_chair` (mandatory) → alice
  - `igb_secretary` (mandatory) → eva
  - `igb_member` (optional) → charlie

### Change Advisory Board (`change_advisory_board`)
- Cadence: biweekly, Thursday, 10:00, 60 min
- Functions:
  - `cab_chair` (mandatory) → henry
  - `cab_technical_lead` (mandatory) → **VACANT** — intentional, triggers vacancy warning generator
  - `cab_member` (optional) → grace

### Cross-domain relations
- `budget_committee` `escalates_to` `it_governance_board`
- `change_advisory_board` `feeds_into` `it_governance_board`

---

## Meetings

Covers all 5 workflow statuses: `projected`, `confirmed`, `in_progress`, `completed`, `cancelled`.

| entity name | ToR | date | status |
|-------------|-----|------|--------|
| `igb_meeting_2026_01_21` | it_governance_board | 2026-01-21 | completed |
| `igb_meeting_2026_02_18` | it_governance_board | 2026-02-18 | completed |
| `igb_meeting_2026_03_18` | it_governance_board | 2026-03-18 | confirmed |
| `igb_meeting_2026_04_15` | it_governance_board | 2026-04-15 | projected |
| `cab_meeting_2026_02_06` | change_advisory_board | 2026-02-06 | completed |
| `cab_meeting_2026_02_13` | change_advisory_board | 2026-02-13 | cancelled |
| `cab_meeting_2026_02_20` | change_advisory_board | 2026-02-20 | in_progress |
| `cab_meeting_2026_03_05` | change_advisory_board | 2026-03-05 | projected |
| `bc_meeting_2026_01_08` | budget_committee | 2026-01-08 | completed |

---

## COAs & Opinions

Agenda point on the IGB March confirmed meeting (`igb_meeting_2026_03_18`):
- Label: "Select Cloud Platform for 2026"
- Type: `decision`

Two Courses of Action:
- `coa_igb_cloud_azure` — "Adopt Azure Cloud Platform" (simple)
- `coa_igb_cloud_aws` — "Adopt AWS Cloud Platform" (simple)

Two Opinions:
- alice: opinion on agenda point, prefers Azure COA
- henry: opinion on agenda point, prefers AWS COA

Relations used: `considers_coa`, `opinion_by`, `opinion_on`, `prefers_coa`

---

## Suggestions & Proposals

Covers all status variants across both entity types.

### Suggestions
| name | status | target ToR |
|------|--------|-----------|
| `suggestion_rejected_igb` | rejected | it_governance_board |
| `suggestion_open_cab` | open | change_advisory_board |

(existing staging.json already has: open ×3, accepted ×2)

### Proposals
| name | status | target ToR |
|------|--------|-----------|
| `proposal_under_review_igb` | under_review | it_governance_board |
| `proposal_approved_igb` | approved | it_governance_board |
| `proposal_rejected_cab` | rejected | change_advisory_board |

(existing staging.json already has: draft ×1, submitted ×2)

---

## Edge Cases Deliberately Covered

| Edge case | How |
|-----------|-----|
| Vacant mandatory position | `cab_technical_lead` has no `fills_position` relation |
| Multi-role user | grace has both `editor` and `manager` roles |
| All meeting statuses | 5 meetings across IGB + CAB |
| All suggestion statuses | open, accepted, rejected |
| All proposal statuses | draft, submitted, under_review, approved, rejected |
| COA + opinion chain | IGB March agenda decision item |
| Cross-domain ToR graph | escalates_to + feeds_into between domains |
