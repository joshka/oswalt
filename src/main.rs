use std::{thread, time::Duration};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use ratatui::Frame;
use tokio::sync::mpsc::{self, error::TryRecvError, UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let handle = EventLoopHandle::new();
    let ui_thread = thread::spawn(|| run(handle));
    let result = ui_thread.join().unwrap();
    ratatui::restore();
    result
}

fn run(mut handle: EventLoopHandle) -> color_eyre::Result<()> {
    let mut terminal = ratatui::init();
    const FPS: f64 = 60.0;
    let mut last_event = None;
    loop {
        match handle.next() {
            Ok(event) => {
                if let Event::Key(KeyEvent { code, .. }) = event {
                    if code == KeyCode::Char('q') {
                        handle.cancel();
                    }
                }
                last_event = Some(event);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }
        thread::sleep(Duration::from_secs_f64(1.0 / FPS));
        terminal.draw(|frame| render(frame, &last_event))?;
    }
    Ok(())
}

fn render(frame: &mut Frame, event: &Option<Event>) {
    let text = format!("Last event: {:?}", event);
    frame.render_widget(text, frame.area());
}

struct EventLoop {
    tx: UnboundedSender<Event>,
    cancellation_token: CancellationToken,
    events: EventStream,
}

impl EventLoop {
    fn new(tx: UnboundedSender<Event>, cancellation_token: CancellationToken) -> Self {
        Self {
            tx,
            cancellation_token,
            events: EventStream::new(),
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                _ = self.cancellation_token.cancelled() => break,
                event = self.events.next() => {
                    if let Some(event) = event {
                        let event = event.unwrap();
                        let _ = self.tx.send(event); // ignore error (indicates receiver dropped)
                    }
                }
            }
        }
    }
}

struct EventLoopHandle {
    rx: UnboundedReceiver<Event>,
    cancellation_token: CancellationToken,
}

impl EventLoopHandle {
    fn new() -> Self {
        let cancellation_token = CancellationToken::new();
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = EventLoop::new(tx, cancellation_token.clone());
        tokio::spawn(actor.run());
        Self {
            rx,
            cancellation_token,
        }
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    fn next(&mut self) -> Result<Event, TryRecvError> {
        self.rx.try_recv()
    }
}
