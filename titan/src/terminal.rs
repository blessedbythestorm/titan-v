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
use titan_core::{info, tasks, tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt}, ArcLock, Channels, Result};
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerWidget};

type TitanTerminal = Terminal<CrosstermBackend<Stdout>>;

pub enum TermView {
    Tasks,
    Log,
}

pub struct TerminalSubsystem {
    pub channels: Channels,
    pub terminal: ArcLock<Option<TitanTerminal>>,
    pub view: ArcLock<TermView>,
    pub mut_test: bool,
}

#[titan_core::subsystem]
impl TerminalSubsystem {

    #[titan_core::task]
    async fn init(&self) -> Result<()> {        
        tui_logger::init_logger(titan_core::log::LevelFilter::Trace)?;
                        
        self.terminal.write(Some(ratatui::init()))
         .await;
        
        Ok(())
    }
    
    #[titan_core::task]
    pub async fn mutable_task(&mut self) -> Result<()> {
        self.mut_test = !self.mut_test;
        info!("{}", self.mut_test);
        Ok(())
    }

    #[titan_core::task(benchmark)]
    async fn render(&mut self) -> Result<()> {
        // let task_displays = self
        //     .channels
        //     .get::<tasks::TasksSubsystem>()
        //     .send(tasks::GetTaskDisplays)
        //     .await?;

        let benchmark_displays = self
            .channels
            .get::<tasks::TasksSubsystem>()
            .send(tasks::GetBenchmarkDisplays)
            .await?;

        let task_displays = Vec::new(); 
        
        {
            let view = self.view
                .lock()
                .await;
            
            let mut term_lock = self.terminal
                .lock()
                .await;

            let term = term_lock
                .as_mut()
                .expect("Terminal not initialized!");
            
            term.draw(|f| Self::ui(f, &view, task_displays, benchmark_displays))?;
        }
        
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

    async fn events(&self) -> Result<()> {
        if event::poll(std::time::Duration::from_secs(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('q') {
                    self.channels
                        .get::<engine::EngineSubsystem>()
                        .send(engine::RequestQuit)
                        .await?;
                };

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('1') {
                    *self.view.lock().await = TermView::Tasks;
                }

                if key.kind == event::KeyEventKind::Press && key.code == event::KeyCode::Char('2') {
                    *self.view.lock().await = TermView::Log;
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
