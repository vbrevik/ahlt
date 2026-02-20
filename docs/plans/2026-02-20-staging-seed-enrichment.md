# Staging Seed Enrichment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend `data/seed/staging.json` with two new roles, six users, two new ToRs, nine meetings covering all five statuses, COAs/opinions, and suggestions/proposals at every workflow stage.

**Architecture:** All changes are JSON edits to `data/seed/staging.json`. The import system uses `ConflictMode::Skip` — idempotent. Entities reference each other via name-based `"source": "entity_type:name"` in the relations array. No Rust code changes required.

**Tech Stack:** JSON (staging fixture), SQLite via `data/seed/staging.json` → `db.rs` import on `APP_ENV=staging` startup.

**Design doc:** `docs/plans/2026-02-20-staging-seed-enrichment-design.md`

---

## How to edit `staging.json`

The file has two top-level arrays: `entities` and `relations`. Every task below appends items to one or both. The edit pattern is:

- Find the last item in the target array (e.g., last entity before `]`)
- Add a comma after it, then paste the new items
- Keep `]` closing bracket in place

Verify JSON validity after every task: `python3 -m json.tool data/seed/staging.json > /dev/null && echo OK`

---

## Task 1: Add two new roles

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add role entities**

In the `entities` array, after the last existing entity (the `agenda_point:agenda_2026_02_25_br` block), append before the closing `]`:

```json
,
{
  "entity_type": "role",
  "name": "secretary",
  "label": "Secretary",
  "sort_order": 6,
  "properties": {
    "description": "Records meeting minutes and manages documentation"
  }
},
{
  "entity_type": "role",
  "name": "governance_officer",
  "label": "Governance Officer",
  "sort_order": 7,
  "properties": {
    "description": "Manages governance processes, ToRs, and proposal approvals"
  }
}
```

**Step 2: Add role permissions**

In the `relations` array, after the last existing relation, append before the closing `]`:

```json
,
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:dashboard.view" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:tor.list" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:agenda.view" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:meetings.view" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:minutes.generate" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:minutes.edit" },
{ "relation_type": "has_permission", "source": "role:secretary", "target": "permission:minutes.approve" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:dashboard.view" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:tor.list" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:tor.create" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:tor.edit" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:tor.manage_members" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:suggestion.view" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:suggestion.review" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:proposal.view" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:proposal.review" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:proposal.approve" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:agenda.view" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:agenda.manage" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:agenda.decide" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:meetings.view" },
{ "relation_type": "has_permission", "source": "role:governance_officer", "target": "permission:minutes.approve" }
```

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

Expected: `OK`

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add secretary and governance_officer roles with permissions"
```

---

## Task 2: Add six new users

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add user entities**

Append to `entities` array (after the roles added in Task 1):

```json
,
{
  "entity_type": "user",
  "name": "eva",
  "label": "Eva Secretary",
  "sort_order": 0,
  "properties": { "email": "eva@example.com" }
},
{
  "entity_type": "user",
  "name": "frank",
  "label": "Frank Ops",
  "sort_order": 0,
  "properties": { "email": "frank@example.com" }
},
{
  "entity_type": "user",
  "name": "grace",
  "label": "Grace Lead",
  "sort_order": 0,
  "properties": { "email": "grace@example.com" }
},
{
  "entity_type": "user",
  "name": "henry",
  "label": "Henry Officer",
  "sort_order": 0,
  "properties": { "email": "henry@example.com" }
},
{
  "entity_type": "user",
  "name": "irene",
  "label": "Irene Manager",
  "sort_order": 0,
  "properties": { "email": "irene@example.com" }
},
{
  "entity_type": "user",
  "name": "jack",
  "label": "Jack Member",
  "sort_order": 0,
  "properties": { "email": "jack@example.com" }
}
```

**Step 2: Add has_role relations**

Append to `relations` array (after role permissions from Task 1):

```json
,
{ "relation_type": "has_role", "source": "user:eva", "target": "role:secretary" },
{ "relation_type": "has_role", "source": "user:frank", "target": "role:viewer" },
{ "relation_type": "has_role", "source": "user:grace", "target": "role:editor" },
{ "relation_type": "has_role", "source": "user:grace", "target": "role:manager" },
{ "relation_type": "has_role", "source": "user:henry", "target": "role:governance_officer" },
{ "relation_type": "has_role", "source": "user:irene", "target": "role:manager" },
{ "relation_type": "has_role", "source": "user:jack", "target": "role:viewer" }
```

Note: `grace` gets two `has_role` relations — this exercises the permission-union code path.

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add 6 new users (eva, frank, grace, henry, irene, jack)"
```

