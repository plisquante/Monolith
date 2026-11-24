
---
name: background-tasks
description: Efficiently manage long-running commands, background shell processes, builds, tests, deployments, and parallel task execution in Claude Code. Use whenever a command may take more than 30 seconds.
---

# Background Tasks

## Overview

This skill defines how Claude Code should execute, monitor, coordinate, and collect results from long-running shell commands.

The primary objective is to:

- Keep the main conversation responsive.
- Avoid wasting tool calls on repeated status checks.
- Execute independent operations concurrently when beneficial.
- Collect complete command output.
- Prevent duplicate builds, tests, and deployments.
- Handle failures, timeouts, cancellation, and interrupted sessions correctly.
- Verify that operations actually completed before reporting success.

The Iron Law:

For long-running commands, use background execution and retrieve their results using "TaskOutput(block: true)" whenever the available tools support it.

NEVER use "sleep && cat", repeated polling, or "tail -f" as a substitute for proper task management.

---

0. Wait Mechanisms (read first)

TaskOutput is not loaded in every session. Check once with ToolSearch (query "select:TaskOutput"). If it is not found, do not retry it. Use one of these:

A. Background Bash with a completion marker (default when TaskOutput is unavailable)

1. Start the job; it writes its output and an exit marker to a log file:
   Bash(command: "(<job> > build.log 2>&1; echo EXIT=$? >> build.log)", run_in_background: true)
2. Wait with ONE background call that exits when the marker appears:
   Bash(command: "until grep -q '^EXIT=' build.log; do sleep 5; done; tail -n 40 build.log", run_in_background: true, timeout: 1800000)
   It sends one completion notification. There is no polling.
3. Read the exit code from the EXIT= line. Read only the filtered lines (grep -E 'error|FAIL|Traceback'), never the whole log.

B. Monitor (progress and failure signals while the job runs)

   Monitor(command: "tail -n 0 -F build.log | grep -E --line-buffered 'EXIT=|Traceback|error|FAIL|panicked|Killed'", description: "<job>: exit and failure signals", timeout_ms: 1800000)

The filter must match every terminal state, not only success. Re-arm the Monitor if the job is still running when it expires.

Wherever this document shows TaskOutput(task_id, block: true, timeout: N), use method A with the same timeout as its upper bound. Keep the task id or log path in your task record. Do not use foreground sleep (blocked in some environments) and do not repeat tail or cat to check progress.

---

1. When to Use Background Execution

Use the following guidelines:

Command Type| Expected Duration| Background?
Simple shell commands| Under 10 seconds| NO
File inspection| Under 10 seconds| NO
Single unit test| Under 30 seconds| Usually NO
Multiple unit tests| 30 seconds–2 minutes| YES
Full test suite| 1–5 minutes| YES
Integration tests| 1–5 minutes| YES
E2E tests| 2–10 minutes| YES
Project compilation| 30 seconds–5 minutes| YES
Full production build| 1–10 minutes| YES
Git push with hooks| 30 seconds–3 minutes| YES
Dependency installation| 30 seconds–5 minutes| YES
Large code generation| 1–15 minutes| YES
Large repository analysis| Variable| YES, when executed through shell
Database migrations| Variable| YES, with appropriate safeguards
Deployment operations| Variable| YES, with appropriate safeguards

General Rule

If a command might take longer than 30 seconds, prefer background execution.

Do not background commands simply because they are shell commands.

Use foreground execution when:

- The command requires interactive user input.
- The command needs an interactive terminal.
- Immediate output is essential to the next decision.
- The command is short enough to finish quickly.
- The operation cannot safely run without maintaining an interactive session.

---

2. The Correct Execution Pattern

Standard Background Execution

Use this pattern for long-running operations.

Step 1: Start the command

Use the Bash tool with background execution enabled.

Bash(
  command: "pnpm build",
  run_in_background: true
)

The tool returns a task identifier.

Example:

task_id: "abc123"

Step 2: Wait for completion

Use TaskOutput with blocking enabled.

