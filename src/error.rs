use std::error::Error;
use std::process;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{
        self,
        Event, KeyCode, KeyEventKind
    },
    layout::{
        Layout, 
        Constraint,
    },
    text::{
        Line
    },
    widgets::{
        Paragraph,
        Block,
        Wrap,
    },
    DefaultTerminal,
    prelude::*,
    style::{
        Color
    }
};


pub fn error() {
    let mut terminal = ratatui::init();
    let _error_widget = ErrorTerm::default().run(&mut terminal);
    ratatui::restore();
    process::exit(1);
}

pub fn error_with_message(message: &str) {
    let mut terminal = ratatui::init();
    let _error_widget = ErrorTerm::new(message).run(&mut terminal);
    ratatui::restore();
    process::exit(1);
}

struct ErrorTerm {
    should_exit: bool,
    message: String
}

impl ErrorTerm {
    fn new(message: &str) -> ErrorTerm {
        ErrorTerm {
           should_exit: false,
           message: String::from(message)
        }
    }
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn Error>> {
        while !self.should_exit {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            let e = event::read()?;
            self.handle_events(e)?;
        }
        Ok(())
    }

    fn handle_events(&mut self, e: Event) -> Result<(), Box<dyn Error>> {
        if let Event::Key(key) = e {
            if key.kind == KeyEventKind::Press {
               match key.code {
                   KeyCode::Esc | KeyCode::Char('q') => {
                       self.should_exit = true;
                   },
                   _ => {}
                }
            }
        }
        Ok(())
    }

}

impl Default for ErrorTerm {
    fn default() -> ErrorTerm {
        ErrorTerm {
            should_exit: false,
            message: String::new()
        }
    }
}

impl Widget for &ErrorTerm {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines = Vec::with_capacity(20);

        if self.message != "" {
            lines = vec![
                Line::raw(""),
                Line::raw("Media Timer could not be started."),
                Line::raw(""),
                Line::raw(&self.message),
                Line::raw(""),
                Line::raw("To reset the program run 'mediatimer' in a terminal."),
                Line::raw(""),
                Line::raw("Or manually edit the mediatimer config file at"),
                Line::raw("/home/adaptable/.mediatimer_config/vars"),
            ];

        } else {
            lines = vec![
                Line::raw(""),
                Line::raw("Media Timer could not be started."),
                Line::raw(""),
                Line::raw("Please run 'mediatimer' in a terminal to setup the program."),
                Line::raw(""),
                Line::raw("Or manually edit the Media Timer config file at"),
                Line::raw("/home/adaptable/.mediatimer_config/vars"),
            ];
        }

        let title = "ERROR";

        Paragraph::new(lines)
            .block(
                Block::bordered()
                .black()
                .bg(Color::Gray)
                .title(title.bold().into_centered_line())
            )
            .bg(Color::Red)
            .white()
            .centered()
            .wrap(Wrap { trim: true })
            .render(area, buf)
    }
}
