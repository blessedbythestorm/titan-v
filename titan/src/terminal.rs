use crate::{engine, tasks, Channels};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout},
    style::{palette::tailwind, Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame, Terminal,
};
use std::{io::Stdout, ops::Deref, sync::Arc};
use titan_core::{async_trait, runtime::sync::Mutex, Result, Subsystem, Task};
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerWidget};

type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

pub enum TermView {
    Tasks,
    Log,
}

pub struct TerminalSubsystem {
    pub channels: Channels,
    pub terminal: Arc<Mutex<Option<CrosstermTerminal>>>,
    pub view: Arc<Mutex<TermView>>,
}

impl TerminalSubsystem {
    async fn init(&self) -> Result<()> {
        tui_logger::init_logger(titan_core::log::LevelFilter::Trace)?;

        *self.terminal.lock().await = Some(ratatui::init());
        Ok(())
    }

    async fn render(&self) -> Result<()> {
        let task_displays = self
            .channels
            .tasks
            .send(tasks::GetTaskDisplays)
            .await?;

        let benchmark_displays = self
            .channels
            .tasks
            .send(tasks::GetBenchmarkDisplays)
            .await?;

        {
            let mut term_lock = self.terminal.lock().await;

            let term = term_lock
                .as_mut()
                .expect("Terminal not initialized!");

            let view_lock = self.view.lock().await;

            let view = view_lock.deref();

            term.draw(|f| self.ui(f, view, task_displays, benchmark_displays))?;
        };

        self.events().await?;

        Ok(())
    }

    fn ui(
        &self,
        frame: &mut Frame,
        view: &TermView,
        tasks: Vec<tasks::Display>,
        benches: Vec<tasks::Display>,
    ) {
        let headers = ["Name", "Display"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(
                Style::new()
                    .fg(tailwind::SLATE.c200)
                    .bg(tailwind::SLATE.c900),
            )
            .height(1);

        let task_rows = tasks.into_iter().map(|task| {
            Row::new(vec![Cell::new(task.name), Cell::new(task.display)])
                .style(Style::new().fg(tailwind::SLATE.c200))
                .height(1)
        });

        let task_table = Table::new(task_rows, [Constraint::Fill(1), Constraint::Fill(3)])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Benchmarks")
                    .title_style(Style::default().fg(Color::LightCyan)),
            )
            .header(headers.clone());

        let bench_rows = benches.into_iter().map(|bench| {
            Row::new(vec![Cell::new(bench.name), Cell::new(bench.display)])
                .style(Style::new().fg(tailwind::SLATE.c200))
                .height(1)
        });

        let benchmark_table = Table::new(bench_rows, [Constraint::Fill(1), Constraint::Fill(3)])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Benchmarks")
                    .title_style(Style::default().fg(Color::LightCyan)),
            )
            .header(headers);

        let logger = TuiLoggerWidget::default()
            .block(
                Block::bordered()
                    .title("Log")
                    .title_style(Style::default().fg(Color::LightCyan)),
            )
            .output_separator('|')
            .output_timestamp(None)
            .output_level(Some(TuiLoggerLevelOutput::Long))
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .style_error(Style::default().fg(Color::Red))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_info(Style::default().fg(Color::Green))
            .style_trace(Style::default().fg(Color::Cyan))
            .style_debug(Style::default().fg(Color::Magenta));

        match view {
            TermView::Tasks => {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(frame.area());

                frame.render_widget(task_table, layout[0]);
                frame.render_widget(benchmark_table, layout[1]);
            }
            TermView::Log => {
                frame.render_widget(logger, frame.area());
            }
        }

        // Render the list instead of the paragraph
    }

    async fn events(&self) -> Result<()> {
        if event::poll(std::time::Duration::from_secs(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('q') {
                    self.channels
                        .engine
                        .send(engine::Quit)
                        .await?;
                };

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('1') {
                    *self.view.lock().await = TermView::Tasks;
                }

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('2') {
                    *self.view.lock().await = TermView::Log;
                }
            }
        }
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        ratatui::restore();
        Ok(())
    }
}

impl Subsystem for TerminalSubsystem {}

pub struct Init;

#[async_trait]
impl Task<TerminalSubsystem> for Init {
    type Output = ();

    async fn execute(self, terminal: &TerminalSubsystem) -> Result<Self::Output> {
        terminal.init().await?;
        Ok(())
    }
}

pub struct Render;

#[async_trait]
impl Task<TerminalSubsystem> for Render {
    type Output = ();

    fn benchmark() -> bool {
        true
    }

    async fn execute(self, terminal: &TerminalSubsystem) -> Result<Self::Output> {
        terminal.render().await?;
        Ok(())
    }
}

pub struct Shutdown;

#[async_trait]
impl Task<TerminalSubsystem> for Shutdown {
    type Output = ();

    async fn execute(self, terminal: &TerminalSubsystem) -> Result<Self::Output> {
        terminal.shutdown()?;
        Ok(())
    }
}
