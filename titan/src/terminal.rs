use crate::engine;

use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout},
    style::{palette::tailwind, Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame, Terminal,
};
use std::io::Stdout;
use titan_core::{info, tasks::{self, GetBenchmarkDisplays}, Channels, Result};
use tui_logger::TuiLoggerWidget;

type TitanTerminal = Terminal<CrosstermBackend<Stdout>>;

pub enum TermView {
    Tasks,
    Log,
}

pub struct TerminalSubsystem {
    pub channels: Channels,
    pub terminal: Option<TitanTerminal>,
    pub view: TermView,
    pub task_displays: Vec<String>,
}

#[titan_core::subsystem]
impl TerminalSubsystem {

    #[titan_core::task]
    async fn init(&mut self) -> Result<()> {        
        tui_logger::init_logger(titan_core::log::LevelFilter::Trace)?;
                        
        self.terminal = Some(ratatui::init());

        self.channels
            .subscribe_mut::<tasks::StartTask, AddTaskDisplay>()
            .await;
                
        Ok(())
    }

    #[titan_core::task]
    async fn add_task_display(&mut self, id: String, name: &'static str, depth: usize) {
        info!("Hello from subscription!");
    }
    
    #[titan_core::task(benchmark)]
    async fn render(&mut self) -> Result<()> {
        // let task_displays = self
        //     .channels
        //     .get::<tasks::TasksSubsystem>()
        //     .send(tasks::GetTaskDisplays)
        //     .await;

        let benchmark_displays = self
            .channels
            .get::<tasks::TasksSubsystem>()
            .send(tasks::GetBenchmarkDisplays)
            .await?;
       
        self.terminal
            .as_mut()
            .expect("Terminal not initialized!")
            .draw(|f| Self::ui(f, &self.view, vec![], benchmark_displays))?;
        
        self.events()
            .await?;

        Ok(())
    }

    fn ui(
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
                    .title("Task Stack")
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
            // .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .style_error(Style::default().fg(Color::Red))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_info(Style::default().fg(Color::Green))
            .style_trace(Style::default().fg(Color::Blue))
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
    }

    async fn events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_secs(0))? {
            info!("Checking events...");
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('q') {
                    self.channels
                        .get::<engine::EngineSubsystem>()
                        .send_mut(engine::RequestQuit);
                    
                    info!("Here");
                };

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('1') {
                    self.view = TermView::Tasks;
                }

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('2') {
                    self.view = TermView::Log;
                }
                
                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Up {
                    
                }
            }
        }
        Ok(())
    }

    #[titan_core::task]
    fn shutdown(&self) -> Result<()> {
        ratatui::restore();
        Ok(())
    }
}
