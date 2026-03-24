---
name: take-issue
description: Take a GitHub issue and implement it following the project workflow
user_invocable: true
arguments:
  - name: issue
    description: "GitHub issue number to implement"
    required: true
---

# Take Issue Workflow

You are implementing a GitHub issue for the bombfork/internet-exploder project. Follow this process exactly.

## Phase 1: Read the issue

Fetch the issue details:
```bash
gh issue view {{ issue }} --repo bombfork/internet-exploder
```

## Phase 2: Check for open children

Check if this issue has open sub-issues:
```bash
gh api graphql -f query='query {
  repository(owner: "bombfork", name: "internet-exploder") {
    issue(number: {{ issue }}) {
      subIssues(first: 50, filter: {states: [OPEN]}) {
        nodes { number title }
      }
    }
  }
}' --jq '.data.repository.issue.subIssues.nodes'
```

**If there are open children: STOP.** Tell the user this issue has open sub-issues that must be completed first, and list them. Do not proceed.

## Phase 3: Check parent and siblings

Only if there are NO open children, continue.

Get the parent issue:
```bash
gh api graphql -f query='query {
  repository(owner: "bombfork", name: "internet-exploder") {
    issue(number: {{ issue }}) {
      parentIssue { number title }
    }
  }
}' --jq '.data.repository.issue.parentIssue'
```

If there is a parent, read it and its children (siblings of our issue):
```bash
# Read the parent issue
gh issue view <PARENT_NUMBER> --repo bombfork/internet-exploder

# Get all sibling issues with their state
gh api graphql -f query='query {
  repository(owner: "bombfork", name: "internet-exploder") {
    issue(number: <PARENT_NUMBER>) {
      subIssues(first: 50) {
        nodes { number title state }
      }
    }
  }
}' --jq '.data.repository.issue.subIssues.nodes'
```

Assess whether issue #{{ issue }} is the right one to implement next, considering:
- Dependencies between sibling issues (some must be done before others)
- Whether prerequisite siblings are already closed
- The implementation order described in the parent issue

**If there is a better candidate**: tell the user which issue should be implemented first and why. Ask whether they want to stop or keep going with #{{ issue }} anyway. If they want to stop, end here.

## Phase 4: Read comments and implement

Read any comments on the issue:
```bash
gh api repos/bombfork/internet-exploder/issues/{{ issue }}/comments --jq '.[].body'
```

Now implement the issue:
- Work on the `main` branch directly (no feature branches)
- Create one or more commits as you go
- Each commit message follows conventional commit format referencing the issue: `feat(scope): description #{{ issue }}`
- Never bypass pre-commit hooks
- Never force push
- Run `mise run check` to verify everything passes before considering the issue done

Work through all the implementation items and acceptance criteria listed in the issue.

## Phase 5: Review and confirm

When all acceptance criteria are met:

1. Review all non-pushed commits:
```bash
git log origin/main..HEAD --oneline
git diff origin/main..HEAD --stat
```

2. Show the user a summary of what was implemented and the commits made.

3. Ask the user if they are satisfied with the implementation.

4. If satisfied, propose to push and close the issue.

## Phase 6: Push, close, and prepare next

Only if the user agreed to push and close:

1. Identify the probable next sibling issue to implement:
   - Look at the open siblings from Phase 3
   - Consider the implementation order
   - Pick the next one whose prerequisites are now met

2. Assess if the current implementation impacts the definition of the next issue:
   - Did you discover something during implementation that changes what the next issue should do?
   - Did you make an architectural decision that the next issue should know about?
   - Did you add/change APIs that the next issue depends on?

   If so, add a comment to the next issue with the relevant information:
   ```bash
   gh issue comment <NEXT_ISSUE> --repo bombfork/internet-exploder --body "..."
   ```

3. Push the code:
```bash
git push origin main
```

4. Close the issue:
```bash
gh issue close {{ issue }} --repo bombfork/internet-exploder
```

5. Tell the user the issue is closed and suggest the next issue to take.
