# Budget Proposal Workflow — E2E Test Scenario

**Status**: Planned for Phase 2+ testing
**Date Created**: 2026-02-14
**Dependencies**: Phase 2 (Item Pipeline), Phase 3 (Meetings), Phase 4 (Calendar)

## Scenario Overview

Multi-department budget proposal workflow demonstrating the full ToR governance cycle with suggestions, proposals, reviews, and ad-hoc meetings.

## Workflow Steps

### 1. Pre-Planning Phase
- **Finance Department** provides current budget status to IT and HR departments
- IT and HR review financial situation and future expectations

### 2. Departmental Working Group Phase

#### IT Department
- **IT Working Group Meeting**: Team collaborates to draft budget proposal
  - Analysts present cost estimates
  - Discussion of priorities and justifications
  - Draft proposal created
- **IT Department Board Meeting**: Reviews and approves draft
  - Department board reviews working group's proposal
  - Makes internal decision to approve
  - Authorizes submission to Finance Board

#### HR Department
- **HR Working Group Meeting**: Team collaborates to draft budget proposal
  - Analysts present staffing needs and costs
  - Discussion of hiring priorities
  - Draft proposal created
- **HR Department Board Meeting**: Reviews and approves draft
  - Department board reviews working group's proposal
  - Makes internal decision to approve
  - Authorizes submission to Finance Board

**Key feature**: Departments can view each other's proposals during working phase (transparency)

### 3. Proposal Submission Phase
- **IT Department** submits approved budget proposal to Finance Board
- **HR Department** submits approved budget proposal to Finance Board
- Both proposals enter the Finance Board's item pipeline

### 4. Finance Board Review Meeting
- Finance Board reviews both proposals in scheduled meeting
- **IT Proposal**: Accepted
- **HR Proposal**: Rejected, sent back for rework with feedback

### 5. HR Rework Phase
- HR Department receives rejection feedback
- **HR Working Group Meeting (2nd round)**: Revises proposal based on Finance Board feedback
- **HR Department Board Meeting (2nd round)**: Approves revised proposal
- Resubmits to Finance Board

### 6. Finance Board Ad-hoc Meeting
- Finance Board schedules special ad-hoc meeting for HR budget decision
- Reviews revised HR proposal
- Makes final decision

## Data Elements to Mock

When implementing this test scenario, the following data needs to be created:

### Organizations/ToRs
- [ ] IT Working Group (ToR for drafting IT proposals)
- [ ] IT Department Board (ToR for internal IT decisions)
- [ ] HR Working Group (ToR for drafting HR proposals)
- [ ] HR Department Board (ToR for internal HR decisions)
- [ ] Finance Board (main decision-making ToR for budget approval)
- [ ] Finance Department (provider of current status)

### Users
- [ ] IT Department representative(s)
- [ ] HR Department representative(s)
- [ ] Finance Department representative(s)
- [ ] Finance Board members (decision makers)
- [ ] Finance Board chair (with elevated authority)

### ToR Functions/Roles
- [ ] Proposal submitter (can create proposals)
- [ ] Proposal reviewer (can review and accept/reject)
- [ ] Meeting caller (can schedule ad-hoc meetings)
- [ ] Status provider (can submit informative items)

### Item Pipeline Data
- [ ] Suggestion: "IT Budget 2026" → Proposal
- [ ] Suggestion: "HR Budget 2026" → Proposal
- [ ] Informative item: "Current Budget Status Q4 2025" (from Finance Dept)
- [ ] Proposal: "IT Department Budget Proposal 2026" (status: approved)
- [ ] Proposal: "HR Department Budget Proposal 2026 v1" (status: rejected)
- [ ] Proposal: "HR Department Budget Proposal 2026 v2" (status: under review)

### Meetings
- [ ] IT Working Group meeting (draft proposal creation)
- [ ] IT Department Board meeting (internal approval)
- [ ] HR Working Group meeting #1 (initial draft proposal)
- [ ] HR Department Board meeting #1 (internal approval)
- [ ] Regular Finance Board meeting (reviews IT + HR proposals)
- [ ] HR Working Group meeting #2 (rework after rejection)
- [ ] HR Department Board meeting #2 (approve revised proposal)
- [ ] Ad-hoc Finance Board meeting (HR budget final decision)

### Meeting Agenda Points
- [ ] Informative: Current Budget Status (Finance Dept)
- [ ] Decision: IT Budget Proposal (approved)
- [ ] Decision: HR Budget Proposal v1 (rejected, reason: "Needs cost justification for 3 new positions")
- [ ] Decision: HR Budget Proposal v2 (TBD in ad-hoc meeting)

## Clarification Questions to Resolve

Before implementing, clarify:

1. **ToR Structure**: Should IT/HR/Finance Dept be separate ToRs, or member groupings within Finance Board?
2. **Visibility Rules**: How exactly can departments see each other's proposals? Read-only? Comment?
3. **User Count**: Minimal (3-5), realistic (8-12), or comprehensive (15+)?
4. **Meeting Cadence**: Finance Board regular meetings monthly, quarterly, or ad-hoc only?
5. **Proposal Versioning**: Should rejected proposals create new entities or update status?
6. **Authority Model**: Who can accept/reject? Board members vote or chair decides?

## Expected Test Coverage

This scenario tests:
- ✅ Multiple ToRs with overlapping members
- ✅ Hierarchical decision-making (Working Group → Dept Board → Finance Board)
- ✅ Suggestion → Proposal → Agenda Point pipeline
- ✅ Proposal acceptance and rejection workflows
- ✅ Ad-hoc meeting scheduling
- ✅ Informative vs. decision agenda points
- ✅ Multi-stakeholder coordination
- ✅ Proposal visibility and transparency
- ✅ Rework/resubmission flows
- ✅ Internal approval before external submission
- ✅ Parallel departmental workflows (IT and HR working simultaneously)
- ✅ Multiple meetings per ToR (working group meetings, board meetings)

## Implementation Checklist

When ready to implement:
- [ ] Create seed data script in `src/db.rs` or separate test fixture
- [ ] Document in CLAUDE.md as canonical test scenario
- [ ] Create automated E2E test using the scenario
- [ ] Add manual test checklist for QA validation
- [ ] Create screenshots for documentation

## Notes

- This scenario represents realistic organizational workflow
- Tests cross-department collaboration patterns
- Demonstrates both recurring and ad-hoc meeting types
- Shows proposal iteration (v1 → v2) based on feedback
- Good candidate for demo/documentation purposes