TaskOutput(
  task_id: "abc123",
  block: true,
  timeout: 300000
)

This waits for the task and retrieves its output.

Step 3: Evaluate the result

Check:

- Whether the command completed.
- Whether its exit code indicates success.
- Whether errors occurred.
- Whether warnings require attention.
- Whether the expected output files or artifacts exist.
- Whether additional verification is necessary.

Never report success merely because a background command was started.

A started task is not a completed task.

---

3. The Anti-Pattern: Repeated Polling

NEVER use the following approach:

sleep 30 && cat output.log

Do not repeatedly execute:

sleep 60 && tail output.log

Do not repeatedly inspect processes using:

ps aux | grep build

Do not repeatedly invoke:

TaskOutput(
  task_id: "abc123",
  block: false,
  timeout: 1000
)

Do not repeatedly request empty or incomplete output.

Why this is prohibited

Repeated polling:

- Consumes unnecessary tool calls.
- Wastes context.
- Produces little or no useful information.
- Makes task management inefficient.
- Can obscure the actual completion result.

Correct alternative

Bash(
  command: "./gradlew build",
  run_in_background: true
)

// Wait for the actual result.
TaskOutput(
  task_id: "abc123",
  block: true,
  timeout: 300000
)

Use blocking waits instead of repeatedly checking whether a task has finished.

---

4. TaskOutput Parameters

TaskOutput(
  task_id: string,
  block: boolean,
  timeout: number
)

Parameter| Description
"task_id"| Identifier returned by background Bash execution
"block"| Whether to wait for completion
"timeout"| Maximum time to wait, in milliseconds

Recommended settings

Operation| Timeout
Single unit test| 60,000 ms
Multiple unit tests| 120,000 ms
Integration tests| 120,000 ms
E2E single spec| 300,000 ms
Full E2E suite| 600,000 ms
Full build| 300,000 ms
Large compilation| 600,000 ms
Git push with hooks| 180,000 ms
Dependency installation| 300,000 ms

These are recommended waiting limits, not guarantees about how long the underlying process will run.

Important timeout behavior

If TaskOutput reaches its waiting timeout:

1. Do not assume that the underlying command was terminated.
2. Determine whether the task is still running.
3. Do not launch the same command again.
4. Continue other useful work if possible.
5. Retrieve the original task's result when appropriate.

A timeout while waiting does not necessarily mean the command itself failed.

---

5. Parallel Background Tasks

Independent operations should run concurrently whenever this improves execution time.

Example: Build, tests, and lint

Start all independent operations first:

Bash(
  command: "pnpm test:unit",
  run_in_background: true
)

// task_id: task_a

Bash(
  command: "pnpm test:e2e",
  run_in_background: true
)

// task_id: task_b

Bash(
  command: "pnpm lint",
  run_in_background: true
)

// task_id: task_c

Then collect their results:

TaskOutput(
  task_id: "task_a",
  block: true,
  timeout: 120000
)

TaskOutput(
  task_id: "task_b",
  block: true,
  timeout: 600000
)

TaskOutput(
  task_id: "task_c",
  block: true,
  timeout: 120000
)

Parallel execution rules

- Start independent tasks before waiting for individual results.
- Track every returned task ID.
- Never confuse one task's output with another.
- Wait for all required operations before declaring the overall workflow complete.
- Do not start duplicate tasks.
- Do not run conflicting operations simultaneously.
- Avoid excessive parallelism that exhausts CPU, RAM, disk I/O, network bandwidth, or API rate limits.

When NOT to parallelize

Do not parallelize operations when:

- One operation depends on another's output.
- Two builds modify the same output directory.
- Tests share mutable databases or external resources.
- Multiple Git operations modify the same working tree.
- Two migrations target the same database.
- Parallel execution could corrupt artifacts.

Dependency takes priority over concurrency.

---

6. Common Development Workflows

6.1 Running E2E Tests

Bash(
  command: "pnpm test:e2e -- checkout.spec.ts",
  run_in_background: true
)