---

## Task 3: Add IT Governance Board and Change Advisory Board ToRs

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add ToR entities and functions**

Append to `entities` array:

```json
,
{
  "entity_type": "tor",
  "name": "it_governance_board",
  "label": "IT Governance Board",
  "sort_order": 8,
  "properties": {
    "meeting_cadence": "monthly",
    "cadence_day": "wednesday",
    "cadence_time": "14:00",
    "cadence_duration_minutes": "120",
    "status": "active",
    "description": "Oversees IT strategy, architecture decisions, and technology investments",
    "default_location": "IT Hub Conference Room",
    "tor_number": "201",
    "classification": "INTERNAL",
    "version": "1.0",
    "organization": "IT Division",
    "focus_scope": "IT strategy, architecture governance, technology investment decisions, and vendor management",
    "objectives": "[\"Approve major IT architecture decisions\",\"Oversee technology investment portfolio\",\"Ensure alignment of IT initiatives with organizational strategy\"]",
    "inputs_required": "[\"Architecture proposals\",\"Technology investment requests\",\"Vendor assessment reports\"]",
    "outputs_expected": "[\"Approved architecture decisions\",\"Technology investment approvals\",\"IT strategy updates\"]",
    "poc_contact": "CTO Office, ext. 3100",
    "info_platform": "IT Governance Portal"
  }
},
{
  "entity_type": "tor_function",
  "name": "igb_chair",
  "label": "Chair",
  "sort_order": 0,
  "properties": { "membership_type": "mandatory" }
},
{
  "entity_type": "tor_function",
  "name": "igb_secretary",
  "label": "Secretary",
  "sort_order": 1,
  "properties": { "membership_type": "mandatory" }
},
{
  "entity_type": "tor_function",
  "name": "igb_member",
  "label": "Member",
  "sort_order": 2,
  "properties": { "membership_type": "optional" }
},
{
  "entity_type": "tor",
  "name": "change_advisory_board",
  "label": "Change Advisory Board",
  "sort_order": 9,
  "properties": {
    "meeting_cadence": "biweekly",
    "cadence_day": "thursday",
    "cadence_time": "10:00",
    "cadence_duration_minutes": "60",
    "status": "active",
    "description": "Reviews and approves significant changes to IT systems and infrastructure",
    "default_location": "Operations Room 2",
    "tor_number": "202",
    "classification": "INTERNAL",
    "version": "1.2",
    "organization": "IT Division",
    "focus_scope": "Change risk assessment, approval of significant IT changes, post-implementation review",
    "objectives": "[\"Review and approve change requests above risk threshold\",\"Assess impact and risk of proposed IT changes\",\"Conduct post-implementation reviews for major changes\"]",
    "inputs_required": "[\"Change requests from project teams\",\"Risk assessments\",\"Impact analysis reports\"]",
    "outputs_expected": "[\"Change approval decisions\",\"Implementation guidance\",\"Risk mitigation requirements\"]",
    "poc_contact": "Change Management Office, change@it.example.com",
    "info_platform": "ITSM Platform"
  }
},
{
  "entity_type": "tor_function",
  "name": "cab_chair",
  "label": "Change Manager",
  "sort_order": 0,
  "properties": { "membership_type": "mandatory" }
},
{
  "entity_type": "tor_function",
  "name": "cab_technical_lead",
  "label": "Technical Lead",
  "sort_order": 1,
  "properties": { "membership_type": "mandatory" }
},
{
  "entity_type": "tor_function",
  "name": "cab_member",
  "label": "Advisory Member",
  "sort_order": 2,
  "properties": { "membership_type": "optional" }
}
```

