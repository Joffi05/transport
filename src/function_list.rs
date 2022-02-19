use std::error::Error;
use crate::socket::Sendable;



pub struct Funnel {
    funcs: Vec<Box<dyn Fn(dyn Sendable) -> dyn Sendable>>,
    current: u32,
}

impl Funnel {
    pub fn new() -> Self {
        Self {
            funcs: vec![],
            current: 0
        }
    }

    pub fn next(&mut self) -> Result<Box<dyn Fn(Box<dyn Sendable>) -> Box<dyn Sendable>>, Box<dyn Error>> {
        self.current += 1;
        Ok(Box::new(self.funcs.get(self.current)))
    }

    pub fn current() {

    }

    pub fn add_fn() {

    }

    pub fn funnel_through() {

    }
}