// Save returned task ID.

Wait:

TaskOutput(
  task_id: "e2e_123",
  block: true,
  timeout: 300000
)

After completion:

- Inspect failed tests.
- Identify the first meaningful error.
- Check screenshots and test reports when available.
- Distinguish application failures from infrastructure failures.
- Fix actual issues before claiming the test passed.

6.2 Running a Full Build

Bash(
  command: "./gradlew clean build",
  run_in_background: true
)

Wait:

TaskOutput(
  task_id: "build_123",
  block: true,
  timeout: 300000
)

After completion:

- Verify the exit code.
- Inspect compiler errors.
- Check generated JARs or other artifacts.
- Verify artifact size and location.
- Run relevant tests if necessary.

Do not assume that a successful compilation proves runtime correctness.

6.3 Git Push With Hooks

Bash(
  command: "git push -u origin feature/my-branch",
  run_in_background: true
)

Wait:

TaskOutput(
  task_id: "push_456",
  block: true,
  timeout: 180000
)

After completion:

- Verify the push result.
- Check whether pre-push hooks succeeded.
- Inspect the remote branch when necessary.
- Distinguish a successful local commit from a successful remote push.

Never claim that changes were pushed if the push failed or remains unverified.

6.4 Dependency Installation

Bash(
  command: "pnpm install",
  run_in_background: true
)

Wait for completion and inspect:

- Exit code.
- Dependency resolution errors.
- Lockfile changes.
- Missing packages.
- Post-install failures.

Do not start another installation while the first one is still running unless there is a specific reason.

---

7. Managing Background Tasks in Claude Code

Claude Code provides task-management capabilities beyond collecting shell output.

"/tasks"

Use "/tasks" to inspect background sessions when available.

It can help identify:

- Running background sessions.
- Active subagents.
- Background shell commands.
- Cloud sessions.
- Task progress and available management actions.

Use the task panel or session interface when you need to inspect, stop, or manage a running operation.

Background shell commands

Background Bash commands and their task identifiers belong to the current execution environment.

Always retain the original task ID when continuing a workflow.

Subagents

Use subagents when a task benefits from independent context and specialized work.

Good candidates:

- Repository exploration.
- Independent code reviews.
- Test investigation.
- Documentation research.
- Separate implementation tasks.

Do not use a subagent merely to execute a trivial shell command.

A subagent is not the same thing as a background shell process.

Cloud sessions

Claude Code cloud sessions are separate execution sessions.

They can be started independently using supported Claude Code cloud functionality.

Example:

claude --cloud "Analyze and fix the failing integration tests"

Multiple independent cloud sessions can run concurrently.

However:

- Cloud sessions have separate execution environments.
- Local shell task IDs must not be confused with cloud session IDs.
- Cloud sessions have their own resource and rate limits.
- A cloud session may lose unfinished background processes if its environment expires.

Use "/tasks" and the appropriate session-management interface to inspect available sessions.

---

8. Failure Handling

Every background task must have a clear outcome.

Possible outcomes:

Status| Meaning| Action
Success| Command completed successfully| Verify artifacts
Failed| Command returned an error| Inspect and diagnose
Timed out| Waiting limit reached| Determine whether still running
Cancelled| Execution was stopped| Report cancellation
Interrupted| Environment or process stopped| Determine what must be rerun
Unknown| Completion cannot be established| Do not claim success

When a command fails

1. Retrieve its output.
2. Identify the actual failure.
3. Determine whether the issue is code-related, environmental, or external.
4. Fix the underlying cause.
5. Rerun only the necessary operation.
6. Verify the new result.

Do not rerun an entire test suite if only one independent test needs to be repeated.

Avoid blind retries

Never automatically retry:

- Destructive database operations.
- Deployments.
- Publishing operations.
- Git pushes with uncertain remote state.
- Commands that may have partially completed.

First determine what happened.

---

9. Cancellation and Recovery

Cancelling a task

When cancellation is necessary:

- Use the task-management interface when available.
- Use the available task cancellation mechanism.
- Do not assume that cancelling the Claude conversation automatically terminates every external process.

Recovering interrupted work

If a session is interrupted:

1. Inspect the current task state.
2. Identify unfinished operations.
3. Check whether outputs or artifacts were partially generated.
4. Verify repository and Git state.
5. Resume the original task when possible.
6. Rerun only operations that genuinely need restarting.

Never assume an interrupted build, test, or deployment completed successfully.

---

10. Output Management

Long-running commands can generate substantial output.

Avoid flooding the main context with irrelevant logs.

Prioritize:

1. Exit code.
2. Final success or failure message.
3. First meaningful error.
4. Relevant stack trace.
5. Failed test names.
6. Artifact paths.
7. Warnings that affect correctness.

If complete logs are necessary, inspect the relevant log file or output range rather than repeatedly dumping the entire output.

Do not discard important errors simply to reduce context usage.

For very large outputs, summarize the findings while preserving the exact error messages and relevant locations.

---

11. Resource Management

Parallel execution should improve performance, not destabilize the development environment.

Before starting many tasks, consider:

- Available RAM.
- CPU core count.
- Disk capacity.
- Disk I/O.
- Network bandwidth.
- Database connection limits.
- API rate limits.
- Build cache contention.

For memory-intensive projects, prefer a smaller number of concurrent operations.

For example, running three large JVM compilations simultaneously may be slower and less stable than running them sequentially.

Do not maximize concurrency blindly.

---

12. Task Tracking Requirements

Maintain a clear association between each task and its purpose.

Example:

Task ID| Operation| Status
task_a| Unit tests| Running
task_b| E2E tests| Completed
task_c| Lint| Failed
task_d| Production build| Pending

When useful, keep track of:

- Task identifier.
- Command.
- Working directory.
- Purpose.
- Start state.
- Completion state.
- Exit code.
- Output location.

Never lose track of an active task.

Do not start the same operation twice because its first result has not yet been collected.

---

13. Red Flags — STOP

The following behaviors are prohibited:

- Using "sleep" before retrieving task output.
- Repeatedly calling "cat" to inspect logs.
- Using "tail -f" as a task-completion mechanism.
- Repeatedly calling "TaskOutput(block: false)".
- Launching duplicate builds or tests.
- Starting dependent operations before prerequisites finish.
- Reporting success before checking the exit code.
- Ignoring failed background tasks.
- Assuming a waiting timeout killed the underlying process.
- Losing track of task IDs.
- Running excessive parallel operations without considering resources.
- Claiming that a deployment or Git push succeeded without verification.

---

14. Best Practices Summary

Situation| Required Behavior
Command takes under 10 seconds| Run normally
Command may take over 30 seconds| Prefer background execution
Background command starts| Record its task ID
Task needs to finish| Use "TaskOutput(block: true)"
Multiple independent tasks| Start concurrently
Tasks have dependencies| Execute in dependency order
Waiting timeout occurs| Check original task
Task fails| Diagnose before retrying
Task is cancelled| Report cancellation
Session is interrupted| Verify state before recovery
Build succeeds| Verify artifacts
Tests succeed| Verify test results
Deployment succeeds| Verify actual deployment state

---

15. Final Operating Principle

The objective is not to minimize the number of tool calls at any cost.

The objective is to minimize unnecessary tool calls while maintaining correctness, reliability, and complete verification.

Preferred workflow:

Identify long-running operation
          |
          v
Determine dependencies
          |
          v
Start independent commands in background
          |
          v
Record task IDs
          |
          v
Wait using TaskOutput(block: true)
          |
          v
Inspect outputs and exit codes
          |
          v
Handle failures if necessary
          |
          v
Verify artifacts and final state
          |
          v
Report accurate results

Remember:

- Background execution is the default for long-running commands.
- Blocking TaskOutput is the preferred completion mechanism.
- Parallelize independent work.
- Never blindly retry.
- Never confuse started with completed.
- Never claim success without verification.

Reliability always takes priority over speed.