**Step 2: Add ToR relations (positions and cross-domain)**

Append to `relations` array:

```json
,
{ "relation_type": "belongs_to_tor", "source": "tor_function:igb_chair", "target": "tor:it_governance_board" },
{ "relation_type": "fills_position", "source": "user:alice", "target": "tor_function:igb_chair" },
{ "relation_type": "belongs_to_tor", "source": "tor_function:igb_secretary", "target": "tor:it_governance_board" },
{ "relation_type": "fills_position", "source": "user:eva", "target": "tor_function:igb_secretary" },
{ "relation_type": "belongs_to_tor", "source": "tor_function:igb_member", "target": "tor:it_governance_board" },
{ "relation_type": "fills_position", "source": "user:charlie", "target": "tor_function:igb_member" },
{ "relation_type": "belongs_to_tor", "source": "tor_function:cab_chair", "target": "tor:change_advisory_board" },
{ "relation_type": "fills_position", "source": "user:henry", "target": "tor_function:cab_chair" },
{ "relation_type": "belongs_to_tor", "source": "tor_function:cab_technical_lead", "target": "tor:change_advisory_board" },
{ "relation_type": "belongs_to_tor", "source": "tor_function:cab_member", "target": "tor:change_advisory_board" },
{ "relation_type": "fills_position", "source": "user:grace", "target": "tor_function:cab_member" },
{
  "relation_type": "escalates_to",
  "source": "tor:budget_committee",
  "target": "tor:it_governance_board",
  "properties": {
    "description": "Financial technology investment decisions escalate to IT Governance Board"
  }
},
{
  "relation_type": "feeds_into",
  "source": "tor:change_advisory_board",
  "target": "tor:it_governance_board",
  "properties": {
    "description": "Approved changes and architectural impacts feed into IT Governance Board review",
    "output_types": "approved change records, architectural impact reports"
  }
}
```

