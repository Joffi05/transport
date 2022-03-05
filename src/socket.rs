#![allow(dead_code)]

use bytes::BytesMut;
use std::error::Error;
use std::mem::size_of;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use bincode::{DefaultOptions, Options};

const HEADER_SIZE: usize = size_of::<Header>();
const BUFFER_SIZE: usize = 1024;
const MAX_SENDING_SIZE: usize = 1024;

/* 
//die sind temporär bis serde
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    // hier war mal noch .to_vec() und die fn hat dann vec returned
} */
/* 
unsafe fn any_struct_from_u8_slice<T: Sized + Clone>(bytes: &[u8]) -> T {
    let (head, body, _tail) = bytes.align_to::<T>();
    assert!(
        head.is_empty(),
        "Data was wrongly alligned in: [any_struct_from_u8_slice, event.rs]"
    );
    body[0].clone()
} */
//

fn any_as_u8_slice<T: Serialize>(p: &T) -> Result<Vec<u8>, Box<bincode::ErrorKind>> {
    bincode::serialize(p)
}


fn any_struct_from_u8_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Box<bincode::ErrorKind>> {
    bincode::deserialize(bytes)
}

pub trait Sendable {
    fn to_sendable(&self) -> Vec<u8>;
}

impl Sendable for &[u8] {
    fn to_sendable(&self) -> Vec<u8> {
        self.to_vec()
    }
}
impl Sendable for Vec<u8> {
    fn to_sendable(&self) -> Vec<u8> {
        self.to_vec()
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
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

impl Sendable for Header {
    fn to_sendable(&self) -> Vec<u8> {
        let bin_vec = any_as_u8_slice::<Self>(&self).unwrap();

        println!("Bin vec size: {:?}", bin_vec.len());


        bin_vec
    }
}

#[derive(Serialize, Deserialize)]
struct Message {
    header: Header,
    data: Vec<u8>,
}

impl Message {
    pub fn new(header: Header, data: Vec<u8>) -> Message {
        Message { header, data }
    }
}
pub struct Socket {
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
                //panic if the read length isnt the header length, hier muss error handling rein
                Ok(ref n) if n != &data.len() => panic!("couldnt send all"),
                //continue if blocking
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                //anderer error wird hochpropagiert
                Err(e) => return Err(e.into()),
                _ => break,
            }
        }

