//! Terminal UI abstraction and event loop.
//!
//! Manages terminal initialization, event polling, and rendering via ratatui.
//! Provides a clean interface to the underlying crossterm and ratatui libraries.

#![allow(dead_code)] // Remove this once you start using the code

use std::{
    io::{Stdout, stdout},
    ops::{Deref, DerefMut},
    time::Duration,
};

use crossterm::{
    cursor,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind, MouseEvent,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::backend::CrosstermBackend as Backend;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::interval,
};
use tokio_util::sync::CancellationToken;
use tracing::error;

/// Terminal UI events — keyboard, mouse, resize, paste, and lifecycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    /// Initialization event (typically the first event).
    Init,
    /// Quit request.
    Quit,
    /// Error occurred.
    Error,
    /// Terminal closed/exited.
    Closed,
    /// Game logic tick (configurable rate, default 4/sec).
    Tick,
    /// Render frame (configurable rate, default 60/sec).
    Render,
    /// Terminal gained focus.
    FocusGained,
    /// Terminal lost focus.
    FocusLost,
    /// Pasted text (requires EnableBracketedPaste).
    Paste(String),
    /// Keyboard key pressed.
    Key(KeyEvent),
    /// Mouse event (click, scroll, move, etc.).
    Mouse(MouseEvent),
    /// Terminal resized to `(width, height)` cells.
    Resize(u16, u16),
}

/// Terminal UI state and event loop.
///
/// Manages the crossterm backend, ratatui Terminal, and the event polling loop.
pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<Stdout>>,
    pub task: JoinHandle<()>,
    pub cancellation_token: CancellationToken,
    pub event_rx: UnboundedReceiver<Event>,
    pub event_tx: UnboundedSender<Event>,
    pub frame_rate: f64,
    pub tick_rate: f64,
    pub mouse: bool,
    pub paste: bool,
}