**Note on the vacant position:** `cab_technical_lead` has a `belongs_to_tor` relation but **no** `fills_position` relation. This is intentional — it will trigger the vacancy warning generator on the next scheduler tick.

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add IT Governance Board and Change Advisory Board ToRs"
```

---

## Task 4: Add nine meetings (all five statuses)

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add meeting entities**

Append to `entities` array:

```json
,
{
  "entity_type": "meeting",
  "name": "igb_meeting_2026_01_21",
  "label": "IT Governance Board — 2026-01-21",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-01-21",
    "status": "completed",
    "location": "IT Hub Conference Room",
    "notes": "Architecture review completed. Cloud platform selection deferred to Q1 board.",
    "meeting_number": "IGB-2026-01"
  }
},
{
  "entity_type": "meeting",
  "name": "igb_meeting_2026_02_18",
  "label": "IT Governance Board — 2026-02-18",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-02-18",
    "status": "completed",
    "location": "IT Hub Conference Room",
    "notes": "Reviewed Q4 IT spend. Cloud platform options presented for decision next session.",
    "meeting_number": "IGB-2026-02"
  }
},
{
  "entity_type": "meeting",
  "name": "igb_meeting_2026_03_18",
  "label": "IT Governance Board — 2026-03-18",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-03-18",
    "status": "confirmed",
    "location": "IT Hub Conference Room",
    "notes": "",
    "meeting_number": "IGB-2026-03"
  }
},
{
  "entity_type": "meeting",
  "name": "igb_meeting_2026_04_15",
  "label": "IT Governance Board — 2026-04-15",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-04-15",
    "status": "projected",
    "location": "IT Hub Conference Room",
    "notes": "",
    "meeting_number": "IGB-2026-04"
  }
},
{
  "entity_type": "meeting",
  "name": "cab_meeting_2026_02_06",
  "label": "Change Advisory Board — 2026-02-06",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-02-06",
    "status": "completed",
    "location": "Operations Room 2",
    "notes": "Approved 3 standard changes. 1 emergency change post-review completed.",
    "meeting_number": "CAB-2026-03"
  }
},
{
  "entity_type": "meeting",
  "name": "cab_meeting_2026_02_13",
  "label": "Change Advisory Board — 2026-02-13",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-02-13",
    "status": "cancelled",
    "location": "Operations Room 2",
    "notes": "Cancelled due to insufficient quorum — Technical Lead position vacant.",
    "meeting_number": "CAB-2026-04"
  }
},
{
  "entity_type": "meeting",
  "name": "cab_meeting_2026_02_20",
  "label": "Change Advisory Board — 2026-02-20",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-02-20",
    "status": "in_progress",
    "location": "Operations Room 2",
    "notes": "",
    "meeting_number": "CAB-2026-05"
  }
},
{
  "entity_type": "meeting",
  "name": "cab_meeting_2026_03_05",
  "label": "Change Advisory Board — 2026-03-05",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-03-05",
    "status": "projected",
    "location": "Operations Room 2",
    "notes": "",
    "meeting_number": "CAB-2026-06"
  }
},
{
  "entity_type": "meeting",
  "name": "bc_meeting_2026_01_08",
  "label": "Budget Committee — 2026-01-08",
  "sort_order": 0,
  "properties": {
    "meeting_date": "2026-01-08",
    "status": "completed",
    "location": "Conference Room A",
    "notes": "Q4 2025 budget review completed. Procurement policy discussion tabled for February.",
    "meeting_number": "BC-2026-01"
  }
}
```

Status coverage after this task:
- `projected` — igb_meeting_2026_04_15, cab_meeting_2026_03_05
- `confirmed` — igb_meeting_2026_03_18
- `in_progress` — cab_meeting_2026_02_20
- `completed` — igb x2, cab x1, bc x1
- `cancelled` — cab_meeting_2026_02_13

**Step 2: Add belongs_to_tor relations for meetings**

Append to `relations` array:

```json
,
{ "relation_type": "belongs_to_tor", "source": "meeting:igb_meeting_2026_01_21", "target": "tor:it_governance_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:igb_meeting_2026_02_18", "target": "tor:it_governance_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:igb_meeting_2026_03_18", "target": "tor:it_governance_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:igb_meeting_2026_04_15", "target": "tor:it_governance_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:cab_meeting_2026_02_06", "target": "tor:change_advisory_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:cab_meeting_2026_02_13", "target": "tor:change_advisory_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:cab_meeting_2026_02_20", "target": "tor:change_advisory_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:cab_meeting_2026_03_05", "target": "tor:change_advisory_board" },
{ "relation_type": "belongs_to_tor", "source": "meeting:bc_meeting_2026_01_08", "target": "tor:budget_committee" }
```

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add 9 meetings covering all 5 workflow statuses"
```

---

## Task 5: Add agenda point, COAs, and opinions for IGB March meeting

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add agenda point, COA, and opinion entities**

Append to `entities` array:

```json
,
{
  "entity_type": "agenda_point",
  "name": "agenda_igb_2026_03_18_cloud",
  "label": "Select Cloud Platform for 2026",
  "sort_order": 0,
  "properties": {
    "title": "Select Cloud Platform for 2026",
    "item_type": "decision",
    "status": "scheduled",
    "time_allocation_minutes": "45",
    "scheduled_date": "2026-03-18",
    "description": "Review and select primary cloud platform for 2026 IT infrastructure migration. Choose between Azure and AWS based on cost analysis, capability assessment, and strategic fit.",
    "created_date": "2026-02-20T14:00:00",
    "priority": "high"
  }
},
{
  "entity_type": "coa",
  "name": "coa_igb_cloud_azure",
  "label": "Adopt Azure Cloud Platform",
  "sort_order": 1,
  "properties": {
    "description": "Migrate primary IT infrastructure to Microsoft Azure. Leverages existing M365 integration, enterprise agreement pricing, and Azure AD identity management.",
    "type": "simple"
  }
},
{
  "entity_type": "coa",
  "name": "coa_igb_cloud_aws",
  "label": "Adopt AWS Cloud Platform",
  "sort_order": 2,
  "properties": {
    "description": "Migrate primary IT infrastructure to Amazon Web Services. Offers broader service portfolio, stronger developer tooling, and competitive spot instance pricing.",
    "type": "simple"
  }
},
{
  "entity_type": "opinion",
  "name": "opinion_alice_cloud_2026",
  "label": "Alice — Cloud Platform Opinion",
  "sort_order": 0,
  "properties": {
    "stance": "support",
    "rationale": "Azure integration with our existing M365 and AD infrastructure significantly reduces migration complexity and operational overhead."
  }
},
{
  "entity_type": "opinion",
  "name": "opinion_henry_cloud_2026",
  "label": "Henry — Cloud Platform Opinion",
  "sort_order": 0,
  "properties": {
    "stance": "support",
    "rationale": "AWS provides superior developer tooling and more flexible pricing for our workload mix. The broader service catalogue future-proofs our architecture."
  }
}
```

