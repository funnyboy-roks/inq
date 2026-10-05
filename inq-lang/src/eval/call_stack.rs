use crate::eval::value::function::FunctionId;

// This struct should be farily cheap to clone
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StackFrame {
    pub id: FunctionId,
}

#[derive(Debug, Clone)]
pub(crate) struct CallStack {
    frames: Vec<StackFrame>,
}

impl CallStack {
    pub(crate) const MAX_DEPTH: usize = 128;

    pub(crate) fn new() -> Self {
        Self {
            frames: Default::default(),
        }
    }

    pub fn push(&mut self, frame: StackFrame) {
        self.frames.push(frame);
    }

    pub fn pop(&mut self, frame: &StackFrame) {
        let popped = self.frames.pop();
        assert_eq!(popped.as_ref(), Some(frame));
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// calls the provided function, asserting that no changes were made to the callstack
    pub fn without_change<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let start_depth = self.depth();
        let t = f(self);
        // TODO: this should probably have a more thorough assertion
        assert_eq!(start_depth, self.depth());
        t
    }

    pub(crate) fn with_frame<T>(&mut self, frame: StackFrame, f: impl FnOnce(&mut Self) -> T) -> T {
        self.push(frame.clone());
        let t = f(self);
        self.pop(&frame);
        t
    }
}
