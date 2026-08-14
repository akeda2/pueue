use pueue_lib::{State, Task};

use crate::{client::helper::*, internal_prelude::*};

/// Test that the normal status command works as expected.
/// Calling `pueue` without any subcommand is equivalent of using `status`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default() -> Result<()> {
    Ok(())
}

/// Test the status output with all columns enabled.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a paused task so we can use it as a dependency.
    run_client_command(
        shared,
        &["add", "--label", "test", "--delay", "1 minute", "ls"],
    )?;

    // Add a second command that depends on the first one.
    run_client_command(shared, &["add", "--after=0", "ls"])?;

    let output = run_status_without_path(shared, &[]).await?;

    let context = get_task_context(&daemon.settings).await?;
    assert_template_matches("status__full", output, context)?;

    Ok(())
}

///// Calling `status` with the `--color=always` flag, colors the output as expected.
//#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
//async fn colored() -> Result<()> {
//    let daemon = daemon().await?;
//    let shared = &daemon.settings.shared;
//
//    // Add a task and wait until it finishes.
//    assert_success(add_task(shared, "ls").await?);
//    wait_for_task_condition(shared, 0, Task::is_done).await?;
//
//    let output = run_status_without_path(shared, &["--color", "always"]).await?;
//
//    let context = get_task_context(&daemon.settings).await?;
//    assert_stdout_matches("status__colored", output, context)?;
//
//    Ok(())
//}

/// Test status for single group
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_group() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a new group
    add_group_with_slots(shared, "testgroup", 1).await?;

    // Add a task to the new testgroup.
    run_client_command(shared, &["add", "--group", "testgroup", "ls"])?;
    // Add another task to the default group.
    run_client_command(shared, &["add", "--stashed", "ls"])?;

    // Make sure the first task finished.
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    let output = run_status_without_path(shared, &["--group", "testgroup"]).await?;

    // The output should only show the first task
    let context = get_task_context(&daemon.settings).await?;
    assert_template_matches("status__single_group", output, context)?;

    Ok(())
}

/// Test status for single group in compact mode.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_group_compact() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a new group
    add_group_with_slots(shared, "testgroup", 1).await?;

    // Add a task to the new testgroup.
    run_client_command(shared, &["add", "--group", "testgroup", "ls"])?;
    // Add another task to the default group.
    run_client_command(shared, &["add", "--stashed", "ls"])?;

    // Make sure the first task finished.
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    let output = run_status_without_path(shared, &["--compact", "--group", "testgroup"]).await?;

    // The output should only show the first task and should not include separator lines.
    let context = get_task_context(&daemon.settings).await?;
    assert_template_matches("status__single_group_compact", output, context)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_flattens_multiline_commands() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    run_client_command(shared, &["add", "--stashed", "echo first\nsecond"])?;

    let output = run_client_command(
        shared,
        &["status", "--compact", "columns=id,status,command"],
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(stdout.contains("echo first second"));
    assert!(!stdout.contains("echo first\nsecond"));

    Ok(())
}

/// Multiple groups
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_groups() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a new group
    add_group_with_slots(shared, "testgroup", 1).await?;
    add_group_with_slots(shared, "testgroup2", 1).await?;

    // Add a task to the new testgroup.
    run_client_command(shared, &["add", "--group", "testgroup", "ls"])?;
    // Add another task to the default group.
    run_client_command(shared, &["add", "--group", "testgroup2", "ls"])?;

    // Make sure the second task finished.
    wait_for_task_condition(shared, 1, Task::is_done).await?;

    let output = run_status_without_path(shared, &[]).await?;

    // The output should show multiple groups
    let context = get_task_context(&daemon.settings).await?;
    assert_template_matches("status__multiple_groups", output, context)?;

    Ok(())
}

/// Calling `pueue status --json` will result in the current state being printed to the cli.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a task and wait until it finishes.
    assert_success(add_task(shared, "ls").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    let output = run_client_command(shared, &["status", "--json"])?;

    let json = String::from_utf8_lossy(&output.stdout);
    let deserialized_state: State =
        serde_json::from_str(&json).context("Failed to deserialize json state")?;

    let state = get_state(shared).await?;
    assert_eq!(
        deserialized_state, *state,
        "Json state differs from actual daemon state."
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn elapsed_and_cpu_columns() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    assert_success(add_task(shared, "ls").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    let output = run_status_without_path(shared, &["-e"]).await?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(stdout.contains("Elapsed"));
    assert!(stdout.contains("CPU"));
    assert!(!stdout.contains(" Start "));
    assert!(!stdout.contains(" End"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tail_limits_status_json_output() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    for _ in 0..8 {
        run_client_command(shared, &["add", "--stashed", "ls"])?;
    }

    let output = run_client_command(shared, &["status", "--json", "--tail", "3"])?;

    let json = String::from_utf8_lossy(&output.stdout);
    let deserialized_state: State =
        serde_json::from_str(&json).context("Failed to deserialize json state")?;

    let ids: Vec<usize> = deserialized_state.tasks.keys().copied().collect();
    assert_eq!(ids, vec![5, 6, 7]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tail_overrides_query_limit() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    for _ in 0..8 {
        run_client_command(shared, &["add", "--stashed", "ls"])?;
    }

    let output = run_client_command(shared, &["status", "--json", "--tail", "2", "first", "6"])?;

    let json = String::from_utf8_lossy(&output.stdout);
    let deserialized_state: State =
        serde_json::from_str(&json).context("Failed to deserialize json state")?;

    let ids: Vec<usize> = deserialized_state.tasks.keys().copied().collect();
    assert_eq!(ids, vec![6, 7]);

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cpu_time_is_recorded_for_process_group_workload() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    assert_success(
        add_task(
            shared,
            "yes > /dev/null & pid=$!; sleep 1; kill $pid; wait $pid || true",
        )
        .await?,
    );
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    let state = get_state(shared).await?;
    let Some(task) = state.tasks.get(&0) else {
        bail!("Failed to get task 0 from state");
    };

    assert!(
        task.cpu_time_ms.unwrap_or_default() > 0,
        "Expected cpu_time_ms to be set for a CPU-bound task, got {:?}",
        task.cpu_time_ms
    );

    Ok(())
}
