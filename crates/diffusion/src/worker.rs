use diffusion_core::{DiffEngine, DiffOptions, DiffResult, Document, Syntax, highlight};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};

pub struct Comparison {
    pub left: Document,
    pub right: Document,
    pub diff: DiffResult,
    pub left_syntax: Syntax,
    pub right_syntax: Syntax,
    pub max_columns: usize,
}
pub struct Request {
    pub generation: u64,
    pub paths: [PathBuf; 2],
    pub options: DiffOptions,
    pub demo: bool,
}
pub struct Reply {
    pub generation: u64,
    pub result: Result<Comparison, String>,
}
#[derive(Default)]
struct Queue {
    pending: Option<Request>,
    closed: bool,
}
pub struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    pub replies: mpsc::Receiver<Reply>,
}
impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let q = queue.clone();
        let (tx, replies) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let request = {
                    let (lock, wake) = &*q;
                    let mut state = lock.lock().unwrap();
                    while state.pending.is_none() && !state.closed {
                        state = wake.wait(state).unwrap();
                    }
                    if state.closed {
                        break;
                    }
                    state.pending.take().unwrap()
                };
                let result = (|| {
                    let left = if request.demo {
                        Document::from_text(
                            "example/before.rs".into(),
                            include_str!("../../../fixtures/before.rs").into(),
                        )
                    } else {
                        Document::load(&request.paths[0])?
                    };
                    let right = if request.demo {
                        Document::from_text(
                            "example/after.rs".into(),
                            include_str!("../../../fixtures/after.rs").into(),
                        )
                    } else {
                        Document::load(&request.paths[1])?
                    };
                    let diff = DiffEngine::compare(&left, &right, &request.options);
                    if q.0.lock().unwrap().pending.is_some() {
                        return Err("Superseded".into());
                    }
                    let left_syntax = highlight(&left);
                    if q.0.lock().unwrap().pending.is_some() {
                        return Err("Superseded".into());
                    }
                    let right_syntax = highlight(&right);
                    let max_columns = [&left, &right]
                        .iter()
                        .flat_map(|d| {
                            (0..d.lines.len()).map(|i| {
                                d.display_line(i)
                                    .chars()
                                    .take(4096)
                                    .map(|c| if c == '\t' { 4 } else { 1 })
                                    .sum::<usize>()
                            })
                        })
                        .max()
                        .unwrap_or(0);
                    Ok(Comparison {
                        left,
                        right,
                        diff,
                        left_syntax,
                        right_syntax,
                        max_columns,
                    })
                })();
                if tx
                    .send(Reply {
                        generation: request.generation,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { queue, replies }
    }
    pub fn submit(&self, request: Request) {
        self.queue.0.lock().unwrap().pending = Some(request);
        self.queue.1.notify_one();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.queue.0.lock().unwrap().closed = true;
        self.queue.1.notify_one();
    }
}
