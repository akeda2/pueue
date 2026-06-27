use chrono::TimeDelta;
use comfy_table::{
    Cell, ContentArrangement, Row, Table, presets::NOTHING, presets::UTF8_HORIZONTAL_ONLY,
};
use crossterm::style::Color;
use crossterm::terminal;
use pueue_lib::{
    settings::Settings,
    task::{Task, TaskResult, TaskStatus},
};

use super::{OutputStyle, formatted_start_end, query::Rule, start_of_today};

/// This builder is responsible for determining which table columns should be displayed and
/// building a full [comfy_table] from a list of given [Task]s.
#[derive(Debug, Clone)]
pub struct TableBuilder<'a> {
    settings: &'a Settings,
    style: &'a OutputStyle,
    show_row_separators: bool,
    truncate_to_terminal_width: bool,

    /// Whether the columns to be displayed are explicitly selected by the user.
    /// If that's the case, we won't do any automated checks whether columns should be displayed or
    /// not.
    selected_columns: bool,

    /// This following fields represent which columns should be displayed when executing
    /// `pueue status`. `true` for any column means that it'll be shown in the table.
    id: bool,
    status: bool,
    priority: bool,
    enqueue_at: bool,
    dependencies: bool,
    label: bool,
    command: bool,
    path: bool,
    start: bool,
    end: bool,
}

impl<'a> TableBuilder<'a> {
    pub fn new(
        settings: &'a Settings,
        style: &'a OutputStyle,
        show_row_separators: bool,
        truncate_to_terminal_width: bool,
    ) -> Self {
        Self {
            settings,
            style,
            show_row_separators,
            truncate_to_terminal_width,
            selected_columns: false,
            id: true,
            status: true,
            priority: false,
            enqueue_at: false,
            dependencies: false,
            label: false,
            command: true,
            path: true,
            start: true,
            end: true,
        }
    }

    pub fn build(mut self, tasks: &[Task]) -> Table {
        self.determine_special_columns(tasks);

        let mut table = Table::new();
        if self.show_row_separators {
            table.load_preset(UTF8_HORIZONTAL_ONLY);
        } else {
            table.load_preset(NOTHING);
        }
        table
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(self.build_header())
            .add_rows(self.build_task_rows(tasks));

        // Explicitly force styling, in case we aren't on a tty, but `--color=always` is set.
        if self.style.enabled {
            table.enforce_styling();
        }

        table
    }

    /// By default, several columns aren't shown until there's at least one task with relevant data.
    /// This function determines whether any of those columns should be shown.
    fn determine_special_columns(&mut self, tasks: &[Task]) {
        if self.selected_columns {
            return;
        }

        // Check whether there are any tasks with a non-default priority
        if tasks.iter().any(|task| task.priority != 0) {
            self.priority = true;
        }

        // Check whether there are any delayed tasks.
        let has_delayed_tasks = tasks.iter().any(|task| {
            matches!(
                task.status,
                TaskStatus::Stashed {
                    enqueue_at: Some(_)
                }
            )
        });
        if has_delayed_tasks {
            self.enqueue_at = true;
        }

        // Check whether there are any tasks with dependencies.
        if tasks.iter().any(|task| !task.dependencies.is_empty()) {
            self.dependencies = true;
        }

        // Check whether there are any tasks a label.
        if tasks.iter().any(|task| task.label.is_some()) {
            self.label = true;
        }
    }

    /// Take a list of given [pest] rules from our `crate::client::query::column_selection::apply`
    /// logic. Set the column visibility based on these rules.
    pub fn set_visibility_by_rules(&mut self, rules: &[Rule]) {
        // Don't change anything, if there're no rules
        if rules.is_empty() {
            return;
        }

        // First of all, make all columns invisible.
        self.id = false;
        self.status = false;
        self.priority = false;
        self.enqueue_at = false;
        self.dependencies = false;
        self.label = false;
        self.command = false;
        self.path = false;
        self.start = false;
        self.end = false;

        // Make sure we don't do any default column visibility checks of our own.
        self.selected_columns = true;

        for rule in rules {
            match rule {
                Rule::column_id => self.id = true,
                Rule::column_status => self.status = true,
                Rule::column_priority => self.priority = true,
                Rule::column_enqueue_at => self.enqueue_at = true,
                Rule::column_dependencies => self.dependencies = true,
                Rule::column_label => self.label = true,
                Rule::column_command => self.command = true,
                Rule::column_path => self.path = true,
                Rule::column_start => self.start = true,
                Rule::column_end => self.end = true,
                _ => (),
            }
        }
    }