**Step 2: Add relations for agenda point, COAs, opinions**

Append to `relations` array:

```json
,
{ "relation_type": "belongs_to_tor", "source": "agenda_point:agenda_igb_2026_03_18_cloud", "target": "tor:it_governance_board" },
{ "relation_type": "considers_coa", "source": "agenda_point:agenda_igb_2026_03_18_cloud", "target": "coa:coa_igb_cloud_azure" },
{ "relation_type": "considers_coa", "source": "agenda_point:agenda_igb_2026_03_18_cloud", "target": "coa:coa_igb_cloud_aws" },
{ "relation_type": "opinion_by", "source": "opinion:opinion_alice_cloud_2026", "target": "user:alice" },
{ "relation_type": "opinion_on", "source": "opinion:opinion_alice_cloud_2026", "target": "agenda_point:agenda_igb_2026_03_18_cloud" },
{ "relation_type": "prefers_coa", "source": "opinion:opinion_alice_cloud_2026", "target": "coa:coa_igb_cloud_azure" },
{ "relation_type": "opinion_by", "source": "opinion:opinion_henry_cloud_2026", "target": "user:henry" },
{ "relation_type": "opinion_on", "source": "opinion:opinion_henry_cloud_2026", "target": "agenda_point:agenda_igb_2026_03_18_cloud" },
{ "relation_type": "prefers_coa", "source": "opinion:opinion_henry_cloud_2026", "target": "coa:coa_igb_cloud_aws" }
```

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add IGB agenda point with COAs and opinions"
```

---

## Task 6: Add suggestions and proposals (all statuses)

**Files:**
- Modify: `data/seed/staging.json`

**Step 1: Add suggestion and proposal entities**

Append to `entities` array:

```json
,
{
  "entity_type": "suggestion",
  "name": "suggestion_rejected_igb",
  "label": "Standardise on a single video conferencing platform",
  "sort_order": 0,
  "properties": {
    "submitted_date": "2026-01-10",
    "description": "We currently use Teams, Zoom, and Meet depending on the team. Standardising on one platform would reduce licence costs and user confusion.",
    "status": "rejected"
  }
},
{
  "entity_type": "suggestion",
  "name": "suggestion_open_cab",
  "label": "Introduce automated rollback for failed deployments",
  "sort_order": 0,
  "properties": {
    "submitted_date": "2026-02-14",
    "description": "Implement automated rollback triggers in the deployment pipeline so failed production deployments revert within 5 minutes without manual CAB intervention.",
    "status": "open"
  }
},
{
  "entity_type": "proposal",
  "name": "proposal_under_review_igb",
  "label": "Unified API Gateway Policy",
  "sort_order": 0,
  "properties": {
    "title": "Unified API Gateway Policy",
    "submitted_date": "2026-02-01",
    "status": "under_review",
    "description": "Mandate all internal service-to-service communication routes through a central API gateway with rate limiting, authentication, and audit logging.",
    "rationale": "Current direct service calls bypass security controls. 3 incidents in Q4 2025 traced to unaudited API calls."
  }
},
{
  "entity_type": "proposal",
  "name": "proposal_approved_igb",
  "label": "Zero-Trust Network Architecture",
  "sort_order": 0,
  "properties": {
    "title": "Zero-Trust Network Architecture",
    "submitted_date": "2025-11-15",
    "status": "approved",
    "description": "Implement zero-trust network segmentation across all environments. No implicit trust based on network location.",
    "rationale": "Security audit identified lateral movement risk. Zero-trust eliminates the assumption that internal traffic is safe."
  }
},
{
  "entity_type": "proposal",
  "name": "proposal_rejected_cab",
  "label": "Remove CAB approval for low-risk changes",
  "sort_order": 0,
  "properties": {
    "title": "Remove CAB approval for low-risk changes",
    "submitted_date": "2026-01-20",
    "status": "rejected",
    "description": "Eliminate CAB review requirement for changes classified as low-risk per the risk matrix, relying solely on automated testing gates.",
    "rationale": "CAB process adds 5-7 days to low-risk deployments. Automated gates already catch regression. Streamlining would cut cycle time significantly."
  }
}
```

Status coverage after this task:
- Suggestions: `open` (×4 total), `accepted` (×2 total), `rejected` (×1 new)
- Proposals: `draft` (×1), `submitted` (×2), `under_review` (×1 new), `approved` (×1 new), `rejected` (×1 new)

**Step 2: Add suggestion/proposal relations**

Append to `relations` array:

```json
,
{ "relation_type": "suggested_to", "source": "suggestion:suggestion_rejected_igb", "target": "tor:it_governance_board" },
{ "relation_type": "suggested_to", "source": "suggestion:suggestion_open_cab", "target": "tor:change_advisory_board" },
{ "relation_type": "submitted_to", "source": "proposal:proposal_under_review_igb", "target": "tor:it_governance_board" },
{ "relation_type": "submitted_to", "source": "proposal:proposal_approved_igb", "target": "tor:it_governance_board" },
{ "relation_type": "submitted_to", "source": "proposal:proposal_rejected_cab", "target": "tor:change_advisory_board" }
```

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```

