use chrono::{Local, TimeDelta};
use pueue_lib::{TaskResult, TaskStatus, log::clean_log_handles, message::*};

use super::*;
use crate::{daemon::internal_state::SharedState, ok_or_save_state_failure};

fn construct_success_clean_message(message: &CleanRequest) -> String {
    let successful_only_fix = if message.successful_only {
        " successfully"
    } else {
        ""
    };

    let older_than_fix = message
        .older_than
        .map(|hours| format!(" older than {hours} hours"))
        .unwrap_or_default();

    let group_fix = message
        .group
        .as_ref()
        .map(|name| format!(" from group '{name}'"))
        .unwrap_or_default();

    format!("All{successful_only_fix} finished tasks{older_than_fix} have been removed{group_fix}")
}

/// Invoked when calling `pueue clean`.
/// Remove all failed or done tasks from the state.
pub fn clean(settings: &Settings, state: &SharedState, message: CleanRequest) -> Response {
    let mut state = state.lock().unwrap();

    let older_than_cutoff = match message.older_than {
        Some(hours) => {
            let Ok(hours) = i64::try_from(hours) else {
                return create_failure_response("`older_than` value is too large.");
            };
            Some(Local::now() - TimeDelta::hours(hours))
        }
        None => None,
    };

    let filtered_tasks =
        state.filter_tasks(|task| matches!(task.status, TaskStatus::Done { .. }), None);

    for task_id in &filtered_tasks.matching_ids {
        // Ensure the task is removable, i.e. there are no dependant tasks.
        if !state.is_task_removable(task_id, &[]) {
            continue;
        }

        let should_skip = {
            let Some(task) = state.tasks().get(task_id) else {
                continue;
            };

            // Check if we should ignore this task, if only successful tasks should be removed.
            if message.successful_only
                && !matches!(
                    task.status,
                    TaskStatus::Done {
                        result: TaskResult::Success,
                        ..
                    }
                )
            {
                true
            // User's can specify a specific group to be cleaned.
            // Skip the task if that's the case and the task's group doesn't match.
            } else if message
                .group
                .as_deref()
                .is_some_and(|group| group != task.group)
            {
                true
            } else if let Some(cutoff) = older_than_cutoff
                && let TaskStatus::Done { end, .. } = &task.status
                && *end > cutoff
            {
                true
            } else {
                false
            }
        };
        if should_skip {
            continue;
        }

        let _ = state.tasks_mut().remove(task_id).unwrap();
        clean_log_handles(*task_id, &settings.shared.pueue_directory());
    }

    ok_or_save_state_failure!(state.save(settings));

    create_success_response(construct_success_clean_message(&message))
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeDelta};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::{super::fixtures::*, *};
    use crate::daemon::internal_state::state::InternalState;

    fn get_message(
        successful_only: bool,
        group: Option<String>,
        older_than: Option<u64>,
    ) -> CleanRequest {
        CleanRequest {
            successful_only,
            group,
            older_than,
        }
    }

    trait TaskAddable {
        fn add_stub_task(&mut self, id: &str, group: &str, task_result: TaskResult);
    }

    impl TaskAddable for InternalState {
        fn add_stub_task(&mut self, id: &str, group: &str, task_result: TaskResult) {
            let task = get_stub_task_in_group(id, group, StubStatus::Done(task_result));
            self.add_task(task);
        }
    }

    /// gets the clean test state with the required groups
    fn get_clean_test_state(groups: &[&str]) -> (SharedState, Settings, TempDir) {
        let (state, settings, tempdir) = get_state();

        {
            let mut state = state.lock().unwrap();

            for &group in groups {
                if !state.groups().contains_key(group) {
                    state.create_group(group);
                }

                state.add_stub_task("0", group, TaskResult::Success);
                state.add_stub_task("1", group, TaskResult::Failed(1));
                state.add_stub_task("2", group, TaskResult::FailedToSpawn("error".to_string()));
                state.add_stub_task("3", group, TaskResult::Killed);
                state.add_stub_task("4", group, TaskResult::Errored);
                state.add_stub_task("5", group, TaskResult::DependencyFailed);
            }
        }

        (state, settings, tempdir)
    }

    #[test]
    fn clean_normal() {
        let (state, settings, _tempdir) = get_stub_state();

        // Only task 1 will be removed, since it's the only TaskStatus with `Done`.
        let message = clean(&settings, &state, get_message(false, None, None));

        // Return message is correct
        assert!(matches!(message, Response::Success(_)));
        if let Response::Success(text) = message {
            assert_eq!(text, "All finished tasks have been removed");
        };

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 4);
    }

    #[test]
    fn clean_normal_for_all_results() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP]);

        // All finished tasks should removed when calling default `clean`.
        let message = clean(&settings, &state, get_message(false, None, None));

        // Return message is correct
        assert!(matches!(message, Response::Success(_)));
        if let Response::Success(text) = message {
            assert_eq!(text, "All finished tasks have been removed");
        };

        let state = state.lock().unwrap();
        assert!(state.tasks().is_empty());
    }

    #[test]
    fn clean_successful_only() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP]);

        // Only successfully finished tasks should get removed when
        // calling `clean` with the `successful_only` flag.
        let message = clean(&settings, &state, get_message(true, None, None));

        // Return message is correct
        assert!(matches!(message, Response::Success(_)));
        if let Response::Success(text) = message {
            assert_eq!(text, "All successfully finished tasks have been removed");
        };

        // Assert that only the first entry has been deleted (TaskResult::Success)
        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 5);
        assert!(!state.tasks().contains_key(&0));
    }

    #[test]
    fn clean_only_in_selected_group() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP, "other"]);

        // All finished tasks should removed in selected group (other)
        let message = clean(
            &settings,
            &state,
            get_message(false, Some("other".into()), None),
        );

        // Return message is correct
        assert!(matches!(message, Response::Success(_)));

        if let Response::Success(text) = message {
            assert_eq!(
                text,
                "All finished tasks have been removed from group 'other'"
            );
        };

        // Assert that only the 'other' group has been cleared
        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 6);
        assert!(state.tasks().iter().all(|(_, task)| &task.group != "other"));
    }

    #[test]
    fn clean_only_successful_only_in_selected_group() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP, "other"]);

        // Only successfully finished tasks should removed in the 'other' group
        let message = clean(
            &settings,
            &state,
            get_message(true, Some("other".into()), None),
        );

        // Return message is correct
        assert!(matches!(message, Response::Success(_)));

        if let Response::Success(text) = message {
            assert_eq!(
                text,
                "All successfully finished tasks have been removed from group 'other'"
            );
        };

        // Assert that only the first entry has been deleted from the 'other' group
        // (TaskResult::Success)
        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 11);
        assert!(!state.tasks().contains_key(&6));
    }

    #[test]
    fn clean_older_than_skips_recent_finished_tasks() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP]);

        // All done tasks are only ~1 minute old in fixtures and therefore not older than 24 hours.
        let message = clean(&settings, &state, get_message(false, None, Some(24)));

        assert!(matches!(message, Response::Success(_)));
        if let Response::Success(text) = message {
            assert_eq!(
                text,
                "All finished tasks older than 24 hours have been removed"
            );
        };

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 6);
    }

    #[test]
    fn clean_older_than_removes_only_old_finished_tasks() {
        let (state, settings, _tempdir) = get_clean_test_state(&[PUEUE_DEFAULT_GROUP]);
        {
            let mut state = state.lock().unwrap();
            if let Some(task) = state.tasks_mut().get_mut(&0)
                && let TaskStatus::Done { end, .. } = &mut task.status
            {
                *end = Local::now() - TimeDelta::hours(30);
            }
        }

        // Only task 0 has been made older than 24 hours.
        let _ = clean(&settings, &state, get_message(false, None, Some(24)));

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 5);
        assert!(!state.tasks().contains_key(&0));
        assert!(state.tasks().contains_key(&1));
    }
}
