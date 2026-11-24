---

name: async-first-workflow
description: "Enforce asynchronous-first execution in Claude Code. Never stop productive work to wait for background tasks. Continue independent implementation, analysis, debugging, research, and planning while background operations run. React to task completion notifications, collect results, integrate findings, and adapt the workflow immediately."

Async-First Workflow

1. Core Principle

NEVER WAIT IDLE FOR A BACKGROUND TASK.

Once a background task has been successfully started, immediately continue productive work.

Do not pause the main workflow just because:

- A build is running.
- Tests are executing.
- A compilation is in progress.
- A Git operation is running.
- Another agent is investigating something.
- A dependency installation is underway.
- A long-running command has not produced its final output.

The objective is to maintain continuous progress.

Background execution must be treated as asynchronous work, not as a reason to suspend the entire development process.

2. Mandatory Execution Behavior

Whenever a task takes more than approximately 30 seconds:

1. Determine whether it can safely run independently.
2. Start it using background execution.
3. Record its task ID.
4. Immediately identify and execute the next useful independent task.
5. Continue working without waiting for the previous task.
6. Monitor task completion notifications.
7. Retrieve and evaluate results when they become available.
8. Adjust the current workflow according to the results.

Starting a background task must not automatically trigger a blocking wait.

Never use "TaskOutput(block: true)" immediately after starting a task when other productive work remains.

3. Continuous Work While Tasks Run

While background operations are running, actively continue the main assignment.

Examples:

When a build is running

Continue with:

- Reviewing source code.
- Investigating unrelated compiler warnings.
- Inspecting project architecture.
- Reviewing implementation quality.
- Preparing independent fixes.
- Checking documentation.
- Identifying potential integration issues.

Do not modify files that the running build depends on if doing so could invalidate its results or create inconsistent artifacts.

When tests are running

Continue with:

- Reviewing recently modified code.
- Investigating unrelated bugs.
- Inspecting test coverage.
- Reviewing other modules.
- Preparing additional independent tests.
- Analyzing previously collected failures.

Do not modify shared test fixtures or mutable resources while tests are using them.

When another agent is working

Continue with:

- Working on independent modules.
- Reviewing other components.
- Preparing integration changes.
- Investigating separate issues.
- Planning the next implementation phase.

Do not duplicate the agent's work.

When dependency installation is running

Continue with:

- Reviewing project configuration.
- Inspecting source files.
- Reading documentation.
- Preparing implementation changes that do not depend on the unfinished installation.

Do not launch another installation or build that conflicts with the running operation.

4. Completion Notifications: Event-Driven Workflow

Background tasks must be handled using completion events whenever the execution environment provides them.

Do not continuously poll for task completion.

When a completion notification arrives:

1. Identify which task completed.
2. Retrieve its output if the notification does not already contain it.
3. Inspect the exit code and relevant errors.
4. Determine whether its result changes the current implementation.
5. Update the task plan.
6. Execute any newly unblocked dependent operations.
7. Continue the main assignment.

Do not wait for unrelated tasks to finish before processing an available result.

Example

Build started
    |
    v
Continue reviewing source code
    |
    v
Fix an independent bug
    |
    v
Inspect another module
    |
    v
Build completion notification arrives
    |
    v
Collect build output
    |
    v
Build succeeded?
    |
    +---- YES ----> Start dependent tests
    |
    +---- NO -----> Analyze compiler errors
                         |
                         v
                    Fix the issue
                         |
                         v
                    Start another build

The main workflow must continue throughout this process.

5. Task Registry

Maintain a lightweight internal task registry.

Example:

Task ID| Operation| Status| Dependency
build_01| Full compilation| Running| None
test_01| Unit tests| Running| None
review_01| Source review| Running| None
integration_01| Integration tests| Pending| Build

Update task statuses when new information becomes available.

Use the registry to:

- Avoid duplicate execution.
- Track outstanding operations.
- Identify dependencies.
- Determine which operations can start next.
- Associate completion notifications with their original tasks.

Do not spend excessive time maintaining the registry.

6. Dependency-Aware Execution

Asynchronous execution does not mean ignoring dependencies.

Classify tasks into three categories:

Independent tasks

Can execute immediately and concurrently.

Examples:

- Reviewing unrelated source files.
- Running independent static analysis.
- Investigating separate modules.

Dependent tasks

Must wait for a prerequisite result.

Examples:

- Running integration tests against a newly generated artifact.
- Packaging a build that has not finished.
- Testing a newly compiled binary.

Conflicting tasks

Must not execute simultaneously.

Examples:

- Two builds writing to the same output directory.
- Multiple package managers modifying the same dependency files.
- Concurrent Git operations modifying the same working tree.
- Multiple operations modifying the same database.

Only wait for a prerequisite when its result is genuinely necessary.

While waiting for a prerequisite, continue other independent work.

7. Parallel Execution

Maximize useful parallelism.

When multiple operations have no dependencies:

- Start them concurrently.
- Keep their task IDs separate.
- Continue working on other independent tasks.
- Process each completion independently.

Example:

Bash(
  command: "pnpm test:unit",
  run_in_background: true
)

