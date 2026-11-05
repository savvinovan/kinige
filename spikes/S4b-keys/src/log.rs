//! What the client and the server write to their logs (ADR-0001 §8.8, appendix §A.4 "Logs"): event kinds, ids and error categories
//! only. One shared sink collects both sides' lines so a test can scan everything that was ever logged.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default, Debug)]
pub struct Log(Rc<RefCell<Vec<String>>>);

impl Log {
    pub fn new() -> Log {
        Log::default()
    }

    /// One line. Callers pass a fixed event name and, at most, ids and categories.
    pub fn event(&self, line: impl Into<String>) {
        self.0.borrow_mut().push(line.into());
    }

    pub fn lines(&self) -> Vec<String> {
        self.0.borrow().clone()
    }

    /// Everything logged, as one text.
    pub fn text(&self) -> String {
        self.0.borrow().join("\n")
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }
}
