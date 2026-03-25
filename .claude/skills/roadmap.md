---
name: roadmap
description: Assess the project roadmap, select the next issue to implement, and drive implementation on main branch
user_invocable: true
---

# Roadmap Workflow

You are orchestrating the implementation of the internet-exploder project. All work happens on the main branch. Follow this process exactly.

## Step 1: Read all open issues

Fetch every open issue with its parent/children structure:

```bash
gh issue list --repo bombfork/internet-exploder --state open --limit 200 --json number,title,labels,state
```

```bash
gh api graphql -f query='query {
  repository(owner: "bombfork", name: "internet-exploder") {
    issues(first: 100, states: [OPEN]) {
      nodes {
        number
        title
        parentIssue { number title }
        subIssues(first: 50) {
          nodes { number title state }
        }
      }
    }
  }
}'
```

## Step 2: Read the last closed issue

Check what was implemented most recently:

```bash
gh issue list --repo bombfork/internet-exploder --state closed --limit 5 --json number,title,closedAt --jq 'sort_by(.closedAt) | reverse | .[0:5]'
```

If there is a last closed issue, read it to understand what was just completed and what state the codebase is in.

## Step 3: Deduce the roadmap path

From the issue tree, reconstruct the implementation order:

1. Identify the **phase hierarchy**: Phase issues (1, 2, 3, 4) contain sub-phase issues (1a, 1b), which contain step issues.
2. A step issue is **ready** if:
   - All its open children (if any) are completed
   - Its prerequisite sibling issues (per the parent issue's ordering) are completed
3. Walk the tree: find the lowest-phase, lowest-step issue that is ready and open.
4. Present the deduced roadmap path to the user: what's done, what's next, what's blocked.

## Step 4: Select the best next candidate

Pick the best next issue to implement based on:
- Implementation order defined in parent issues
- Dependency chain (prerequisites must be closed)
- Parallelizable issues: if multiple issues have no dependency between them, pick the one that unblocks the most downstream work

Present the candidate to the user with a brief rationale.

## Step 5: Assess scope

Read the candidate issue fully:

```bash
gh issue view <NUMBER> --repo bombfork/internet-exploder
```

Assess whether this issue can be implemented in less than a day by a single engineer. Consider:
- Number of files to create/modify
- Complexity of the implementation
- Number of tests to write
- Whether it touches multiple crates with complex interactions

### If too large

Tell the user the issue is too large for a single implementation pass. Then:

1. Break it into smaller child issues, each implementable in less than a day
2. Create them and link as sub-issues:
   ```bash
   gh issue create --repo bombfork/internet-exploder --title "..." --body "Parent: #<NUMBER>\n\n..."
   # Then link via GraphQL addSubIssue mutation
   ```
3. Select the best first child issue as the new candidate
4. Present the breakdown to the user

### If small enough

Tell the user which issue you're about to implement and why. Then invoke the take-issue skill:

```
/take-issue <NUMBER>
```

## Goal

Keep the main branch clean:
- Every commit compiles and passes `mise run check`
- Features are added one at a time in logical order
- The issue tree reflects actual project progress
- No half-implemented features left uncommitted