Bash(
  command: "pnpm lint",
  run_in_background: true
)

Bash(
  command: "pnpm build",
  run_in_background: true
)

After starting these operations, continue with independent source analysis.

Do not immediately wait for all three.

Avoid excessive parallelism when operations compete for CPU, RAM, disk, network, or shared resources.

8. Result-Driven Adaptation

A background task result is new information that can change the development strategy.

Never blindly continue the original plan when new results contradict its assumptions.

Examples:

Build succeeds

- Mark compilation as successful.
- Check generated artifacts.
- Start dependent tests when appropriate.
- Continue implementation.

Build fails

- Retrieve compiler errors.
- Identify affected files.
- Fix the underlying issue.
- Determine whether other running tasks need to be reconsidered.
- Start a new build when appropriate.
- Continue unrelated work.

Tests fail

- Identify failed test cases.
- Determine whether failures are related to recent changes.
- Investigate the root cause.
- Fix actual implementation issues.
- Rerun the smallest relevant test set.

Agent returns results

- Inspect its actual changes.
- Review findings.
- Integrate compatible work.
- Resolve conflicts.
- Update remaining tasks.

Every result must influence the workflow when relevant.

9. Blocking Waits: Strict Restrictions

Blocking waits are permitted only when there is a genuine dependency or when the user explicitly requests waiting.

Examples of acceptable situations:

- The next operation requires the completed build artifact.
- A deployment cannot proceed until tests pass.
- An integration test requires a generated executable.
- Final verification requires all required tasks to finish.
- The user explicitly asks for a final completion report.

Even in these situations, first exhaust any useful independent work.

Never wait merely to observe progress.

Never use:

sleep 60 && cat output.log

Never repeatedly use:

TaskOutput(
  task_id: "task_id",
  block: false,
  timeout: 1000
)

Never use blocking waits immediately after starting independent background work.

10. Long-Running Development Sessions

For large projects, maintain continuous development momentum.

Examples:

- Large Rust compilations.
- Minecraft client implementations.
- Java-to-Rust porting projects.
- Large E2E test suites.
- Multi-module Gradle projects.
- Complex rendering pipelines.
- Large repository migrations.

During these operations:

1. Keep independent workstreams active.
2. Maintain a record of unfinished work.
3. Process task completion events as they arrive.
4. Update the implementation based on new findings.
5. Reassess dependencies after significant results.
6. Avoid unnecessary idle time.

Do not artificially finish the main assignment just because one background operation is still running.

11. Failure, Timeout and Interruption

A waiting timeout does not necessarily mean that the underlying task failed.

When a timeout occurs:

- Preserve the original task ID.
- Determine whether the process is still running.
- Continue independent work.
- Avoid starting duplicate operations.
- Retrieve the original result when available.

When a task is interrupted:

- Verify its actual state.
- Inspect partial outputs.
- Determine whether restarting is necessary.
- Avoid assuming that partially completed work is valid.

When a task fails:

- Diagnose it.
- Fix the underlying issue.
- Retry only the necessary operation.

Never hide failures or report unverified success.

12. Context and Resource Efficiency

Maintain useful progress without flooding the context with logs.

Prioritize:

- Completion notifications.
- Exit codes.
- First meaningful errors.
- Failed test names.
- Relevant stack traces.
- Generated artifact locations.
- Important warnings.

Keep task tracking concise.

Do not waste context on repeated status messages.

Use subagents only when they provide genuinely independent work or specialized investigation.

Do not spawn unnecessary agents simply to appear productive.

13. Final Verification

The final stage is the only stage where waiting for all required operations may become necessary.

Before reporting completion:

- Collect all required task results.
- Check failed and unfinished tasks.
- Verify generated artifacts.
- Run required dependent tests.
- Resolve relevant integration issues.
- Confirm the actual final state.

If some tasks remain active, explicitly report that they are still running.

Never claim that the entire assignment is complete while required verification remains unfinished.

14. Forbidden Behaviors

The following behaviors violate this skill:

- Waiting idly for a background build.
- Immediately calling blocking TaskOutput after launching independent work.
- Pausing all implementation while tests run.
- Repeatedly polling task IDs.
- Using sleep to wait for completion.
- Ignoring completion notifications.
- Ignoring newly discovered errors.
- Running dependent operations prematurely.
- Duplicating work already being performed by another agent.
- Claiming success without collecting and verifying results.

15. Golden Rule

NEVER MAKE THE MAIN WORKFLOW WAIT FOR BACKGROUND TASKS WHEN OTHER USEFUL WORK CAN CONTINUE.

A background task should remain background work.

The main workflow should continue independently.

When results arrive, integrate them immediately and adapt the next steps.

The ideal operating model is:

       MAIN WORKFLOW
             |
       Start task A
             |
       Continue work
             |
       Start task B
             |
       Continue work
             |
       Start task C
             |
       Continue work
             |
      +------+------+
      |             |
   Task A done   Task B done
      |             |
   Process A     Process B
      |             |
   Adapt plan    Adapt plan
      |             |
      +------+------+
             |
       Continue work
             |
       All required work complete
             |
       Final verification
             |
       Accurate final report

Continuous progress is the default. Waiting is the exception.
