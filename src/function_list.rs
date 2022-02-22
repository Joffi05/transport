use std::{error::Error, ops::Index, rc::Rc};
use crate::socket::Sendable;

pub struct Funnel {
    funcs: Vec<Rc<dyn Fn(Box<dyn Sendable>) -> Box<dyn Sendable>>>,
    current: u32,
}

impl Funnel {
    pub fn new() -> Self {
        Self {
            funcs: vec![],
            current: 0
        }
    }

    pub fn next(&mut self) -> Result<Rc<dyn Fn(Box<dyn Sendable>) -> Box<dyn Sendable>>, Box<dyn Error>> {
        self.current += 1;
        Ok(self.funcs[self.current as usize].clone())
    }

    pub fn current(&mut self) -> Result<Rc<dyn Fn(Box<dyn Sendable>) -> Box<dyn Sendable>>, Box<dyn Error>> {
        Ok(self.funcs[self.current as usize].clone())
    }

    pub fn add_fn(&mut self, fun: Box<dyn Fn(Box<dyn Sendable>) -> Box<dyn Sendable>>) {
        self.funcs.push(Rc::new(fun))
    }

    pub fn funnel_through(&mut self, start_val: Box<dyn Sendable>) -> Box<dyn Sendable> {
        let mut last: Box<dyn Sendable> = self.funcs[0](start_val);

        Box::new(for (i, e) in self.funcs.iter().enumerate() {
            if i >= 2 {
                last = e(last)
            }
        });

        last
    }
}


#[allow(unused)]
#[cfg(test)]
mod tests {
    use core::panic;

    use super::Funnel;
    use super::Sendable;

    #[test]
    fn funnel_test() {
        let mut test_vec = vec![0,0,0,0,0];

        type FunnelFn = fn(dyn Sendable) -> dyn Sendable;

        let mut funnel = Funnel::new();
            funnel.add_fn(shortener as Box<dyn Fn(Box<(dyn Sendable + 'static)>) -> Box<(dyn Sendable + 'static)>>);
    }

    fn shortener(mut vec: Vec<u8>) -> impl Sendable {
        match vec.pop() {
            Some(x) => {},
            None => panic!("scheise")
        }
        vec
    }
}