    /// Build a header row based on the current selection of columns.
    fn build_header(&self) -> Row {
        let mut header = Vec::new();

        // Create table header row
        if self.id {
            header.push(Cell::new("Id"));
        }
        if self.status {
            header.push(Cell::new("Status"));
        }
        if self.priority {
            header.push(Cell::new("Prio"));
        }
        if self.enqueue_at {
            header.push(Cell::new("Enqueue At"));
        }
        if self.dependencies {
            header.push(Cell::new("Deps"));
        }
        if self.label {
            header.push(Cell::new("Label"));
        }
        if self.command {
            header.push(Cell::new("Command"));
        }
        if self.path {
            header.push(Cell::new("Path"));
        }
        if self.start {
            header.push(Cell::new("Start"));
        }
        if self.end {
            header.push(Cell::new("End"));
        }

        Row::from(header)
    }

    fn build_task_rows(&self, tasks: &[Task]) -> Vec<Row> {
        let mut rows = Vec::new();
        let truncation = self.get_truncation_widths(tasks);

        // Add rows one by one.
        for task in tasks.iter() {
            let mut row = Row::new();
            // Users can set a max height per row.
            if let Some(height) = self.settings.client.max_status_lines {
                row.max_height(height);
            }

            if self.id {
                row.add_cell(Cell::new(task.id));
            }

            if self.status {
                // Determine the human readable task status representation and the respective color.
                let status_string = task.status.to_string();
                let (status_text, color) = match &task.status {
                    TaskStatus::Running { .. } => (status_string, Color::Green),
                    TaskStatus::Paused { .. } | TaskStatus::Locked { .. } => {
                        (status_string, Color::White)
                    }
                    TaskStatus::Done { result, .. } => match result {
                        TaskResult::Success => (TaskResult::Success.to_string(), Color::Green),
                        TaskResult::DependencyFailed => {
                            ("Dependency failed".to_string(), Color::Red)
                        }
                        TaskResult::FailedToSpawn(_) => ("Failed to spawn".to_string(), Color::Red),
                        TaskResult::Failed(code) => (format!("Failed ({code})"), Color::Red),
                        _ => (result.to_string(), Color::Red),
                    },
                    _ => (status_string, Color::Yellow),
                };
                row.add_cell(self.style.styled_cell(status_text, Some(color), None));
            }

            if self.priority {
                row.add_cell(Cell::new(task.priority.to_string()));
            }

            if self.enqueue_at {
                if let TaskStatus::Stashed {
                    enqueue_at: Some(enqueue_at),
                } = task.status
                {
                    // Only show the date if the task is not supposed to be enqueued today.
                    let enqueue_today =
                        enqueue_at <= start_of_today() + TimeDelta::try_days(1).unwrap();
                    let formatted_enqueue_at = if enqueue_today {
                        enqueue_at.format(&self.settings.client.status_time_format)
                    } else {
                        enqueue_at.format(&self.settings.client.status_datetime_format)
                    };
                    row.add_cell(Cell::new(formatted_enqueue_at));
                } else {
                    row.add_cell(Cell::new(""));
                }
            }

            if self.dependencies {
                let text = task
                    .dependencies
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                row.add_cell(Cell::new(text));
            }

            if self.label {
                row.add_cell(Cell::new(task.label.as_deref().unwrap_or_default()));
            }

            // Add command and path.
            if self.command {
                let command = if self.settings.client.show_expanded_aliases {
                    task.command.as_str()
                } else {
                    task.original_command.as_str()
                };
                let command = truncate_text(command, truncation.command);
                row.add_cell(Cell::new(command));
            }

            if self.path {
                let path = task.path.to_string_lossy();
                let path = truncate_text(&path, truncation.path);
                row.add_cell(Cell::new(path));
            }

            // Add start and end info
            let (start, end) = formatted_start_end(task, self.settings);
            if self.start {
                row.add_cell(Cell::new(start));
            }
            if self.end {
                row.add_cell(Cell::new(end));
            }

            rows.push(row);
        }

        rows
    }

