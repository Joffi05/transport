#![allow(dead_code)]

use std::{error::Error};
use std::mem::size_of;
use tokio::net::{TcpStream};

const HEADER_SIZE: usize = size_of::<Header>();
const BUFFER_SIZE: usize = 1024;

//die sind temporär bis serde
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>()) // hier war mal noch .to_vec() und die fn hat dann vec returned
}

unsafe fn any_struct_from_u8_slice<T: Sized + Clone>(bytes: &[u8]) -> T {
    let (head, body, _tail) = bytes.align_to::<T>();
    assert!(
        head.is_empty(),
        "Data was wrongly alligned in: [any_struct_from_u8_slice, event.rs]"
    );
    body[0].clone()
}
//

pub trait Send {
    fn to_sendable(&self) -> &[u8];
}

impl Send for &[u8] {
    fn to_sendable(&self) -> &[u8] {
        self
    }
}
impl Send for Vec<u8> {
    fn to_sendable(&self) -> &[u8] {
        self
    }
}


#[derive(Clone, Debug)]
struct Header {
    //ca. 4.3 GB ist maximum
    data_size: u32,
}

impl Header {
    pub fn new(data_size: u32) -> Self {
        Header { data_size }
    }

    pub fn get_size(&self) -> u32 {
        self.data_size
    }
}

impl Send for Header {
    fn to_sendable(&self) -> &[u8] {
        unsafe { any_as_u8_slice::<Self>(&self) }
    }
}

struct Message {
    header: Header,
    data: Vec<u8>,
}

impl Message {
    pub fn new(header: Header, data: Vec<u8>) -> Message {
        Message {
            header,
            data,
        }
    }
}

struct Socket {
    stream: TcpStream,
    recv_buffer: Vec<u8>,
}

impl Socket {
    pub fn new(stream: TcpStream) -> Self {
        Socket { 
            stream: stream,
            recv_buffer: vec![],
        }
    }

    async fn send_u8_arr(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        loop {
            //wait until writable
            self.stream.writable().await?;
            match self.stream.try_write(data) {
                //panic if the read length isnt the header length, that makes version comability problamatic
                Ok(ref n) if n != &data.len() => panic!("couldnt send all"),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
                _ => break,
            }
        }

        Ok(())
    }

    async fn recv(&mut self) -> Result<(), Box<dyn Error>> {
        let mut buf: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

        loop {
            let mut _bytes_read: usize = 0;
            //wait until readable
            self.stream.readable().await?;

            match self.stream.try_read(&mut buf) {
                Ok(ref n) if *n == 0 as usize => break,
                Ok(n) => _bytes_read = n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }

            self.recv_buffer.extend(&buf[0.._bytes_read]);
        }

        Ok(())
    }

    fn parse_msg(&mut self) -> Message {
        let header = unsafe {any_struct_from_u8_slice::<Header>(&self.recv_buffer.clone()[0..HEADER_SIZE])};
        let msg = &self.recv_buffer.clone()[HEADER_SIZE..(header.get_size() + HEADER_SIZE as u32) as usize];

        self.recv_buffer.drain(0..(header.data_size + HEADER_SIZE as u32) as usize);

        Message::new(header, msg.to_vec())
    }

    fn parse_all(&mut self) -> Vec<Message> {
        let mut messages: Vec<Message> = vec![];

        while self.recv_buffer.len() >= HEADER_SIZE {
            messages.push(self.parse_msg());
        }
        messages
    }

    pub async fn send<T: Send>(&mut self, data: &T) -> Result<(), Box<dyn Error>> {
        let data = data.to_sendable();
        
        //creating header for data
        let header = Header::new(data.len() as u32);
        //sending header
        self.send_u8_arr(header.to_sendable()).await?;

        //sending data
        self.send_u8_arr(data).await?;
        
        Ok(())
    }

    pub async fn recv_one(&mut self) -> Result<Message, Box<dyn Error>> {
        if self.recv_buffer.len() == 0 {
            self.recv().await?;
        }

        Ok(self.parse_msg())        
    }

    pub async fn recv_all(&mut self) -> Result<Vec<Message>, Box<dyn Error>> {
        if self.recv_buffer.len() == 0 {
            self.recv().await?;
        }
        
        Ok(self.parse_all())  
    }
}


#[allow(unused)]
#[cfg(test)]
mod tests {
    use crate::socket::*;
    use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

    #[tokio::test]
    async fn test_single_sending() {
        let handle_recv = tokio::spawn(async move {
            let listener_recv = TcpListener::bind("127.0.0.1:7001").await.unwrap();
            let (socket_recv, _) = listener_recv.accept().await.unwrap();
            let mut recver = Socket::new(socket_recv);
            //

            recver.recv().await.unwrap();
            assert_eq!(recver.recv_one().await.unwrap().data, vec![0; 100])

        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7001").await.unwrap();
            let mut sender = Socket::new(socket_sender);
            //

            let mut send_vec = vec![0; 100];
            sender.send(&send_vec).await;
        });

        tokio::join!(handle_recv, handle_send);
    }

    #[tokio::test]
    async fn test_sending() {
        let handle_recv = tokio::spawn(async move {
            let listener_recv = TcpListener::bind("127.0.0.1:7000").await.unwrap();
            let (socket_recv, _) = listener_recv.accept().await.unwrap();
            let mut recver = Socket::new(socket_recv);
            //

            assert_eq!(recver.recv_one().await.unwrap().data, vec![0,1,2,3,4,5,6,7,8]);

            assert_eq!(recver.recv_one().await.unwrap().data, vec![0; 10_000]);

            let msgs = recver.recv_all().await.unwrap();

            for i in 0..10 {
                assert_eq!(msgs[i].data, vec![0; 8])
            }
        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7000").await.unwrap();
            let mut sender = Socket::new(socket_sender);
            //


            let mut send_vec: Vec<u8> = vec![0,1,2,3,4,5,6,7,8];
            sender.send(&send_vec).await;

            let mut send_vec = vec![0; 10_000];
            sender.send(&send_vec).await;

            for i in 0..10 {
                let mut send_vec: Vec<u8> = vec![0; 8];
                sender.send(&send_vec).await;
            }
        });

        tokio::join!(handle_recv, handle_send);
    }
}