        Ok(())
    }

    async fn recv(&mut self) -> Result<(), Box<dyn Error>> {
        let mut buf: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

        let mut _bytes_read: usize = 0;
        //wait until readable
        self.stream.readable().await?;

        match self.stream.try_read(&mut buf) {
            //wenn socket closed: break
            Ok(ref n) if *n == 0 as usize => return Ok(()),
            Ok(n) => _bytes_read = n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
            Err(e) => return Err(e.into()),
        }

        //appende gelesenes an den buffer
        self.recv_buffer.extend(&buf[0.._bytes_read]);

        Ok(())
    }

    fn parse_msg(&mut self) -> Result<Message, Box<dyn Error>> {
        let cloned_buf = self.recv_buffer.clone();

        if self.recv_buffer.len() < (HEADER_SIZE) {
            return Err("Couldn't parse message".into())
        }

        let header = any_struct_from_u8_slice::<Header>(&cloned_buf[0..HEADER_SIZE])?;
        
        if self.recv_buffer.len() < (header.data_size as usize + HEADER_SIZE) {
            return Err("Couldn't parse message".into())
        }

        let msg = &cloned_buf[HEADER_SIZE..(header.get_size() + HEADER_SIZE as u32) as usize];

        self.recv_buffer.drain(0..header.data_size as usize + HEADER_SIZE);

        Ok(Message::new(header, msg.to_vec()))
    }

    /*fn parse_all(&mut self) -> Vec<Message> {
        let mut messages: Vec<Message> = vec![];

        while self.recv_buffer.len() >= HEADER_SIZE {
            messages.push(self.parse_msg());
        }
        messages
    } */

    /// Sends an item with the Sendable trait to the in the
    /// Socket specified Address.
    pub async fn send<T: Sendable>(&mut self, data: &T) -> Result<(), Box<dyn Error>> {
        let data = data.to_sendable();

        //creating header for data
        let header = Header::new(data.len() as u32);
        //sending header
        self.send_u8_arr(&header.to_sendable()).await?;

        let last_index = 0;
        //sending data split in 1024 packages
        for i in 0..((data.len() - (data.len() % MAX_SENDING_SIZE)) / MAX_SENDING_SIZE) {
            self.send_u8_arr(&data[i * MAX_SENDING_SIZE..(i + 1) * MAX_SENDING_SIZE]).await?;
        }
        //sending rest thats not dividable by 1024
        self.send_u8_arr(&data[last_index * MAX_SENDING_SIZE..]).await?;

        Ok(())
    }

    /// First recieves new data, if the buffer is empty and
    /// then tries to parse a new Message.
    /* pub async fn recv_one(&mut self) -> Result<Message, Box<dyn Error>> {
        if self.recv_buffer.len() == 0 {
            self.recv().await?;
        }

        Ok(self.parse_msg())
    } */


    //new recv_one()
    pub async fn recv_one(&mut self) -> Result<Option<Message>, Box<dyn Error>> {
        loop {
            if let Ok(msg) = self.parse_msg() {
                return Ok(Some(msg));
            }

            let mut buf: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

            let mut _bytes_read: usize = 0;
            //wait until readable
            self.stream.readable().await?;
    
            match self.stream.try_read(&mut buf) {
                //wenn socket closed: break
                Ok(ref n) if *n == 0 as usize => return Ok(None),
                Ok(n) => _bytes_read = n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
    
            //appende gelesenes an den buffer
            self.recv_buffer.extend(&buf[0.._bytes_read]);
        }
    }

    /*/// First recieves new data, if the buffer is empty and
    /// then tries to parse all available data into a Vec<Message>.
    pub async fn recv_all(&mut self) -> Result<Vec<Message>, Box<dyn Error>> {
        if self.recv_buffer.len() == 0 {
            self.recv().await?;
        }

        Ok(self.parse_all())
    }*/
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use std::vec;

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
            let x = recver.recv_one().await.unwrap();
            assert_eq!(x.unwrap().data, vec![0; 100])
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

            assert_eq!(
                recver.recv_one().await.unwrap().unwrap().data,
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
            );

            assert_eq!(recver.recv_one().await.unwrap().unwrap().data, vec![0; 10_000]);

            /*let msgs = recver.recv_all().await.unwrap();

            for i in 0..10 {
                assert_eq!(msgs[i].data, vec![0; 8])
            } */
        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7000").await.unwrap();
            let mut sender = Socket::new(socket_sender);
            //

            let mut send_vec: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
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

    #[tokio::test]
    async fn test_network_sending() {
        let mut socket_sender = TcpStream::connect("192.168.178.124:7000").await.unwrap();
        let mut sender = Socket::new(socket_sender);

        let mut send_vec: Vec<u8> = vec![0; 100];

        println!("sending...");
        sender.send(&send_vec).await;

        println!("Finished!");
    }

    #[tokio::test]
    async fn test_network_recieving() {
        let listener_recv = TcpListener::bind("192.168.178.124:7000").await.unwrap();

        match listener_recv.accept().await {
            Ok(x) => {
                println!("socket {:?}", x.0);
                let mut recver = Socket::new(x.0);

                println!("recieving...");
                let recvd = recver.recv_one().await.unwrap().unwrap().data;

                println!("recvd.len: {}", recvd.len());
                assert_eq!(recvd.len(), 100);

                println!("Finished!");
            }
            Err(e) => panic!("err"),
        }
    }
}