    fn get_truncation_widths(&self, tasks: &[Task]) -> TruncationWidths {
        if !self.truncate_to_terminal_width {
            return TruncationWidths::default();
        }

        let Ok((width, _height)) = terminal::size() else {
            return TruncationWidths::default();
        };

        // We approximate table overhead from column spacing and choose conservative widths.
        let terminal_width = usize::from(width);
        let mut fixed_content_width: usize = 0;
        let mut visible_columns: usize = 0;

        if self.id {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Id".chars().count(),
                tasks
                    .iter()
                    .map(|task| task.id.to_string().chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.status {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Status".chars().count(),
                tasks
                    .iter()
                    .map(|task| status_text(task).chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.priority {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Prio".chars().count(),
                tasks
                    .iter()
                    .map(|task| task.priority.to_string().chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.enqueue_at {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Enqueue At".chars().count(),
                tasks
                    .iter()
                    .map(|task| enqueue_text(task, self.settings).chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.dependencies {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Deps".chars().count(),
                tasks
                    .iter()
                    .map(|task| {
                        task.dependencies
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<String>>()
                            .join(", ")
                            .chars()
                            .count()
                    })
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.label {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Label".chars().count(),
                tasks
                    .iter()
                    .map(|task| task.label.as_deref().unwrap_or_default().chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.start {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "Start".chars().count(),
                tasks
                    .iter()
                    .map(|task| formatted_start_end(task, self.settings).0.chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }
        if self.end {
            visible_columns += 1;
            fixed_content_width += std::cmp::max(
                "End".chars().count(),
                tasks
                    .iter()
                    .map(|task| formatted_start_end(task, self.settings).1.chars().count())
                    .max()
                    .unwrap_or(0),
            );
        }

        let mut command = None;
        let mut path = None;
        if self.command {
            visible_columns += 1;
            let min = "Command".chars().count();
            let desired = std::cmp::max(
                min,
                tasks
                    .iter()
                    .map(|task| {
                        if self.settings.client.show_expanded_aliases {
                            task.command.as_str()
                        } else {
                            task.original_command.as_str()
                        }
                        .chars()
                        .count()
                    })
                    .max()
                    .unwrap_or(0),
            );
            command = Some(VariableColumnWidths { min, desired });
        }
        if self.path {
            visible_columns += 1;
            let min = "Path".chars().count();
            let desired = std::cmp::max(
                min,
                tasks
                    .iter()
                    .map(|task| task.path.to_string_lossy().chars().count())
                    .max()
                    .unwrap_or(0),
            );
            path = Some(VariableColumnWidths { min, desired });
        }

        // Keep a conservative spacing overhead. Comfy-table applies additional cell paddings,
        // which can otherwise cause wrapped words in heavily truncated command columns.
        let spacing_overhead = visible_columns * 2 + 3;
        let fixed_total = fixed_content_width + spacing_overhead;
        let available = terminal_width.saturating_sub(fixed_total);

        allocate_variable_widths(available, command, path)
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct TruncationWidths {
    command: Option<usize>,
    path: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct VariableColumnWidths {
    min: usize,
    desired: usize,
}

fn allocate_variable_widths(
    available: usize,
    command: Option<VariableColumnWidths>,
    path: Option<VariableColumnWidths>,
) -> TruncationWidths {
    let mut widths = TruncationWidths::default();

    match (command, path) {
        (Some(command), Some(path)) => {
            let min_total = command.min + path.min;
            if available <= min_total {
                widths.command = Some(command.min);
                widths.path = Some(path.min);
                return widths;
            }

            let desired_total = command.desired + path.desired;
            if available >= desired_total {
                widths.command = Some(command.desired);
                widths.path = Some(path.desired);
                return widths;
            }

            let extra_available = available - min_total;
            let command_demand = command.desired.saturating_sub(command.min);
            let path_demand = path.desired.saturating_sub(path.min);
            let total_demand = command_demand + path_demand;

            if total_demand == 0 {
                widths.command = Some(command.min);
                widths.path = Some(path.min);
                return widths;
            }

            let command_extra = extra_available * command_demand / total_demand;
            let path_extra = extra_available - command_extra;

            widths.command = Some(command.min + command_extra);
            widths.path = Some(path.min + path_extra);
        }
        (Some(command), None) => {
            widths.command = Some(std::cmp::min(available.max(command.min), command.desired));
        }
        (None, Some(path)) => {
            widths.path = Some(std::cmp::min(available.max(path.min), path.desired));
        }
        (None, None) => (),
    }

    widths
}

fn truncate_text(text: &str, width: Option<usize>) -> String {
    let Some(width) = width else {
        return text.to_string();
    };
    if text.chars().count() <= width {
        return text.to_string();
    }

    if width <= 3 {
        return "..."[..width].to_string();
    }

    let keep = width - 3;
    let left_keep = keep / 2;
    let right_keep = keep - left_keep;
    let left = text.chars().take(left_keep).collect::<String>();
    let right = text
        .chars()
        .rev()
        .take(right_keep)
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let left = left.trim_end();
    let right = right.trim_start();
    format!("{left}...{right}")
}

fn status_text(task: &Task) -> String {
    let status_string = task.status.to_string();
    match &task.status {
        TaskStatus::Done { result, .. } => match result {
            TaskResult::Success => TaskResult::Success.to_string(),
            TaskResult::DependencyFailed => "Dependency failed".to_string(),
            TaskResult::FailedToSpawn(_) => "Failed to spawn".to_string(),
            TaskResult::Failed(code) => format!("Failed ({code})"),
            _ => result.to_string(),
        },
        _ => status_string,
    }
}

fn enqueue_text(task: &Task, settings: &Settings) -> String {
    if let TaskStatus::Stashed {
        enqueue_at: Some(enqueue_at),
    } = task.status
    {
        let enqueue_today = enqueue_at <= start_of_today() + TimeDelta::try_days(1).unwrap();
        if enqueue_today {
            enqueue_at
                .format(&settings.client.status_time_format)
                .to_string()
        } else {
            enqueue_at
                .format(&settings.client.status_datetime_format)
                .to_string()
        }
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{VariableColumnWidths, allocate_variable_widths, truncate_text};

    #[test]
    fn truncate_text_keeps_short_text() {
        assert_eq!(truncate_text("abc", Some(5)), "abc");
    }

    #[test]
    fn truncate_text_applies_ellipsis() {
        assert_eq!(truncate_text("abcdefgh", Some(6)), "a...gh");
    }

    #[test]
    fn truncate_text_trims_whitespace_around_split() {
        assert_eq!(truncate_text("1234 5678", Some(7)), "12...78");
    }

    #[test]
    fn allocation_prefers_column_with_higher_demand() {
        let widths = allocate_variable_widths(
            40,
            Some(VariableColumnWidths {
                min: 7,
                desired: 10,
            }),
            Some(VariableColumnWidths {
                min: 4,
                desired: 35,
            }),
        );

        assert_eq!(widths.command, Some(9));
        assert_eq!(widths.path, Some(31));
    }

    #[test]
    fn allocation_uses_desired_when_enough_space() {
        let widths = allocate_variable_widths(
            80,
            Some(VariableColumnWidths {
                min: 7,
                desired: 12,
            }),
            Some(VariableColumnWidths {
                min: 4,
                desired: 16,
            }),
        );

        assert_eq!(widths.command, Some(12));
        assert_eq!(widths.path, Some(16));
    }
}