impl Tui {
    /// Create a new [`Tui`] instance bound to `stdout`.
    ///
    /// Initialises the crossterm backend and a ratatui `Terminal` but does
    /// **not** enter raw mode or start the event loop.  Call [`Tui::enter`]
    /// to do both.
    ///
    /// # Errors
    ///
    /// Returns an error if the ratatui `Terminal` cannot be constructed
    /// (e.g. `stdout` is not a valid TTY).
    pub fn new() -> color_eyre::Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Ok(Self {
            terminal: ratatui::Terminal::new(Backend::new(stdout()))?,
            task: tokio::spawn(async {}),
            cancellation_token: CancellationToken::new(),
            event_rx,
            event_tx,
            frame_rate: 60.0,
            tick_rate: 4.0,
            mouse: false,
            paste: false,
        })
    }

    /// Set the game-logic tick rate in ticks per second (builder pattern).
    pub fn tick_rate(mut self, tick_rate: f64) -> Self {
        self.tick_rate = tick_rate;
        self
    }

    /// Set the render frame rate in frames per second (builder pattern).
    pub fn frame_rate(mut self, frame_rate: f64) -> Self {
        self.frame_rate = frame_rate;
        self
    }

    /// Enable or disable mouse event capture (builder pattern).
    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    /// Enable or disable bracketed paste support (builder pattern).
    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }

    /// (Re)start the background event-polling task.
    ///
    /// Cancels any previously running task, creates a fresh cancellation token,
    /// then spawns a new Tokio task that generates [`Event::Tick`] and
    /// [`Event::Render`] on configured intervals while forwarding crossterm
    /// events via the unbounded channel.
    pub fn start(&mut self) {
        self.cancel();
        self.cancellation_token = CancellationToken::new();
        let event_loop = Self::event_loop(
            self.event_tx.clone(),
            self.cancellation_token.clone(),
            self.tick_rate,
            self.frame_rate,
        );
        self.task = tokio::spawn(async {
            event_loop.await;
        });
    }

    /// Core event-polling loop — runs until the cancellation token is triggered.
    ///
    /// Sends an [`Event::Init`] immediately on startup, then drives three
    /// concurrent futures in a `select!` loop:
    ///
    /// - A `tick_interval` that fires at `tick_rate` Hz → [`Event::Tick`]
    /// - A `render_interval` that fires at `frame_rate` Hz → [`Event::Render`]
    /// - The crossterm [`EventStream`], mapping raw crossterm events to the
    ///   typed [`Event`] variants; non-press key events and unrecognised
    ///   crossterm events are silently discarded.
    ///
    /// The loop exits when the cancellation token fires, the event stream
    /// closes, or the receiver end of the channel has been dropped.  Before
    /// returning it calls `cancellation_token.cancel()` so the [`Tui`] owner
    /// can observe the shutdown.
    async fn event_loop(
        event_tx: UnboundedSender<Event>,
        cancellation_token: CancellationToken,
        tick_rate: f64,
        frame_rate: f64,
    ) {
        let mut event_stream = EventStream::new();
        let mut tick_interval = interval(Duration::from_secs_f64(1.0 / tick_rate));
        let mut render_interval = interval(Duration::from_secs_f64(1.0 / frame_rate));

        // if this fails, then it's likely a bug in the calling code
        event_tx
            .send(Event::Init)
            .expect("failed to send init event");
        loop {
            let event = tokio::select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }
                _ = tick_interval.tick() => Event::Tick,
                _ = render_interval.tick() => Event::Render,
                crossterm_event = event_stream.next().fuse() => match crossterm_event {
                    Some(Ok(event)) => match event {
                        CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => Event::Key(key),
                        CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
                        CrosstermEvent::Resize(x, y) => Event::Resize(x, y),
                        CrosstermEvent::FocusLost => Event::FocusLost,
                        CrosstermEvent::FocusGained => Event::FocusGained,
                        CrosstermEvent::Paste(s) => Event::Paste(s),
                        _ => continue, // ignore other events
                    }
                    Some(Err(_)) => Event::Error,
                    None => break, // the event stream has stopped and will not produce any more events
                },
            };
            if event_tx.send(event).is_err() {
                // the receiver has been dropped, so there's no point in continuing the loop
                break;
            }
        }
        cancellation_token.cancel();
    }

    /// Stop the background event-polling task, blocking until it finishes.
    ///
    /// Cancels the task via the cancellation token, then polls
    /// [`JoinHandle::is_finished`] in 1 ms increments.  If the task has not
    /// finished within 50 ms it is forcibly aborted; if it still has not
    /// finished after 100 ms a tracing error is emitted and the method returns.
    ///
    /// # Errors
    ///
    /// Currently always returns `Ok(())`.  The error return type is kept for
    /// future compatibility.
    pub fn stop(&self) -> color_eyre::Result<()> {
        self.cancel();
        let mut counter = 0;
        while !self.task.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
            counter += 1;
            if counter > 50 {
                self.task.abort();
            }
            if counter > 100 {
                error!("Failed to abort task in 100 milliseconds for unknown reason");
                break;
            }
        }
        Ok(())
    }

    /// Enter the terminal's alternate screen and start the event loop.
    ///
    /// Enables crossterm raw mode, switches to the alternate screen buffer,
    /// hides the cursor, and — if configured — enables mouse capture and
    /// bracketed paste.  Finally calls [`Tui::start`] to spawn the background
    /// event-polling task.
    ///
    /// Call [`Tui::exit`] to restore the terminal to its original state.
    ///
    /// # Errors
    ///
    /// Returns an error if any crossterm terminal-setup call fails (e.g. the
    /// output is not a TTY, or the OS denies the `ioctl`).
    pub fn enter(&mut self) -> color_eyre::Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(stdout(), EnterAlternateScreen, cursor::Hide)?;
        if self.mouse {
            crossterm::execute!(stdout(), EnableMouseCapture)?;
        }
        if self.paste {
            crossterm::execute!(stdout(), EnableBracketedPaste)?;
        }
        self.start();
        Ok(())
    }

    /// Leave the alternate screen and restore the terminal to its original state.
    ///
    /// Stops the background event-polling task, flushes any buffered output,
    /// disables bracketed paste and mouse capture (if they were enabled), shows
    /// the cursor, leaves the alternate screen, and disables raw mode.
    ///
    /// This method is also called by the [`Drop`] implementation, so it is safe
    /// to let a [`Tui`] drop naturally on process exit.
    ///
    /// # Errors
    ///
    /// Returns an error if [`Tui::stop`], [`Tui::flush`], or any crossterm
    /// tear-down call fails.
    pub fn exit(&mut self) -> color_eyre::Result<()> {
        self.stop()?;
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            if self.paste {
                crossterm::execute!(stdout(), DisableBracketedPaste)?;
            }
            if self.mouse {
                crossterm::execute!(stdout(), DisableMouseCapture)?;
            }
            crossterm::execute!(stdout(), LeaveAlternateScreen, cursor::Show)?;
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }

    /// Signal the background event-polling task to stop.
    ///
    /// Triggers the internal [`CancellationToken`].  The task will exit at its
    /// next `select!` iteration.  This does **not** wait for the task to finish;
    /// call [`Tui::stop`] if you need to block until it has exited.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Suspend the application, releasing the terminal back to the shell.
    ///
    /// Calls [`Tui::exit`] to restore the terminal, then raises `SIGTSTP` on
    /// Unix platforms (a no-op on Windows) so the shell job-control layer can
    /// pause the process.  Call [`Tui::resume`] to re-enter the terminal after
    /// the process is foregrounded again.
    ///
    /// # Errors
    ///
    /// Returns an error if [`Tui::exit`] fails or if `signal_hook` cannot raise
    /// `SIGTSTP`.
    pub fn suspend(&mut self) -> color_eyre::Result<()> {
        self.exit()?;
        #[cfg(not(windows))]
        signal_hook::low_level::raise(signal_hook::consts::signal::SIGTSTP)?;
        Ok(())
    }

    /// Resume the application after a [`Tui::suspend`].
    ///
    /// Re-enters the alternate screen and restarts the event-polling task by
    /// delegating to [`Tui::enter`].
    ///
    /// # Errors
    ///
    /// Returns an error if [`Tui::enter`] fails (see its documentation).
    pub fn resume(&mut self) -> color_eyre::Result<()> {
        self.enter()?;
        Ok(())
    }

    /// Receive the next [`Event`] from the background event-polling task.
    ///
    /// Awaits the next item on the internal unbounded channel.  Returns `None`
    /// when the sender has been dropped (i.e. the event loop has exited and
    /// will not produce further events).
    pub async fn next_event(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

/// Dereference to the inner [`ratatui::Terminal`], allowing callers to call
/// terminal methods (e.g. [`ratatui::Terminal::draw`]) directly on a [`Tui`]
/// reference without going through the `.terminal` field.
impl Deref for Tui {
    type Target = ratatui::Terminal<Backend<Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

/// Mutable dereference to the inner [`ratatui::Terminal`], enabling direct
/// mutable access (e.g. for `draw` calls that require `&mut Terminal`).
impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

/// Restore the terminal on drop.
///
/// Calls [`Tui::exit`] and panics on failure.  Prefer calling [`Tui::exit`]
/// explicitly so errors can be handled gracefully before the value is dropped.
impl Drop for Tui {
    fn drop(&mut self) {
        self.exit().unwrap();
    }
}
