use std::{error::Error, ops::Index, rc::Rc};
use crate::socket::Sendable;

pub struct Funnel {
    funcs: Vec<Rc<dyn Fn(Vec<u8>) -> Vec<u8>>>,
    current: u32,
}

impl Funnel {
    pub fn new() -> Self {
        Self {
            funcs: vec![],
            current: 0
        }
    }

    pub fn next(&mut self) -> Result<Rc<dyn Fn(Vec<u8>) -> Vec<u8>>, Box<dyn Error>> {
        self.current += 1;
        Ok(self.funcs[self.current as usize].clone())
    }

    pub fn current(&mut self) -> Result<Rc<dyn Fn(Vec<u8>) -> Vec<u8>>, Box<dyn Error>> {
        Ok(self.funcs[self.current as usize].clone())
    }

    pub fn add_fn(&mut self, fun: Box<dyn Fn(Vec<u8>) -> Vec<u8>>) {
        self.funcs.push(Rc::new(fun))
    }

    pub fn funnel_through(&mut self, start_val: Vec<u8>) -> Vec<u8> {
        let mut last = self.funcs[0](start_val);

        Box::new(for (i, e) in self.funcs.iter().enumerate() {
            if i >= 1 {
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
        funnel.add_fn(Box::new(shortener));
        funnel.add_fn(Box::new(shortener));
        funnel.add_fn(Box::new(shortener));
        let result = funnel.funnel_through(test_vec);
        assert_eq!(result, vec![0,0])
    }

    fn shortener(mut vec: Vec<u8>) -> Vec<u8> {
        match vec.pop() {
            Some(x) => {},
            None => panic!("scheise")
        }
        vec
    }
}