**Step 4: Commit**

```bash
git add data/seed/staging.json
git commit -m "feat(seed): add suggestions and proposals covering all workflow statuses"
```

---

## Task 7: Re-seed and verify

**Step 1: Delete staging DB**

```bash
rm -f data/staging/app.db
```

**Step 2: Start server with staging data**

```bash
APP_ENV=staging cargo run
```

Watch startup output. Expected: no errors, seed import completes, server starts on port 8080.

**Step 3: Log in and verify each entity type**

Open http://localhost:8080 and log in as `admin` / `admin123`.

Checklist:
- [ ] Users list (`/users`): 10 users visible (admin + alice + bob + charlie + diana + eva + frank + grace + henry + irene + jack)
- [ ] Roles list (`/roles`): `secretary` and `governance_officer` visible
- [ ] grace shows two roles in the users table
- [ ] ToR list (`/tor`): `IT Governance Board` and `Change Advisory Board` visible
- [ ] Governance map (`/governance/map`): IGB + CAB nodes visible, cross-domain edges (escalates_to, feeds_into) rendered
- [ ] Meeting Outlook (`/tor/outlook`): IGB and CAB meetings appear on calendar
- [ ] IGB ToR detail: 4 meetings visible, `cab_technical_lead` vacancy warning triggered
- [ ] CAB ToR detail: meetings show all 5 statuses
- [ ] IGB March meeting: agenda point "Select Cloud Platform for 2026" with 2 COAs
- [ ] Workflow page (`/workflow`): 3 suggestions (rejected visible), 5 proposals across all stages

**Step 4: Commit if any tweaks were needed**

```bash
git add data/seed/staging.json
git commit -m "fix(seed): correct any issues found during verification"
```

---

## Status Coverage Summary

| Entity | Statuses covered |
|--------|-----------------|
| `meeting` | projected, confirmed, in_progress, completed, cancelled |
| `suggestion` | open, accepted, rejected |
| `proposal` | draft, submitted, under_review, approved, rejected |
| `agenda_point` | scheduled |
| `tor_function` | filled (×8), vacant mandatory (×1) |

## Entity Types Covered

`role`, `user`, `tor`, `tor_function`, `meeting`, `agenda_point`, `coa`, `opinion`, `suggestion`, `proposal`, `permission` (existing), `nav_item` (existing), `workflow_status` (existing), `workflow_transition` (existing)
