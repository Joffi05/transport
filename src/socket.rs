use std::error::Error;
use std::mem::size_of;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

const HEADER_SIZE: usize = size_of::<Header>();
const BUFFER_SIZE: usize = 1024;

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> Vec<u8> {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>()).to_vec()
}

unsafe fn any_struct_from_u8_slice<T: Sized + Clone>(bytes: &[u8]) -> T {
    let (head, body, _tail) = bytes.align_to::<T>();
    assert!(
        head.is_empty(),
        "Data was wrongly alligned in: [any_struct_from_u8_slice, event.rs]"
    );
    body[0].clone()
}

#[derive(Clone, Debug)]
struct Header {
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

struct Socket {
    stream: TcpStream,
}

impl Socket {
    pub fn new(stream: TcpStream) -> Self {
        Socket { stream }
    }

    pub async fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let header = Header::new(data.len() as u32);


        if let Err(x) = self.send_header(unsafe { &any_as_u8_slice(&header) }).await {
            panic!("Err {} in socket meth send", x.to_string())
        } 

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

    pub async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let packet_size;

        if let Ok(x) = self.recv_header().await {
            packet_size = unsafe { any_struct_from_u8_slice::<Header>(&x).get_size() };
        } else {
            //error handling
            panic!("Err in sockets recv meth");
        }

        let mut buf: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
        let mut data: Vec<u8> = vec![];

        loop {
            let mut bytes_read: usize = 0;
            //wait until readable
            self.stream.readable().await?;

            match self.stream.try_read(&mut buf) {
                Ok(ref n) if *n == 0 as usize => break,
                Ok(n) => bytes_read = n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
                _ => (),
            }

            data.extend_from_slice(&buf[0..bytes_read]);
        }

        if data.len() as u32 != packet_size {
            //error handling
            println!("data.len(): {} packet size: {}", data.len(), packet_size);
            panic!("data.len != packet_size");
        }

        Ok(data)
    }

    //sollte Header struct als arg nehmen
    pub async fn send_header(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        loop {
            //wait until writable
            self.stream.writable().await?;

            match self.stream.try_write(data) {
                //panic if the read length isnt the header length, that makes version comability problamatic
                Ok(ref n) if n != &HEADER_SIZE => panic!(
                    "didnt send correct sized HEADER, size: {}, header size: {}",
                    n, HEADER_SIZE
                ),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
                _ => break,
            }
        }

        Ok(())
    }

    //sollte Header struct returnen
    pub async fn recv_header(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];

        loop {
            //wait until readable
            self.stream.readable().await?;

            match self.stream.try_read(&mut buf) {
                //panic if the read length isnt the header length, that makes version comability problamatic
                Ok(ref n) if !(n == &HEADER_SIZE) => panic!("didnt recieve correct sized HEADER"),
                //try again next loop
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
                _ => break,
            }
        }
        Ok(buf.to_vec())
    }
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use crate::socket::*;
    #[tokio::test]
    async fn test_header_sending() {
        let handle_recv = tokio::spawn(async move {
            let listener_recv = TcpListener::bind("127.0.0.1:7002").await.unwrap();
            let (socket_recv, _) = listener_recv.accept().await.unwrap();
            let mut recver = Socket::new(socket_recv);
            let recv_header_binary = recver.recv_header().await.unwrap();
            let recv_header = unsafe { any_struct_from_u8_slice::<Header>(&recv_header_binary) };
            
            println!("recv_header: {:?}", recv_header);
            return assert_eq!(recv_header.get_size(), 100);
        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7002").await.unwrap();
            let mut sender = Socket::new(socket_sender);

            let send_header = Header::new(100);

            let binary_header = unsafe { any_as_u8_slice(&send_header) };

            sender.send_header(&binary_header).await.unwrap();
        });

        tokio::join!(handle_recv, handle_send);
    }

    #[tokio::test]
    async fn test_single_sending() {
        let handle_recv = tokio::spawn(async move {
            let listener_recv = TcpListener::bind("127.0.0.1:7001").await.unwrap();
            let (socket_recv, _) = listener_recv.accept().await.unwrap();
            let mut recver = Socket::new(socket_recv);
            //

            let mut recv = recver.recv().await.unwrap();
            assert_eq!(recv, vec![0; 10_000])

        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7001").await.unwrap();
            let mut sender = Socket::new(socket_sender);
            //

            let mut send_vec = vec![0; 10_000];
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

            let mut recv = recver.recv().await.unwrap();
            assert_eq!(recv, vec![0,1,2,3,4,5,6,7,8,9,10,11,12]);

            let mut recv = recver.recv().await.unwrap();
            assert_eq!(recv, vec![0; 10_000])

        });

        let handle_send = tokio::spawn(async move {
            let socket_sender = TcpStream::connect("127.0.0.1:7000").await.unwrap();
            let mut sender = Socket::new(socket_sender);
            //


            let mut send_vec: Vec<u8> = vec![0,1,2,3,4,5,6,7,8,9,10,11,12];
            sender.send(&send_vec).await;

            let mut send_vec = vec![0; 10_000];
            sender.send(&send_vec).await;
        });

        tokio::join!(handle_recv, handle_send);
    }
}
